use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{ThreadPage, ThreadState, ThreadSummary, ThreadsQuery, UnreadThreads};
use crate::authz;
use crate::messaging::models::Message;
use crate::middleware::AuthUser;
use crate::state::AppState;

const LATEST_REPLIES: i64 = 3;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_threads))
        .routes(routes!(unread_threads))
        .routes(routes!(mark_thread_read))
        .routes(routes!(follow_thread, unfollow_thread))
}

pub async fn announce(state: &AppState, user_id: Uuid, workspace_id: Uuid, thread: &ThreadState) {
    let payload = serde_json::json!({
        "user_id": user_id,
        "workspace_id": workspace_id,
        "root_message_id": thread.root_message_id,
        "following": thread.following,
        "unread_count": thread.unread_count,
    });
    if let Err(e) = state.publisher.publish("thread.read_state", payload).await {
        tracing::warn!(root_message_id = %thread.root_message_id, "failed to announce a thread's read state: {e}");
    }
}

pub async fn announce_reply(state: &AppState, root_message_id: Uuid, workspace_id: Uuid) {
    let followers = match state.thread_repo.follower_states(root_message_id).await {
        Ok(followers) => followers,
        Err(e) => {
            tracing::warn!(%root_message_id, "failed to read a thread's followers: {e}");
            return;
        }
    };
    for follower in followers {
        let thread = ThreadState {
            root_message_id,
            following: follower.following,
            unread_count: follower.unread_count,
        };
        announce(state, follower.user_id, workspace_id, &thread).await;
    }
}

async fn thread_root(
    state: &AppState,
    message_id: Uuid,
    user_id: Uuid,
) -> AppResult<(Message, Uuid)> {
    let message = state
        .message_repo
        .find_root(message_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Thread not found".into()))?;
    let root = match message.thread_parent_id {
        Some(parent) => state
            .message_repo
            .find_root(parent)
            .await?
            .ok_or_else(|| AppError::NotFound("Thread not found".into()))?,
        None => message,
    };
    let access = authz::require_channel_access(state, root.channel_id, user_id).await?;
    Ok((root, access.channel.workspace_id))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/threads", tag = "threads",
    params(ThreadsQuery),
    responses((status = 200, body = ThreadPage)))]
async fn list_threads(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<ThreadsQuery>,
) -> AppResult<Json<ThreadPage>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let followed = state
        .thread_repo
        .list_followed(auth.user_id, ws_id, query.cursor, limit)
        .await?;
    let root_ids: Vec<Uuid> = followed.iter().map(|t| t.root_message_id).collect();
    let mut roots: HashMap<Uuid, Message> = state
        .thread_repo
        .messages_by_ids(&root_ids)
        .await?
        .into_iter()
        .map(|m| (m.id, m))
        .collect();
    let mut replies: HashMap<Uuid, Vec<Message>> = HashMap::new();
    for reply in state
        .thread_repo
        .latest_replies(&root_ids, LATEST_REPLIES)
        .await?
    {
        if let Some(root) = reply.thread_parent_id {
            replies.entry(root).or_default().push(reply);
        }
    }
    let next_cursor = (followed.len() as i64 == limit)
        .then(|| followed.last().map(|t| t.last_reply_at))
        .flatten();
    let data = followed
        .into_iter()
        .filter_map(|thread| {
            Some(ThreadSummary {
                root: roots.remove(&thread.root_message_id)?,
                latest_replies: replies.remove(&thread.root_message_id).unwrap_or_default(),
                unread_count: thread.unread_count,
                last_read_at: thread.last_read_at,
                last_reply_at: thread.last_reply_at,
            })
        })
        .collect();
    Ok(Json(ThreadPage { data, next_cursor }))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/threads/unread", tag = "threads",
    responses((status = 200, body = UnreadThreads)))]
async fn unread_threads(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<UnreadThreads>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let count = state
        .thread_repo
        .unread_thread_count(auth.user_id, ws_id)
        .await?;
    Ok(Json(UnreadThreads { count }))
}

#[utoipa::path(post, path = "/threads/{root_id}/read", tag = "threads",
    responses((status = 200, body = ThreadState)))]
async fn mark_thread_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(root_id): Path<Uuid>,
) -> AppResult<Json<ThreadState>> {
    let (root, workspace_id) = thread_root(&state, root_id, auth.user_id).await?;
    let thread = match state.thread_repo.mark_read(auth.user_id, root.id).await? {
        Some(thread) => {
            announce(&state, auth.user_id, workspace_id, &thread).await;
            thread
        }
        None => ThreadState {
            root_message_id: root.id,
            following: false,
            unread_count: 0,
        },
    };
    Ok(Json(thread))
}

async fn set_following(
    state: &AppState,
    user_id: Uuid,
    root_id: Uuid,
    following: bool,
) -> AppResult<Json<ThreadState>> {
    let (root, workspace_id) = thread_root(state, root_id, user_id).await?;
    let thread = state
        .thread_repo
        .set_following(user_id, workspace_id, root.id, following)
        .await?;
    announce(state, user_id, workspace_id, &thread).await;
    Ok(Json(thread))
}

#[utoipa::path(put, path = "/threads/{root_id}/follow", tag = "threads",
    responses((status = 200, body = ThreadState)))]
async fn follow_thread(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(root_id): Path<Uuid>,
) -> AppResult<Json<ThreadState>> {
    set_following(&state, auth.user_id, root_id, true).await
}

#[utoipa::path(delete, path = "/threads/{root_id}/follow", tag = "threads",
    responses((status = 200, body = ThreadState)))]
async fn unfollow_thread(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(root_id): Path<Uuid>,
) -> AppResult<Json<ThreadState>> {
    set_following(&state, auth.user_id, root_id, false).await
}
