use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation;

use super::models::*;
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub const MAX_GROUP_PARTICIPANTS: usize = 9;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/workspaces/:ws_id/conversations", get(list_conversations))
        .route(
            "/workspaces/:ws_id/conversations",
            post(create_conversation),
        )
        .route("/conversations/:conv_id/messages", get(list_messages))
        .route("/conversations/:conv_id/messages", post(send_message))
        .route("/conversations/:conv_id/read", post(mark_read))
        .route("/conversations/messages/:msg_id", patch(edit_message))
        .route("/conversations/messages/:msg_id", delete(delete_message))
        .route(
            "/conversations/messages/:msg_id/reactions",
            post(add_reaction),
        )
        .route(
            "/conversations/messages/:msg_id/reactions/:emoji",
            delete(remove_reaction),
        );

    crate::protected(state, routes)
}

async fn publish_conversation_event(
    state: &AppState,
    event: &str,
    conversation_id: Uuid,
    workspace_id: Uuid,
    mut payload: serde_json::Value,
) {
    let participants = state
        .conversation_repo
        .participant_ids(conversation_id)
        .await
        .unwrap_or_default();
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("conversation_id".into(), serde_json::json!(conversation_id));
        obj.insert("workspace_id".into(), serde_json::json!(workspace_id));
        obj.insert("participant_ids".into(), serde_json::json!(participants));
    }
    let _ = state.publisher.publish(event, payload).await;
}

async fn list_conversations(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let conversations = state
        .conversation_repo
        .list_for_user(ws_id, auth.user_id)
        .await?;
    Ok(Json(serde_json::json!({ "data": conversations })))
}

async fn create_conversation(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateConversationRequest>,
) -> AppResult<Json<Conversation>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let mut participants: Vec<Uuid> = req
        .participant_ids
        .into_iter()
        .filter(|id| *id != auth.user_id)
        .collect();
    participants.sort();
    participants.dedup();

    if participants.is_empty() {
        return Err(AppError::Validation(
            "A conversation needs at least one other participant".into(),
        ));
    }
    if participants.len() + 1 > MAX_GROUP_PARTICIPANTS {
        return Err(AppError::Validation(format!(
            "A conversation holds at most {MAX_GROUP_PARTICIPANTS} people"
        )));
    }
    for participant in &participants {
        authz::require_workspace_member(&state, ws_id, *participant).await?;
    }

    if participants.len() == 1 {
        if let Some(existing) = state
            .conversation_repo
            .find_direct(ws_id, auth.user_id, participants[0])
            .await?
        {
            return Ok(Json(existing));
        }
    }

    let kind = if participants.len() == 1 {
        ConversationKind::Direct
    } else {
        ConversationKind::Group
    };

    let mut everyone = participants.clone();
    everyone.push(auth.user_id);

    let conversation = state
        .conversation_repo
        .create(ws_id, kind, auth.user_id, &everyone)
        .await?;

    publish_conversation_event(
        &state,
        "conversation.created",
        conversation.id,
        ws_id,
        serde_json::to_value(&conversation).unwrap_or_default(),
    )
    .await;

    Ok(Json(conversation))
}

async fn list_messages(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(conv_id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_conversation_participant(&state, conv_id, auth.user_id).await?;

    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(1, 200);
    let before = params.get("before").and_then(|v| v.parse().ok());

    let messages = state
        .conversation_repo
        .list_messages(conv_id, limit, before)
        .await?;

    let next_cursor = if messages.len() as i64 == limit {
        messages.last().map(|m| m.id.to_string())
    } else {
        None
    };

    let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();
    let reactions = state
        .conversation_repo
        .list_reactions_for_messages(&message_ids)
        .await?;
    let mut reactions_map: std::collections::HashMap<Uuid, Vec<_>> =
        std::collections::HashMap::new();
    for reaction in reactions {
        reactions_map
            .entry(reaction.message_id)
            .or_default()
            .push(reaction);
    }

    let data: Vec<serde_json::Value> = messages
        .into_iter()
        .map(|msg| {
            let mut msg_json = serde_json::to_value(&msg).unwrap_or_default();
            if let Some(obj) = msg_json.as_object_mut() {
                obj.insert(
                    "reactions".to_string(),
                    serde_json::to_value(reactions_map.get(&msg.id).cloned().unwrap_or_default())
                        .unwrap_or_default(),
                );
            }
            msg_json
        })
        .collect();

    Ok(Json(
        serde_json::json!({ "data": data, "next_cursor": next_cursor }),
    ))
}

async fn send_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(conv_id): Path<Uuid>,
    Json(req): Json<SendConversationMessageRequest>,
) -> AppResult<Json<ConversationMessage>> {
    let conversation =
        authz::require_conversation_participant(&state, conv_id, auth.user_id).await?;
    validation::validate_message_content(&req.content)?;

    if let Some(client_id) = req.client_message_id {
        validation::validate_client_message_id(client_id)?;
    }

    let message = match state
        .conversation_repo
        .create_message(
            Uuid::new_v4(),
            conv_id,
            auth.user_id,
            &req.content,
            req.client_message_id,
        )
        .await
    {
        Ok(msg) => msg,
        // The only unique key a client controls is `(conversation_id,
        // client_message_id)`, so a violation means this exact send already
        // landed — in this conversation, by definition of the index.
        Err(ref e) if shared_common::errors::is_unique_violation(e) => {
            let client_id = req
                .client_message_id
                .ok_or_else(|| AppError::Database(e.to_string()))?;
            state
                .conversation_repo
                .find_by_client_id(conv_id, client_id)
                .await?
                .ok_or_else(|| AppError::Conflict("Message id already in use".into()))?
        }
        Err(e) => return Err(AppError::Database(e.to_string())),
    };

    crate::files::service::link_to_conversation_message(
        &state,
        &req.content,
        message.id,
        conversation.workspace_id,
        auth.user_id,
    )
    .await;

    publish_conversation_event(
        &state,
        "conversation.message.created",
        conv_id,
        conversation.workspace_id,
        serde_json::to_value(&message).unwrap_or_default(),
    )
    .await;

    Ok(Json(message))
}

async fn edit_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
    Json(req): Json<EditConversationMessageRequest>,
) -> AppResult<Json<ConversationMessage>> {
    validation::validate_message_content(&req.content)?;
    let existing = state
        .conversation_repo
        .find_message(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
    let conversation =
        authz::require_conversation_participant(&state, existing.conversation_id, auth.user_id)
            .await?;

    if existing.user_id != auth.user_id {
        return Err(AppError::Forbidden(
            "Can only edit your own messages".into(),
        ));
    }

    let message = state
        .conversation_repo
        .update_message(msg_id, &req.content)
        .await?;

    crate::files::service::link_to_conversation_message(
        &state,
        &req.content,
        message.id,
        conversation.workspace_id,
        auth.user_id,
    )
    .await;
    crate::files::service::release_unlinked_from_conversation_message(
        &state,
        &req.content,
        message.id,
    )
    .await;

    publish_conversation_event(
        &state,
        "conversation.message.updated",
        conversation.id,
        conversation.workspace_id,
        serde_json::to_value(&message).unwrap_or_default(),
    )
    .await;

    Ok(Json(message))
}

async fn delete_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let existing = state
        .conversation_repo
        .find_message(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
    let conversation =
        authz::require_conversation_participant(&state, existing.conversation_id, auth.user_id)
            .await?;

    if existing.user_id != auth.user_id {
        return Err(AppError::Forbidden(
            "Can only delete your own messages".into(),
        ));
    }

    let message = state.conversation_repo.soft_delete_message(msg_id).await?;
    crate::files::service::delete_for_conversation_message(&state, msg_id).await;

    publish_conversation_event(
        &state,
        "conversation.message.deleted",
        conversation.id,
        conversation.workspace_id,
        serde_json::to_value(&message).unwrap_or_default(),
    )
    .await;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

async fn add_reaction(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
    Json(req): Json<AddConversationReactionRequest>,
) -> AppResult<Json<ConversationReaction>> {
    validation::validate_reaction_emoji(&req.emoji)?;
    let existing = state
        .conversation_repo
        .find_message(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
    let conversation =
        authz::require_conversation_participant(&state, existing.conversation_id, auth.user_id)
            .await?;

    let reaction = state
        .conversation_repo
        .add_reaction(msg_id, auth.user_id, &req.emoji)
        .await
        .map_err(|e| {
            if shared_common::errors::is_unique_violation(&e) {
                AppError::Conflict("You already reacted with this emoji".into())
            } else {
                AppError::Database(e.to_string())
            }
        })?;

    publish_conversation_event(
        &state,
        "conversation.reaction.added",
        conversation.id,
        conversation.workspace_id,
        serde_json::json!({ "message_id": msg_id, "reaction": reaction }),
    )
    .await;

    Ok(Json(reaction))
}

async fn remove_reaction(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((msg_id, emoji)): Path<(Uuid, String)>,
) -> AppResult<Json<serde_json::Value>> {
    let existing = state
        .conversation_repo
        .find_message(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
    let conversation =
        authz::require_conversation_participant(&state, existing.conversation_id, auth.user_id)
            .await?;

    state
        .conversation_repo
        .remove_reaction(msg_id, auth.user_id, &emoji)
        .await?;

    publish_conversation_event(
        &state,
        "conversation.reaction.removed",
        conversation.id,
        conversation.workspace_id,
        serde_json::json!({ "message_id": msg_id, "user_id": auth.user_id, "emoji": emoji }),
    )
    .await;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn mark_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(conv_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_conversation_participant(&state, conv_id, auth.user_id).await?;
    state
        .conversation_repo
        .mark_read(conv_id, auth.user_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}
