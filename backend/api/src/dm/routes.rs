use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{middleware, Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation;

use super::models::*;
use crate::middleware::{auth_middleware, AuthUser};
use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/workspaces/:ws_id/dm", get(list_conversations))
        .route("/workspaces/:ws_id/dm/:user_id", get(list_messages))
        .route("/workspaces/:ws_id/dm/:user_id", post(send_message))
        .route("/workspaces/:ws_id/dm/:user_id/read", post(mark_read))
        .route(
            "/workspaces/:ws_id/dm/:user_id/:msg_id",
            patch(edit_message),
        )
        .route(
            "/workspaces/:ws_id/dm/:user_id/:msg_id",
            delete(delete_message),
        )
        .route(
            "/workspaces/:ws_id/dm/:user_id/:msg_id/reactions",
            post(add_reaction),
        )
        .route(
            "/workspaces/:ws_id/dm/:user_id/:msg_id/reactions/:emoji",
            delete(remove_reaction),
        )
        .layer(middleware::from_fn(auth_middleware))
        .with_state(state)
}

async fn require_workspace_member(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<()> {
    let is_member = state
        .workspace_service
        .is_workspace_member(workspace_id, user_id)
        .await?;
    if !is_member {
        return Err(AppError::Forbidden("Not a workspace member".into()));
    }
    Ok(())
}

async fn list_conversations(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;
    let convs = state
        .dm_repo
        .list_conversations(ws_id, auth.user_id)
        .await?;
    Ok(Json(serde_json::json!({ "data": convs })))
}

async fn list_messages(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, partner_id)): Path<(Uuid, Uuid)>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<serde_json::Value>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;
    require_workspace_member(&state, ws_id, partner_id).await?;

    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(1, 200);
    let before = params.get("before").and_then(|v| v.parse().ok());

    let messages = state
        .dm_repo
        .list_messages(ws_id, auth.user_id, partner_id, limit, before)
        .await?;

    let next_cursor = if messages.len() as i64 == limit {
        messages.last().map(|m| m.created_at.to_rfc3339())
    } else {
        None
    };

    let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();
    let reactions = state
        .dm_repo
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
                let msg_reactions = reactions_map.get(&msg.id).cloned().unwrap_or_default();
                obj.insert(
                    "reactions".to_string(),
                    serde_json::to_value(msg_reactions).unwrap_or_default(),
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
    Path((ws_id, partner_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<SendDmRequest>,
) -> AppResult<Json<DirectMessage>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;
    require_workspace_member(&state, ws_id, partner_id).await?;
    validation::validate_message_content(&req.content)?;

    let id = req.id.unwrap_or_else(Uuid::new_v4);
    let msg = match state
        .dm_repo
        .create(id, ws_id, auth.user_id, partner_id, &req.content)
        .await
    {
        Ok(msg) => msg,
        Err(ref e) if is_unique_violation(e) => state
            .dm_repo
            .get_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("DM ID conflict".into()))?,
        Err(e) => return Err(AppError::Database(e.to_string())),
    };

    let msg_json = serde_json::to_value(&msg).map_err(|e| AppError::Internal(e.to_string()))?;
    let _ = state.publisher.publish("dm.created", msg_json).await;

    Ok(Json(msg))
}

async fn edit_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, _partner_id, msg_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(req): Json<EditDmRequest>,
) -> AppResult<Json<DirectMessage>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;
    validation::validate_message_content(&req.content)?;

    let existing = state
        .dm_repo
        .get_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
    if existing.from_user_id != auth.user_id {
        return Err(AppError::Forbidden(
            "Can only edit your own messages".into(),
        ));
    }

    let msg = state.dm_repo.update(msg_id, &req.content).await?;

    let msg_json = serde_json::to_value(&msg).map_err(|e| AppError::Internal(e.to_string()))?;
    let _ = state.publisher.publish("dm.updated", msg_json).await;

    Ok(Json(msg))
}

async fn delete_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, _partner_id, msg_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;

    let existing = state
        .dm_repo
        .get_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
    if existing.from_user_id != auth.user_id {
        return Err(AppError::Forbidden(
            "Can only delete your own messages".into(),
        ));
    }

    let msg = state.dm_repo.soft_delete(msg_id).await?;

    let msg_json = serde_json::to_value(&msg).map_err(|e| AppError::Internal(e.to_string()))?;
    let _ = state.publisher.publish("dm.deleted", msg_json).await;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

async fn add_reaction(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, _partner_id, msg_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(req): Json<AddDmReactionRequest>,
) -> AppResult<Json<DmReaction>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;
    let dm = state
        .dm_repo
        .get_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    let reaction = state
        .dm_repo
        .add_reaction(msg_id, auth.user_id, &req.emoji)
        .await?;

    let payload = serde_json::json!({
        "message_id": msg_id,
        "workspace_id": ws_id,
        "from_user_id": dm.from_user_id,
        "to_user_id": dm.to_user_id,
        "reaction": reaction,
    });
    let _ = state.publisher.publish("dm.reaction.added", payload).await;

    Ok(Json(reaction))
}

async fn remove_reaction(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, _partner_id, msg_id, emoji)): Path<(Uuid, Uuid, Uuid, String)>,
) -> AppResult<Json<serde_json::Value>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;
    let dm = state
        .dm_repo
        .get_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    state
        .dm_repo
        .remove_reaction(msg_id, auth.user_id, &emoji)
        .await?;

    let payload = serde_json::json!({
        "message_id": msg_id,
        "workspace_id": ws_id,
        "from_user_id": dm.from_user_id,
        "to_user_id": dm.to_user_id,
        "user_id": auth.user_id,
        "emoji": emoji,
    });
    let _ = state
        .publisher
        .publish("dm.reaction.removed", payload)
        .await;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn mark_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, partner_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;
    state
        .dm_repo
        .mark_read(auth.user_id, ws_id, partner_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(dbe) if dbe.code().as_deref() == Some("23505"))
}
