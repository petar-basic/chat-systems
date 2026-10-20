use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use crate::authz;
use crate::dto::{DataList, StatusResponse};
use crate::messaging::publisher::Staged;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub const MAX_GROUP_PARTICIPANTS: usize = 9;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_conversations, create_conversation))
        .routes(routes!(mark_read))
}

async fn stage_conversation_event(
    state: &AppState,
    conn: &mut sqlx::PgConnection,
    event: &str,
    conversation_id: Uuid,
    workspace_id: Uuid,
    participants: &[Uuid],
    mut payload: serde_json::Value,
) -> AppResult<Staged> {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("conversation_id".into(), serde_json::json!(conversation_id));
        obj.insert("workspace_id".into(), serde_json::json!(workspace_id));
        obj.insert("participant_ids".into(), serde_json::json!(participants));
    }
    state
        .publisher
        .stage(conn, event, workspace_id, payload)
        .await
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/conversations", tag = "conversations", responses((status = 200, body = DataList<ConversationSummary>)))]
async fn list_conversations(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<ConversationSummary>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let conversations = state
        .conversation_repo
        .list_for_user(ws_id, auth.user_id)
        .await?;
    Ok(Json(conversations.into()))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/conversations", tag = "conversations", request_body = CreateConversationRequest, responses((status = 200, body = Conversation)))]
async fn create_conversation(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateConversationRequest>,
) -> AppResult<Json<Conversation>> {
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
    let starter = authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    for participant in &participants {
        // Deliberately the same refusal whether the id is unknown, belongs to
        // another workspace, or is simply out of a guest's reach. Three
        // different answers here would rebuild the directory CS-041 closed.
        let reachable = match authz::require_workspace_member(&state, ws_id, *participant).await {
            Ok(_) if starter.role != WorkspaceRole::Guest => true,
            Ok(_) => {
                state
                    .workspace_service
                    .repo
                    .share_a_channel(auth.user_id, *participant)
                    .await?
            }
            Err(_) => false,
        };

        if !reachable {
            return Err(AppError::Forbidden(
                "You cannot start a conversation with that person".into(),
            ));
        }
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

    let mut tx = state.pool.begin().await?;
    let conversation = state
        .conversation_repo
        .create_in(&mut tx, ws_id, kind, auth.user_id, &everyone)
        .await?;
    let staged = stage_conversation_event(
        &state,
        &mut tx,
        "conversation.created",
        conversation.id,
        ws_id,
        &everyone,
        serde_json::to_value(&conversation).unwrap_or_default(),
    )
    .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;

    Ok(Json(conversation))
}

#[utoipa::path(
    operation_id = "conversations_mark_read",
    post, path = "/conversations/{conv_id}/read", tag = "conversations", responses((status = 200, body = StatusResponse)))]
async fn mark_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(conv_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_channel_access(&state, conv_id, auth.user_id).await?;
    state
        .conversation_repo
        .mark_read(conv_id, auth.user_id)
        .await?;
    Ok(Json(StatusResponse::new("ok")))
}
