use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use garde::Validate;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::pagination::PageQuery;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_messages, send_message))
        .routes(routes!(list_pins))
        .routes(routes!(mark_read))
        .routes(routes!(update_message, delete_message))
        .routes(routes!(message_history))
        .routes(routes!(pin_message, unpin_message))
        .routes(routes!(list_thread))
        .routes(routes!(list_reactions, add_reaction))
        .routes(routes!(remove_reaction))
        .routes(routes!(search_messages))
}

#[utoipa::path(
    operation_id = "messaging_list_messages",
    get, path = "/channels/{ch_id}/messages", tag = "messages",
    params(ListMessagesQuery),
    responses((status = 200, body = DataList<MessageWithReactions>)))]
async fn list_messages(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Query(params): Query<ListMessagesQuery>,
) -> AppResult<Json<DataList<MessageWithReactions>>> {
    authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let messages = state
        .message_repo
        .list_channel_messages(ch_id, limit, params.cursor)
        .await?;

    let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();
    let reactions = state
        .message_repo
        .list_reactions_for_messages(&message_ids)
        .await?;

    let mut reactions_map: std::collections::HashMap<Uuid, Vec<Reaction>> =
        std::collections::HashMap::new();
    for reaction in reactions {
        reactions_map
            .entry(reaction.message_id)
            .or_default()
            .push(reaction);
    }

    let data = messages
        .into_iter()
        .map(|message| {
            let reactions = reactions_map.remove(&message.id).unwrap_or_default();
            MessageWithReactions { message, reactions }
        })
        .collect();

    Ok(Json(DataList { data }))
}

/// Threads are one level deep and stay in their channel: replying to a reply
/// joins the root's thread, and a parent from elsewhere is refused rather than
/// letting a message hang off something its readers cannot see.
async fn thread_root(state: &AppState, channel_id: Uuid, parent_id: Uuid) -> AppResult<Uuid> {
    let parent = state
        .message_repo
        .find_by_id(parent_id)
        .await?
        .filter(|parent| parent.channel_id == channel_id)
        .ok_or_else(|| AppError::Validation("Thread parent is not in this channel".into()))?;
    Ok(parent.thread_parent_id.unwrap_or(parent.id))
}

#[utoipa::path(
    operation_id = "messaging_send_message",
    post, path = "/channels/{ch_id}/messages", tag = "messages",
    request_body = SendMessageRequest,
    responses((status = 200, body = Message)))]
async fn send_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<SendMessageRequest>,
) -> AppResult<Json<Message>> {
    req.validate()?;

    let channel = authz::require_channel_post(&state, ch_id, auth.user_id)
        .await?
        .channel;

    let thread_parent_id = match req.thread_parent_id {
        Some(parent_id) => Some(thread_root(&state, ch_id, parent_id).await?),
        None => None,
    };

    // The mention set has to exist before the insert: the unread and mention
    // counters are bumped inside the same transaction that writes the message.
    let mentioned_ids = expand_mentions(&state, ch_id, auth.user_id, &req.content).await;

    let mut tx = state.pool.begin().await?;
    let created = state
        .message_repo
        .create_message_in(
            &mut tx,
            NewMessage {
                channel_id: ch_id,
                user_id: auth.user_id,
                content: &req.content,
                thread_parent_id,
                client_message_id: req.client_message_id,
                mentioned: &mentioned_ids,
            },
        )
        .await;

    let (msg, staged) = match (created, req.client_message_id) {
        (Ok(msg), _) => {
            let staged = state
                .publisher
                .stage_message_created(&mut tx, &msg, channel.workspace_id, &mentioned_ids)
                .await?;
            tx.commit().await?;
            (msg, Some(staged))
        }
        // The only unique key a client controls is `(channel_id,
        // client_message_id)`, so a violation means this exact send already
        // landed — in this channel, by definition of the index.
        (Err(ref e), Some(client_id)) if shared_common::errors::is_unique_violation(e) => {
            drop(tx);
            let existing = state
                .message_repo
                .find_by_client_id(ch_id, client_id)
                .await?
                .ok_or_else(|| AppError::Conflict("Message id already in use".into()))?;
            (existing, None)
        }
        (Err(e), _) => return Err(AppError::Database(e.to_string())),
    };

    crate::files::service::link_to_channel_message(
        &state,
        &req.content,
        msg.id,
        channel.workspace_id,
        auth.user_id,
    )
    .await;

    if let Some(staged) = staged {
        state.publisher.dispatch(staged).await;
    }

    Ok(Json(msg))
}

#[utoipa::path(patch, path = "/messages/{msg_id}", tag = "messages",
    request_body = UpdateMessageRequest,
    responses((status = 200, body = Message)))]
async fn update_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
    Json(req): Json<UpdateMessageRequest>,
) -> AppResult<Json<Message>> {
    req.validate()?;

    let existing = state
        .message_repo
        .find_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    if existing.user_id != auth.user_id {
        return Err(AppError::Forbidden(
            "Can only edit your own messages".into(),
        ));
    }

    let channel = authz::find_channel(&state, existing.channel_id).await?;

    let mut tx = state.pool.begin().await?;
    let msg = state
        .message_repo
        .update_message_in(&mut tx, msg_id, &req.content, auth.user_id)
        .await?;
    let staged = state
        .publisher
        .stage_message_updated(&mut tx, &msg, channel.workspace_id)
        .await?;
    tx.commit().await?;

    crate::files::service::link_to_channel_message(
        &state,
        &req.content,
        msg.id,
        channel.workspace_id,
        auth.user_id,
    )
    .await;
    crate::files::service::release_unlinked_from_channel_message(&state, &req.content, msg.id)
        .await;

    state.publisher.dispatch(staged).await;

    Ok(Json(msg))
}

/// The author sees their own prior versions; a workspace admin sees anyone's,
/// and that read is audited. Everyone else gets the "edited" marker and nothing
/// behind it — showing every earlier draft to every reader is a different
/// product from the one people think they are using.
#[utoipa::path(
    operation_id = "messaging_message_history",
    get, path = "/messages/{msg_id}/history", tag = "messages",
    responses((status = 200, body = DataList<MessageEdit>)))]
async fn message_history(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(msg_id): Path<Uuid>,
) -> AppResult<Json<DataList<MessageEdit>>> {
    let message = state
        .message_repo
        .find_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
    let access = authz::require_channel_access(&state, message.channel_id, auth.user_id).await?;

    if message.user_id != auth.user_id {
        authz::require_workspace_role(
            &state,
            access.channel.workspace_id,
            auth.user_id,
            &WorkspaceRole::Admin,
        )
        .await?;
        audit::record(
            &state,
            AuditEntry::new(AuditAction::MessageHistoryRead, auth.user_id)
                .workspace(access.channel.workspace_id)
                .resource(msg_id)
                .ip(&ip)
                .details(serde_json::json!({ "author_id": message.user_id })),
        )
        .await;
    }

    let edits = state.message_repo.list_edits(msg_id).await?;
    Ok(Json(edits.into()))
}

#[utoipa::path(
    operation_id = "messaging_delete_message",
    delete, path = "/messages/{msg_id}", tag = "messages",
    responses((status = 200, body = StatusResponse)))]
async fn delete_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(msg_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    let existing = state
        .message_repo
        .find_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    if existing.user_id != auth.user_id {
        let can_mod = state
            .message_repo
            .can_moderate_channel(existing.channel_id, auth.user_id)
            .await?;
        if !can_mod {
            return Err(AppError::Forbidden(
                "Can only delete your own messages".into(),
            ));
        }
    }

    let channel = authz::find_channel(&state, existing.channel_id).await?;
    if let Err(e) = state
        .message_repo
        .drop_unread_for_message(existing.channel_id, msg_id, existing.user_id)
        .await
    {
        tracing::warn!(
            "failed to adjust unread counts for a deleted message: {}",
            e
        );
    }
    let mut tx = state.pool.begin().await?;
    state
        .message_repo
        .soft_delete_message_in(&mut tx, msg_id)
        .await?;
    let staged = state
        .publisher
        .stage_message_deleted(&mut tx, msg_id, existing.channel_id, channel.workspace_id)
        .await?;
    tx.commit().await?;

    crate::files::service::delete_for_channel_message(&state, msg_id).await;
    state.publisher.dispatch(staged).await;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::MessageDeleted, auth.user_id)
            .workspace(channel.workspace_id)
            .resource(msg_id)
            .ip(&ip)
            .details(serde_json::json!({
                "channel_id": existing.channel_id,
                "author_id": existing.user_id,
                "moderated": existing.user_id != auth.user_id,
            })),
    )
    .await;

    Ok(Json(StatusResponse { status: "deleted" }))
}

#[utoipa::path(post, path = "/messages/{msg_id}/pin", tag = "messages",
    responses((status = 200, body = StatusResponse)))]
async fn pin_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    let existing = state
        .message_repo
        .find_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    let can_mod = state
        .message_repo
        .can_moderate_channel(existing.channel_id, auth.user_id)
        .await?;
    if !can_mod {
        return Err(AppError::Forbidden(
            "Requires channel or workspace admin".into(),
        ));
    }

    let channel = authz::find_channel(&state, existing.channel_id).await?;
    let mut tx = state.pool.begin().await?;
    let msg = state
        .message_repo
        .set_pinned_in(&mut tx, msg_id, true)
        .await?;
    let staged = state
        .publisher
        .stage_message_pinned(&mut tx, msg_id, msg.channel_id, channel.workspace_id, true)
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;

    Ok(Json(StatusResponse { status: "pinned" }))
}

#[utoipa::path(delete, path = "/messages/{msg_id}/pin", tag = "messages",
    responses((status = 200, body = StatusResponse)))]
async fn unpin_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    let existing = state
        .message_repo
        .find_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    let can_mod = state
        .message_repo
        .can_moderate_channel(existing.channel_id, auth.user_id)
        .await?;
    if !can_mod {
        return Err(AppError::Forbidden(
            "Requires channel or workspace admin".into(),
        ));
    }

    let channel = authz::find_channel(&state, existing.channel_id).await?;
    let mut tx = state.pool.begin().await?;
    let msg = state
        .message_repo
        .set_pinned_in(&mut tx, msg_id, false)
        .await?;
    let staged = state
        .publisher
        .stage_message_pinned(&mut tx, msg_id, msg.channel_id, channel.workspace_id, false)
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;

    Ok(Json(StatusResponse { status: "unpinned" }))
}

#[utoipa::path(get, path = "/channels/{ch_id}/pins", tag = "messages",
    responses((status = 200, body = DataList<Message>)))]
async fn list_pins(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<DataList<Message>>> {
    authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    let pins = state.message_repo.list_pinned(ch_id).await?;
    Ok(Json(pins.into()))
}

#[utoipa::path(
    operation_id = "messaging_list_thread",
    get, path = "/messages/{msg_id}/thread", tag = "messages",
    params(PageQuery),
    responses((status = 200, body = DataList<Message>)))]
async fn list_thread(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
    Query(params): Query<PageQuery>,
) -> AppResult<Json<DataList<Message>>> {
    let channel_id = state
        .message_repo
        .find_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?
        .channel_id;
    authz::require_channel_access(&state, channel_id, auth.user_id).await?;
    let messages = state
        .message_repo
        .list_thread_messages(msg_id, params.limit(), params.offset())
        .await?;
    Ok(Json(messages.into()))
}

#[utoipa::path(get, path = "/messages/{msg_id}/reactions", tag = "messages",
    responses((status = 200, body = DataList<Reaction>)))]
async fn list_reactions(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
) -> AppResult<Json<DataList<Reaction>>> {
    let channel_id = state
        .message_repo
        .find_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?
        .channel_id;
    authz::require_channel_access(&state, channel_id, auth.user_id).await?;
    let reactions = state.message_repo.list_reactions(msg_id).await?;
    Ok(Json(reactions.into()))
}

#[utoipa::path(
    operation_id = "messaging_add_reaction",
    post, path = "/messages/{msg_id}/reactions", tag = "messages",
    request_body = AddReactionRequest,
    responses((status = 200, body = Reaction)))]
async fn add_reaction(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(msg_id): Path<Uuid>,
    Json(req): Json<AddReactionRequest>,
) -> AppResult<Json<Reaction>> {
    req.validate()?;
    let msg = state
        .message_repo
        .find_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    let access = authz::require_channel_access(&state, msg.channel_id, auth.user_id).await?;

    let mut tx = state.pool.begin().await?;
    let reaction = state
        .message_repo
        .add_reaction_in(&mut tx, msg_id, auth.user_id, &req.emoji)
        .await
        .map_err(|e| {
            if shared_common::errors::is_unique_violation(&e) {
                AppError::Conflict("You already reacted with this emoji".into())
            } else {
                AppError::Database(e.to_string())
            }
        })?;
    let staged = state
        .publisher
        .stage_reaction_added(
            &mut tx,
            &reaction,
            msg.channel_id,
            access.channel.workspace_id,
        )
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;

    Ok(Json(reaction))
}

#[utoipa::path(
    operation_id = "messaging_remove_reaction",
    delete, path = "/messages/{msg_id}/reactions/{emoji}", tag = "messages",
    responses((status = 200, body = StatusResponse)))]
async fn remove_reaction(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((msg_id, emoji)): Path<(Uuid, String)>,
) -> AppResult<Json<StatusResponse>> {
    let msg = state
        .message_repo
        .find_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;

    let access = authz::require_channel_access(&state, msg.channel_id, auth.user_id).await?;

    let mut tx = state.pool.begin().await?;
    state
        .message_repo
        .remove_reaction_in(&mut tx, msg_id, auth.user_id, &emoji)
        .await?;
    let staged = state
        .publisher
        .stage_reaction_removed(
            &mut tx,
            msg_id,
            msg.channel_id,
            access.channel.workspace_id,
            auth.user_id,
            &emoji,
        )
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;

    Ok(Json(StatusResponse { status: "removed" }))
}

#[utoipa::path(
    operation_id = "messaging_mark_read",
    post, path = "/channels/{ch_id}/read", tag = "messages",
    request_body = MarkReadRequest,
    responses((status = 200, body = StatusResponse)))]
async fn mark_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<MarkReadRequest>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    state
        .message_repo
        .mark_read(ch_id, auth.user_id, req.message_id)
        .await?;
    Ok(Json(StatusResponse { status: "read" }))
}

#[utoipa::path(get, path = "/search", tag = "messages",
    params(SearchQuery),
    responses((status = 200, body = SearchResponse)))]
async fn search_messages(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<SearchQuery>,
) -> AppResult<Json<SearchResponse>> {
    let query = params.q.clone().unwrap_or_default();
    if query.is_empty() {
        return Err(AppError::Validation("Search query is required".into()));
    }

    let member = authz::require_workspace_member(&state, params.workspace_id, auth.user_id).await?;

    if let Some(ch_id) = params.channel_id {
        authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    }

    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let offset = params.offset.unwrap_or(0).max(0);

    // Asking for one channel is asking about channels, whatever the scope says.
    let scope = match params.channel_id {
        Some(_) => SearchScope::Channels,
        None => params.scope.unwrap_or(SearchScope::All),
    };

    let messages = state
        .message_repo
        .search(crate::messaging::repo::MessageSearch {
            query: &query,
            workspace_id: params.workspace_id,
            requester_id: auth.user_id,
            requester_is_guest: member.role == WorkspaceRole::Guest,
            channel_id: params.channel_id,
            author_id: params.user_id,
            scope,
            limit,
            offset,
        })
        .await?;

    Ok(Json(SearchResponse { data: messages }))
}

#[derive(Debug, Default, PartialEq)]
struct BroadcastMentions {
    channel: bool,
    here: bool,
}

impl BroadcastMentions {
    fn any(&self) -> bool {
        self.channel || self.here
    }
}

fn extract_broadcast_mentions(content: &str) -> BroadcastMentions {
    let mut found = BroadcastMentions::default();
    for target in mention_targets(content) {
        match target.as_str() {
            "channel" | "everyone" => found.channel = true,
            "here" => found.here = true,
            _ => {}
        }
    }
    for word in ["channel", "everyone", "here"] {
        if !contains_bare_mention(content, word) {
            continue;
        }
        if word == "here" {
            found.here = true;
        } else {
            found.channel = true;
        }
    }
    found
}

fn contains_bare_mention(content: &str, word: &str) -> bool {
    let needle = format!("@{word}");
    let mut from = 0;
    while let Some(offset) = content[from..].find(&needle) {
        let start = from + offset;
        let before_ok = start == 0
            || matches!(
                content[..start].chars().next_back(),
                Some(' ') | Some('\n') | Some('(')
            );
        let after = content[start + needle.len()..].chars().next();
        let after_ok = !matches!(after, Some(c) if c.is_alphanumeric() || c == '_');
        if before_ok && after_ok {
            return true;
        }
        from = start + needle.len();
    }
    false
}

pub(crate) async fn expand_mentions(
    state: &AppState,
    channel_id: Uuid,
    author_id: Uuid,
    content: &str,
) -> Vec<Uuid> {
    let mut mentioned: Vec<Uuid> = extract_mentioned_user_ids(content);
    let broadcast = extract_broadcast_mentions(content);
    let groups = extract_group_mentions(content);
    let here_only = broadcast.here && !broadcast.channel;

    let workspace_id = if here_only || !groups.is_empty() {
        state
            .workspace_service
            .repo
            .find_channel_by_id(channel_id)
            .await
            .ok()
            .flatten()
            .map(|c| c.workspace_id)
    } else {
        None
    };

    if broadcast.any() {
        let members = state
            .message_repo
            .list_channel_member_ids(channel_id)
            .await
            .unwrap_or_default();

        let online = match (here_only, workspace_id) {
            (true, Some(workspace_id)) => {
                let mut conn = state.redis.clone();
                Some(crate::presence::online_user_ids(&mut conn, workspace_id).await)
            }
            (true, None) => Some(std::collections::HashSet::new()),
            (false, _) => None,
        };

        for member in members {
            let reachable = online.as_ref().is_none_or(|set| set.contains(&member));
            if reachable {
                mentioned.push(member);
            }
        }
    }

    // A group is a shorthand for people, and it reaches them the same way
    // `@channel` does: through the channel. Notifying a member who is not in the
    // channel would tell them a private channel exists and hand them a preview
    // of a message they cannot open.
    if !groups.is_empty() {
        if let Some(workspace_id) = workspace_id {
            let in_group = state
                .group_repo
                .member_ids_for_groups(workspace_id, &groups)
                .await
                .unwrap_or_default();

            if !in_group.is_empty() {
                let in_channel: std::collections::HashSet<Uuid> = state
                    .message_repo
                    .list_channel_member_ids(channel_id)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .collect();

                mentioned.extend(in_group.into_iter().filter(|id| in_channel.contains(id)));
            }
        }
    }

    mentioned.retain(|id| *id != author_id);
    mentioned.sort();
    mentioned.dedup();
    mentioned
}

fn mention_targets(content: &str) -> Vec<String> {
    let mut targets = Vec::new();
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
        targets.push(remaining[..id_end].trim().to_string());
        remaining = &remaining[id_end + 1..];
    }
    targets
}

/// Group mentions carry `group:<uuid>` where a user mention carries a bare
/// uuid, so the two parse out of the same `@[label](target)` form without either
/// one having to know about the other.
fn extract_group_mentions(content: &str) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = mention_targets(content)
        .into_iter()
        .filter_map(|target| {
            target
                .strip_prefix("group:")
                .and_then(|id| Uuid::parse_str(id).ok())
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_mentions_are_recognised_by_their_target() {
        let channel = extract_broadcast_mentions("heads up @[channel](channel)");
        assert_eq!(
            channel,
            BroadcastMentions {
                channel: true,
                here: false
            }
        );

        let everyone = extract_broadcast_mentions("@[everyone](everyone) ship it");
        assert!(everyone.channel, "@everyone reaches the whole channel");

        let here = extract_broadcast_mentions("@[here](here) quick question");
        assert_eq!(
            here,
            BroadcastMentions {
                channel: false,
                here: true
            }
        );
    }

    #[test]
    fn a_typed_broadcast_counts_even_without_picking_it_from_the_dropdown() {
        assert!(extract_broadcast_mentions("@channel standup in 5").channel);
        assert!(extract_broadcast_mentions("ping @here please").here);
        assert!(extract_broadcast_mentions("(@everyone)").channel);
    }

    #[test]
    fn lookalikes_and_user_mentions_are_not_broadcasts() {
        let user = format!("@[Alice]({})", Uuid::new_v4());
        assert!(!extract_broadcast_mentions(&user).any());
        assert!(!extract_broadcast_mentions("email me at here@example.com").any());
        assert!(!extract_broadcast_mentions("we have many @channels").any());
        assert!(!extract_broadcast_mentions("mail@hereafter.io").any());
    }
}
