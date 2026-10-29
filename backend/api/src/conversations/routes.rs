use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use crate::authz;
use crate::bots::{models::Scope, Caller};
use crate::dto::DataList;
use crate::messaging::publisher::{EventPublisher, Staged};
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::{WorkspaceMember, WorkspaceRole};

pub const MAX_GROUP_PARTICIPANTS: usize = 9;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(list_conversations, create_conversation))
}

async fn stage_conversation_event(
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
    EventPublisher::stage(conn, event, workspace_id, payload).await
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

#[utoipa::path(post, path = "/workspaces/{ws_id}/conversations", tags = ["conversations", "bot"], request_body = CreateConversationRequest, security(("bearer" = []), ("bot" = ["im:write"])), responses((status = 200, body = Conversation)))]
async fn create_conversation(
    State(state): State<Arc<AppState>>,
    caller: Caller,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateConversationRequest>,
) -> AppResult<Json<Conversation>> {
    let auth = caller.scoped(Scope::ImWrite)?;
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
        require_reachable(&state, &starter, *participant).await?;
    }

    if participants.len() == 1 {
        return Ok(Json(
            open_direct(&state, ws_id, auth.user_id, participants[0]).await?,
        ));
    }

    let mut everyone = participants.clone();
    everyone.push(auth.user_id);
    Ok(Json(
        open(
            &state,
            ws_id,
            ConversationKind::Group,
            auth.user_id,
            &everyone,
        )
        .await?,
    ))
}

pub(crate) async fn require_reachable(
    state: &AppState,
    starter: &WorkspaceMember,
    other: Uuid,
) -> AppResult<()> {
    // Deliberately the same refusal whether the id is unknown, belongs to
    // another workspace, or is simply out of a guest's reach. Three
    // different answers here would rebuild the directory CS-041 closed.
    let reachable = match authz::require_workspace_member(state, starter.workspace_id, other).await
    {
        Ok(_) if starter.role != WorkspaceRole::Guest => true,
        Ok(_) => {
            state
                .workspace_service
                .repo
                .share_a_channel(starter.user_id, other)
                .await?
        }
        Err(_) => false,
    };

    if !reachable {
        return Err(AppError::Forbidden(
            "You cannot start a conversation with that person".into(),
        ));
    }
    Ok(())
}

pub(crate) async fn open_direct(
    state: &AppState,
    ws_id: Uuid,
    starter: Uuid,
    other: Uuid,
) -> AppResult<Conversation> {
    if let Some(existing) = state
        .conversation_repo
        .find_direct(ws_id, starter, other)
        .await?
    {
        return Ok(existing);
    }
    open(
        state,
        ws_id,
        ConversationKind::Direct,
        starter,
        &[other, starter],
    )
    .await
}

async fn open(
    state: &AppState,
    ws_id: Uuid,
    kind: ConversationKind,
    starter: Uuid,
    everyone: &[Uuid],
) -> AppResult<Conversation> {
    let mut tx = state.pool.begin().await?;
    let conversation = state
        .conversation_repo
        .create_in(&mut tx, ws_id, kind, starter, everyone)
        .await?;
    let staged = stage_conversation_event(
        &mut tx,
        "conversation.created",
        conversation.id,
        ws_id,
        everyone,
        serde_json::to_value(&conversation).unwrap_or_default(),
    )
    .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(conversation)
}
