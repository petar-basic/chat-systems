use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{middleware, Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use crate::middleware::{auth_middleware, AuthUser};
use crate::state::AppState;
use crate::workspace::models::{Channel, ChannelType};

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/channels/:ch_id/messages", get(list_messages))
        .route("/channels/:ch_id/messages", post(send_message))
        .route("/channels/:ch_id/pins", get(list_pins))
        .route("/channels/:ch_id/read", post(mark_read))
        .route("/messages/:msg_id", patch(update_message))
        .route("/messages/:msg_id", delete(delete_message))
        .route("/messages/:msg_id/pin", post(pin_message))
        .route("/messages/:msg_id/pin", delete(unpin_message))
        .route("/messages/:msg_id/thread", get(list_thread))
        .route("/messages/:msg_id/thread", post(reply_to_thread))
        .route("/messages/:msg_id/reactions", get(list_reactions))
        .route("/messages/:msg_id/reactions", post(add_reaction))
        .route(
            "/messages/:msg_id/reactions/:emoji",
            delete(remove_reaction),
        )
        .route("/search", get(search_messages))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::rate_limit::write_rate_limit,
        ))
        .layer(middleware::from_fn(auth_middleware));

    Router::new().merge(routes).with_state(state)
}

async fn list_messages(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<serde_json::Value>> {
    require_channel_access(&state, ch_id, auth.user_id).await?;
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .min(200);
    let cursor = params.get("cursor").and_then(|v| v.parse().ok());
    let messages = state
        .message_repo
        .list_channel_messages(ch_id, limit, cursor)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();
    let reactions = state
        .message_repo
        .list_reactions_for_messages(&message_ids)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let mut reactions_map: std::collections::HashMap<Uuid, Vec<_>> =
        std::collections::HashMap::new();
    for reaction in reactions {
        reactions_map
            .entry(reaction.message_id)
            .or_default()
            .push(reaction);
    }

    let messages_with_reactions: Vec<serde_json::Value> = messages
        .into_iter()
        .map(|msg| {
            let mut msg_json = serde_json::to_value(&msg).unwrap_or_default();
            if let Some(obj) = msg_json.as_object_mut() {
                let msg_reactions = reactions_map.get(&msg.id).cloned().unwrap_or_default();
                obj.insert(
                    "reactions".to_string(),
                    serde_json::to_value(msg_reactions).unwrap_or_default(),
                );
            }
            msg_json
        })
        .collect();

    Ok(Json(serde_json::json!({ "data": messages_with_reactions })))
}

async fn send_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<SendMessageRequest>,
) -> AppResult<Json<Message>> {
    shared_common::validation::validate_message_content(&req.content)?;

    let channel = require_channel_access(&state, ch_id, auth.user_id).await?;

    let msg = if let Some(id) = req.id {
        match state
            .message_repo
            .create_message_with_id(id, ch_id, auth.user_id, &req.content, req.thread_parent_id)
            .await
        {
            Ok(msg) => msg,
            Err(ref e) if is_unique_violation(e) => state
                .message_repo
                .find_by_id(id)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?
                .ok_or_else(|| AppError::Internal("Message ID conflict".into()))?,
            Err(e) => return Err(AppError::Database(e.to_string())),
        }
    } else {
        state
            .message_repo
            .create_message(ch_id, auth.user_id, &req.content, req.thread_parent_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
    };

    link_attachments(
        &state,
        &req.content,
        msg.id,
        channel.workspace_id,
        auth.user_id,
    )
    .await;

    let mentioned_ids = extract_mentioned_user_ids(&req.content);

    let msg_json = serde_json::to_value(&msg).map_err(|e| AppError::Internal(e.to_string()))?;
    let _ = state
        .publisher
        .publish_message_created(&msg_json, channel.workspace_id, &mentioned_ids)
        .await;

    Ok(Json(msg))
}

async fn update_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
    Json(req): Json<UpdateMessageRequest>,
) -> AppResult<Json<Message>> {
    shared_common::validation::validate_message_content(&req.content)?;

    let existing = state
        .message_repo
        .find_by_id(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    if existing.user_id != auth.user_id {
        return Err(AppError::Forbidden(
            "Can only edit your own messages".into(),
        ));
    }

    let msg = state
        .message_repo
        .update_message(msg_id, &req.content)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let msg_json = serde_json::to_value(&msg).map_err(|e| AppError::Internal(e.to_string()))?;
    let _ = state.publisher.publish_message_updated(&msg_json).await;

    Ok(Json(msg))
}

async fn delete_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let existing = state
        .message_repo
        .find_by_id(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    if existing.user_id != auth.user_id {
        let can_mod = state
            .message_repo
            .can_moderate_channel(existing.channel_id, auth.user_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        if !can_mod {
            return Err(AppError::Forbidden(
                "Can only delete your own messages".into(),
            ));
        }
    }

    state
        .message_repo
        .soft_delete_message(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let _ = state
        .publisher
        .publish_message_deleted(msg_id, existing.channel_id)
        .await;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

async fn pin_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let existing = state
        .message_repo
        .find_by_id(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    let can_mod = state
        .message_repo
        .can_moderate_channel(existing.channel_id, auth.user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    if !can_mod {
        return Err(AppError::Forbidden(
            "Requires channel or workspace admin".into(),
        ));
    }

    let msg = state
        .message_repo
        .set_pinned(msg_id, true)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let _ = state
        .publisher
        .publish_message_pinned(msg_id, msg.channel_id, true)
        .await;

    Ok(Json(serde_json::json!({ "status": "pinned" })))
}

async fn unpin_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let existing = state
        .message_repo
        .find_by_id(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    let can_mod = state
        .message_repo
        .can_moderate_channel(existing.channel_id, auth.user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    if !can_mod {
        return Err(AppError::Forbidden(
            "Requires channel or workspace admin".into(),
        ));
    }

    let msg = state
        .message_repo
        .set_pinned(msg_id, false)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let _ = state
        .publisher
        .publish_message_pinned(msg_id, msg.channel_id, false)
        .await;

    Ok(Json(serde_json::json!({ "status": "unpinned" })))
}

async fn list_pins(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_channel_access(&state, ch_id, auth.user_id).await?;
    let pins = state
        .message_repo
        .list_pinned(ch_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "data": pins })))
}

async fn list_thread(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<serde_json::Value>> {
    let channel_id = state
        .message_repo
        .find_by_id(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?
        .channel_id;
    require_channel_access(&state, channel_id, auth.user_id).await?;
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .min(200);
    let offset = params
        .get("offset")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let messages = state
        .message_repo
        .list_thread_messages(msg_id, limit, offset)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "data": messages })))
}

async fn reply_to_thread(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
    Json(req): Json<SendMessageRequest>,
) -> AppResult<Json<Message>> {
    shared_common::validation::validate_message_content(&req.content)?;

    let parent = state
        .message_repo
        .find_by_id(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Parent message not found".into()))?;

    let channel = require_channel_access(&state, parent.channel_id, auth.user_id).await?;

    let msg = state
        .message_repo
        .create_message(parent.channel_id, auth.user_id, &req.content, Some(msg_id))
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    link_attachments(
        &state,
        &req.content,
        msg.id,
        channel.workspace_id,
        auth.user_id,
    )
    .await;

    let mentioned_ids = extract_mentioned_user_ids(&req.content);

    let msg_json = serde_json::to_value(&msg).map_err(|e| AppError::Internal(e.to_string()))?;
    let _ = state
        .publisher
        .publish_message_created(&msg_json, channel.workspace_id, &mentioned_ids)
        .await;

    Ok(Json(msg))
}

async fn list_reactions(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let channel_id = state
        .message_repo
        .find_by_id(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?
        .channel_id;
    require_channel_access(&state, channel_id, auth.user_id).await?;
    let reactions = state
        .message_repo
        .list_reactions(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "data": reactions })))
}

async fn add_reaction(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
    Json(req): Json<AddReactionRequest>,
) -> AppResult<Json<Reaction>> {
    let msg = state
        .message_repo
        .find_by_id(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    require_channel_access(&state, msg.channel_id, auth.user_id).await?;

    let reaction = state
        .message_repo
        .add_reaction(msg_id, auth.user_id, &req.emoji)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let reaction_json =
        serde_json::to_value(&reaction).map_err(|e| AppError::Internal(e.to_string()))?;
    let _ = state
        .publisher
        .publish_reaction_added(&reaction_json, msg.channel_id)
        .await;

    Ok(Json(reaction))
}

async fn remove_reaction(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((msg_id, emoji)): Path<(Uuid, String)>,
) -> AppResult<Json<serde_json::Value>> {
    let msg = state
        .message_repo
        .find_by_id(msg_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    require_channel_access(&state, msg.channel_id, auth.user_id).await?;

    state
        .message_repo
        .remove_reaction(msg_id, auth.user_id, &emoji)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let _ = state
        .publisher
        .publish_reaction_removed(msg_id, msg.channel_id, auth.user_id, &emoji)
        .await;

    Ok(Json(serde_json::json!({ "status": "removed" })))
}

async fn mark_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<MarkReadRequest>,
) -> AppResult<Json<serde_json::Value>> {
    require_channel_access(&state, ch_id, auth.user_id).await?;
    state
        .message_repo
        .mark_read(ch_id, auth.user_id, req.message_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "status": "read" })))
}

async fn search_messages(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<SearchQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let query = params.q.clone().unwrap_or_default();
    if query.is_empty() {
        return Err(AppError::Validation("Search query is required".into()));
    }

    let is_member = state
        .workspace_service
        .is_workspace_member(params.workspace_id, auth.user_id)
        .await?;
    if !is_member {
        return Err(AppError::Forbidden("Not a member of this workspace".into()));
    }

    if let Some(ch_id) = params.channel_id {
        require_channel_access(&state, ch_id, auth.user_id).await?;
    }

    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0).max(0);

    let messages = state
        .message_repo
        .search(
            &query,
            params.workspace_id,
            params.channel_id,
            params.user_id,
            limit,
            offset,
        )
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Json(serde_json::json!({ "data": messages })))
}

async fn require_channel_access(
    state: &AppState,
    ch_id: Uuid,
    user_id: Uuid,
) -> AppResult<Channel> {
    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(ch_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;

    state
        .workspace_service
        .repo
        .get_member(channel.workspace_id, user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::Forbidden("Not a member of this workspace".into()))?;

    if channel.channel_type == ChannelType::Private || channel.channel_type == ChannelType::GroupDm
    {
        state
            .workspace_service
            .repo
            .get_channel_member(ch_id, user_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| AppError::Forbidden("Not a member of this channel".into()))?;
    }

    Ok(channel)
}

async fn link_attachments(
    state: &AppState,
    content: &str,
    message_id: Uuid,
    workspace_id: Uuid,
    user_id: Uuid,
) {
    let keys = extract_file_keys(content);
    if keys.is_empty() {
        return;
    }
    if let Err(e) = state
        .file_repo
        .link_to_message(&keys, message_id, workspace_id, user_id)
        .await
    {
        tracing::warn!(
            "failed to link attachments to message {}: {}",
            message_id,
            e
        );
    }
}

fn extract_file_keys(content: &str) -> Vec<String> {
    const MARKER: &str = "/api/files/download/";
    let mut keys = Vec::new();
    let mut rest = content;
    while let Some(pos) = rest.find(MARKER) {
        rest = &rest[pos + MARKER.len()..];
        let end = rest.find([')', ']', ' ', '"', '\n']).unwrap_or(rest.len());
        let key = &rest[..end];
        if !key.is_empty() {
            keys.push(key.to_string());
        }
        rest = &rest[end..];
    }
    keys
}

fn extract_mentioned_user_ids(content: &str) -> Vec<Uuid> {
    let mut ids = Vec::new();
    let mut remaining = content;
    while let Some(at_pos) = remaining.find("@[") {
        remaining = &remaining[at_pos + 2..];
        let Some(label_end) = remaining.find("](") else {
            break;
        };
        remaining = &remaining[label_end + 2..];
        let Some(id_end) = remaining.find(')') else {
            break;
        };
        let id_str = remaining[..id_end].trim();
        if let Ok(uuid) = Uuid::parse_str(id_str) {
            ids.push(uuid);
        }
        remaining = &remaining[id_end + 1..];
    }
    ids
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(dbe) if dbe.code().as_deref() == Some("23505"))
}
