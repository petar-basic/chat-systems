use std::sync::Arc;

use chrono::Utc;
use sqlx::PgConnection;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{CreatePollRequest, Poll, PollOptionCount, PollPlace, PollSummary};
use crate::authz;
use crate::messaging::models::{Message, NewMessage};
use crate::messaging::routes::{expand_mentions, mentions_everyone, thread_root};
use crate::state::AppState;
use crate::threads::{self, repo::ReplyFollows};

const DUE_BATCH: i64 = 100;

pub async fn create(
    state: &AppState,
    user_id: Uuid,
    channel_id: Uuid,
    req: &CreatePollRequest,
) -> AppResult<Message> {
    let draft = req.draft(Utc::now())?;
    let channel = authz::require_channel_post(state, channel_id, user_id)
        .await?
        .channel;
    if mentions_everyone(&draft.question) {
        authz::require_broadcast(state, &channel, user_id).await?;
    }
    let root = match req.thread_parent_id {
        Some(parent_id) => Some(thread_root(state, channel_id, parent_id).await?),
        None => None,
    };
    let mentioned = expand_mentions(state, channel_id, user_id, &draft.question).await;

    let mut tx = state.pool.begin().await?;
    let msg = state
        .message_repo
        .create_message_in(
            &mut tx,
            NewMessage {
                channel_id,
                user_id,
                content: &draft.question,
                thread_parent_id: root.as_ref().map(|root| root.id),
                also_in_channel: false,
                forwarded_from: None,
                client_message_id: None,
                mentioned: &mentioned,
            },
        )
        .await?;
    let poll = state
        .poll_repo
        .create_in(&mut tx, msg.id, user_id, &draft)
        .await?;
    let msg = write_summary(state, &mut tx, &poll).await?.0;
    if let Some(root) = &root {
        state
            .thread_repo
            .follow_on_reply_in(
                &mut tx,
                ReplyFollows {
                    root_message_id: root.id,
                    workspace_id: channel.workspace_id,
                    replier_id: user_id,
                    root_author_id: root.user_id,
                    mentioned: &mentioned,
                    replied_at: msg.created_at,
                },
            )
            .await?;
    }
    let staged = state
        .publisher
        .stage_message_created(&mut tx, &msg, channel.workspace_id, &mentioned)
        .await?;
    tx.commit().await?;

    state.publisher.dispatch(staged).await;
    if let Some(root) = &root {
        threads::routes::announce_reply(state, root.id, channel.workspace_id).await;
    }
    Ok(msg)
}

pub async fn find_readable(state: &AppState, poll_id: Uuid, user_id: Uuid) -> AppResult<PollPlace> {
    let place = state
        .poll_repo
        .find(poll_id)
        .await?
        .filter(|place| !place.message_deleted)
        .ok_or_else(|| AppError::NotFound("Poll not found".into()))?;
    authz::require_channel_access(state, place.channel_id, user_id).await?;
    Ok(place)
}

pub async fn vote(
    state: &AppState,
    user_id: Uuid,
    poll_id: Uuid,
    option_ids: &[Uuid],
) -> AppResult<(Vec<Uuid>, PollSummary)> {
    let place = find_readable(state, poll_id, user_id).await?;
    authz::require_not_archived(&authz::find_channel(state, place.channel_id).await?)?;

    let mut tx = state.pool.begin().await?;
    let poll = state.poll_repo.lock_in(&mut tx, poll_id).await?;
    if poll.is_closed(Utc::now()) {
        return Err(AppError::Conflict("This poll is closed".into()));
    }
    let offered = state.poll_repo.option_ids_in(&mut tx, poll_id).await?;
    let chosen: Vec<Uuid> = offered
        .iter()
        .copied()
        .filter(|id| option_ids.contains(id))
        .collect();
    if option_ids.iter().any(|id| !offered.contains(id)) {
        return Err(AppError::Validation(
            "That option is not in this poll".into(),
        ));
    }
    if !poll.multiple_choice && chosen.len() > 1 {
        return Err(AppError::Validation("This poll takes one answer".into()));
    }
    state
        .poll_repo
        .replace_vote_in(&mut tx, &poll, user_id, &chosen)
        .await?;
    let (msg, summary) = write_summary(state, &mut tx, &poll).await?;
    let staged = state
        .publisher
        .stage_message_updated(&mut tx, &msg, place.workspace_id)
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok((chosen, summary))
}

pub async fn close_by(state: &AppState, user_id: Uuid, poll_id: Uuid) -> AppResult<PollSummary> {
    let place = state
        .poll_repo
        .find(poll_id)
        .await?
        .filter(|place| !place.message_deleted)
        .ok_or_else(|| AppError::NotFound("Poll not found".into()))?;
    let channel = if place.poll.created_by == user_id {
        authz::require_channel_access(state, place.channel_id, user_id)
            .await?
            .channel
    } else {
        let channel = authz::find_channel(state, place.channel_id).await?;
        authz::require_channel_moderator(state, &channel, user_id).await?;
        channel
    };
    authz::require_not_archived(&channel)?;
    close(state, &place)
        .await?
        .ok_or_else(|| AppError::Conflict("This poll is already closed".into()))
}

async fn close(state: &AppState, place: &PollPlace) -> AppResult<Option<PollSummary>> {
    let mut tx = state.pool.begin().await?;
    let poll = state.poll_repo.lock_in(&mut tx, place.poll.id).await?;
    if poll.closed_at.is_some() {
        return Ok(None);
    }
    let poll = state.poll_repo.close_in(&mut tx, poll.id).await?;
    let (msg, summary) = write_summary(state, &mut tx, &poll).await?;
    let staged = state
        .publisher
        .stage_message_updated(&mut tx, &msg, place.workspace_id)
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(Some(summary))
}

pub async fn close_due(state: &Arc<AppState>) -> usize {
    let due = match state.poll_repo.due(DUE_BATCH).await {
        Ok(due) => due,
        Err(e) => {
            tracing::warn!("Failed to list polls due to close: {e}");
            return 0;
        }
    };
    let mut closed = 0;
    for poll_id in due {
        let place = match state.poll_repo.find(poll_id).await {
            Ok(Some(place)) => place,
            Ok(None) => continue,
            Err(e) => {
                tracing::warn!(%poll_id, "Failed to load a poll due to close: {e}");
                continue;
            }
        };
        match close(state, &place).await {
            Ok(Some(_)) => closed += 1,
            Ok(None) => {}
            Err(e) => tracing::warn!(%poll_id, "Failed to close a due poll: {e}"),
        }
    }
    closed
}

async fn write_summary(
    state: &AppState,
    conn: &mut PgConnection,
    poll: &Poll,
) -> AppResult<(Message, PollSummary)> {
    let (options, voters) = state.poll_repo.tally_in(conn, poll.id).await?;
    let summary = PollSummary {
        id: poll.id,
        question: poll.question.clone(),
        multiple_choice: poll.multiple_choice,
        anonymous: poll.anonymous,
        closes_at: poll.closes_at,
        closed_at: poll.closed_at,
        created_by: poll.created_by,
        options: options
            .into_iter()
            .map(|option| PollOptionCount {
                id: option.id,
                label: option.label,
                votes: option.votes,
            })
            .collect(),
        voters,
    };
    let json = serde_json::to_value(&summary).map_err(|e| AppError::Internal(e.to_string()))?;
    let msg = state
        .message_repo
        .set_poll_in(conn, poll.message_id, &json)
        .await?;
    Ok((msg, summary))
}
