use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

fn in_an_hour() -> String {
    (Utc::now() + Duration::hours(1)).to_rfc3339()
}

#[sqlx::test(migrations = "../migrations")]
async fn a_member_schedules_into_a_channel_they_can_post_to(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Scheduled WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

    let (status, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&owner_token),
        Some(json!({ "channel_id": ch_id, "content": "morning, all", "send_at": in_an_hour() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "schedule: {created:?}");
    assert!(created["sent_at"].is_null());

    let (status, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listing["data"].as_array().expect("array").len(), 1);

    let (_, messages) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&owner_token),
        None,
    )
    .await;
    assert!(
        messages["data"].as_array().expect("array").is_empty(),
        "nothing lands in the channel before its time"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn scheduling_needs_access_to_the_target(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Scheduled Guard WS").await;
    let private_id = seed_channel(&state, ws_id, owner_id, "secret", true).await;

    let (outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "sched-outsider", false).await;
    add_ws_member(&state, ws_id, outsider_id, "member").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&outsider_token),
        Some(
            json!({ "channel_id": private_id, "content": "sneaking in", "send_at": in_an_hour() }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "cannot schedule into a private channel you are not in"
    );

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [outsider_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id");

    let (stranger_id, _, stranger_token) =
        seed_and_login(&app, &state, "sched-stranger", false).await;
    add_ws_member(&state, ws_id, stranger_id, "member").await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&stranger_token),
        Some(json!({ "conversation_id": conv_id, "content": "not my thread", "send_at": in_an_hour() })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&owner_token),
        Some(json!({ "content": "no target", "send_at": in_an_hour() })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "needs one target");

    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&owner_token),
        Some(json!({
            "channel_id": ch_id,
            "conversation_id": conv_id,
            "content": "two targets",
            "send_at": in_an_hour()
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "exactly one target"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn the_scheduled_time_has_to_be_ahead_and_within_reach(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Scheduled Time WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

    for send_at in [
        (Utc::now() - Duration::minutes(1)).to_rfc3339(),
        (Utc::now() + Duration::days(400)).to_rfc3339(),
    ] {
        let (status, _) = send(
            &app,
            "POST",
            &format!("/api/workspaces/{ws_id}/scheduled-messages"),
            Some(&owner_token),
            Some(json!({ "channel_id": ch_id, "content": "when?", "send_at": send_at })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&owner_token),
        Some(json!({ "channel_id": ch_id, "content": "   ", "send_at": in_an_hour() })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "empty content");
}

#[sqlx::test(migrations = "../migrations")]
async fn only_the_author_reschedules_or_cancels(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Scheduled Owner WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;
    let (other_id, _, other_token) = seed_and_login(&app, &state, "sched-other", false).await;
    add_ws_member(&state, ws_id, other_id, "member").await;

    let (_, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&owner_token),
        Some(json!({ "channel_id": ch_id, "content": "later", "send_at": in_an_hour() })),
    )
    .await;
    let id = created["id"].as_str().expect("id").to_string();

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/scheduled-messages/{id}"),
        Some(&other_token),
        Some(json!({ "send_at": in_an_hour() })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let later = (Utc::now() + Duration::hours(5)).to_rfc3339();
    let (status, moved) = send(
        &app,
        "PATCH",
        &format!("/api/scheduled-messages/{id}"),
        Some(&owner_token),
        Some(json!({ "send_at": later })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "author reschedules: {moved:?}");

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/scheduled-messages/{id}"),
        Some(&other_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/scheduled-messages/{id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/scheduled-messages/{id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a canceled message cannot be canceled twice"
    );

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&owner_token),
        None,
    )
    .await;
    assert!(
        listing["data"].as_array().expect("array").is_empty(),
        "canceled messages drop off the pending list"
    );

    let unknown = Uuid::new_v4();
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/scheduled-messages/{unknown}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../migrations")]
async fn the_dispatcher_delivers_due_messages_exactly_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Dispatch WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;
    let (partner_id, _, partner_token) = seed_and_login(&app, &state, "sched-partner", false).await;
    add_ws_member(&state, ws_id, partner_id, "member").await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [partner_id] })),
    )
    .await;
    let conv_id: Uuid = conv["id"].as_str().expect("id").parse().expect("uuid");

    for (channel, conversation) in [(Some(ch_id), None), (None, Some(conv_id))] {
        state
            .scheduled_repo
            .create(crate::scheduled::repo::NewScheduledMessage {
                workspace_id: ws_id,
                user_id: owner_id,
                channel_id: channel,
                conversation_id: conversation,
                content: "sent by the dispatcher",
                send_at: Utc::now() - Duration::seconds(5),
            })
            .await
            .expect("queue a due message");
    }

    let claimed = state.scheduled_repo.claim_due().await.expect("claim");
    assert_eq!(claimed.len(), 2, "both due messages are claimed");
    for scheduled in &claimed {
        crate::scheduled::executor::deliver_for_test(&state, scheduled)
            .await
            .expect("deliver");
    }

    let again = state.scheduled_repo.claim_due().await.expect("claim again");
    assert!(
        again.is_empty(),
        "claiming marks rows sent, so a second dispatcher tick delivers nothing"
    );

    let (_, channel_messages) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(
        channel_messages["data"][0]["content"],
        "sent by the dispatcher"
    );

    let (_, conversation_messages) = send(
        &app,
        "GET",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&partner_token),
        None,
    )
    .await;
    assert_eq!(
        conversation_messages["data"][0]["content"], "sent by the dispatcher",
        "the partner sees the delivered message"
    );
}

async fn queue_due(
    state: &crate::state::AppState,
    ws_id: Uuid,
    user_id: Uuid,
    channel_id: Option<Uuid>,
    conversation_id: Option<Uuid>,
) -> crate::scheduled::models::ScheduledMessage {
    state
        .scheduled_repo
        .create(crate::scheduled::repo::NewScheduledMessage {
            workspace_id: ws_id,
            user_id,
            channel_id,
            conversation_id,
            content: "written before losing access",
            send_at: Utc::now() - Duration::seconds(5),
        })
        .await
        .expect("queue a due message")
}

async fn deliver_now(
    state: &crate::state::AppState,
    scheduled: &crate::scheduled::models::ScheduledMessage,
) -> Result<(), crate::scheduled::executor::DeliveryFailure> {
    let claimed = state.scheduled_repo.claim_due().await.expect("claim");
    let row = claimed
        .into_iter()
        .find(|c| c.id == scheduled.id)
        .expect("the queued message is due");
    crate::scheduled::executor::deliver_for_test(state, &row).await
}

async fn channel_message_count(state: &crate::state::AppState, ch_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE channel_id = $1")
        .bind(ch_id)
        .fetch_one(&state.pool)
        .await
        .expect("count messages")
}

#[sqlx::test(migrations = "../migrations")]
async fn a_message_from_someone_removed_from_the_channel_is_not_delivered(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reauth WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "private-room", true).await;

    let (author_id, _) = seed(&state, "sched-author", false).await;
    add_ws_member(&state, ws_id, author_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(
            ch_id,
            author_id,
            &crate::workspace::models::ChannelRole::Member,
        )
        .await
        .expect("add to channel");

    let scheduled = queue_due(&state, ws_id, author_id, Some(ch_id), None).await;

    state
        .workspace_service
        .repo
        .remove_channel_member(ch_id, author_id)
        .await
        .expect("remove from channel");

    let failure = deliver_now(&state, &scheduled).await.expect_err("refused");
    assert_eq!(
        failure,
        crate::scheduled::executor::DeliveryFailure::NotAuthorized
    );
    assert_eq!(
        channel_message_count(&state, ch_id).await,
        0,
        "nothing may be written on behalf of a removed member"
    );

    let recorded: Option<String> =
        sqlx::query_scalar("SELECT failure FROM scheduled_messages WHERE id = $1")
            .bind(scheduled.id)
            .fetch_one(&state.pool)
            .await
            .expect("row");
    assert_eq!(recorded.as_deref(), Some("not_authorized"));
}

#[sqlx::test(migrations = "../migrations")]
async fn a_message_from_someone_removed_from_the_workspace_is_not_delivered(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reauth WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

    let (author_id, _) = seed(&state, "sched-author", false).await;
    add_ws_member(&state, ws_id, author_id, "member").await;

    let scheduled = queue_due(&state, ws_id, author_id, Some(ch_id), None).await;

    state
        .workspace_service
        .repo
        .remove_member(ws_id, author_id)
        .await
        .expect("remove from workspace");

    let failure = deliver_now(&state, &scheduled).await.expect_err("refused");
    assert_eq!(
        failure,
        crate::scheduled::executor::DeliveryFailure::NotAuthorized
    );
    assert_eq!(channel_message_count(&state, ch_id).await, 0);
}

#[sqlx::test(migrations = "../migrations")]
async fn a_message_to_an_archived_channel_is_not_delivered(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reauth WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "going-away", false).await;

    let scheduled = queue_due(&state, ws_id, owner_id, Some(ch_id), None).await;

    state
        .workspace_service
        .repo
        .archive_channel(ch_id)
        .await
        .expect("archive");

    let failure = deliver_now(&state, &scheduled).await.expect_err("refused");
    assert_eq!(
        failure,
        crate::scheduled::executor::DeliveryFailure::ChannelArchived
    );
    assert_eq!(channel_message_count(&state, ch_id).await, 0);
}

#[sqlx::test(migrations = "../migrations")]
async fn a_message_into_a_deleted_workspace_is_not_delivered(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Doomed WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

    let scheduled = queue_due(&state, ws_id, owner_id, Some(ch_id), None).await;

    state
        .workspace_service
        .repo
        .soft_delete_workspace(ws_id)
        .await
        .expect("soft delete");

    let failure = deliver_now(&state, &scheduled).await.expect_err("refused");
    assert_eq!(
        failure,
        crate::scheduled::executor::DeliveryFailure::WorkspaceUnavailable
    );
    assert_eq!(channel_message_count(&state, ch_id).await, 0);
}

#[sqlx::test(migrations = "../migrations")]
async fn a_dm_from_a_removed_participant_is_not_delivered(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "DM Reauth WS").await;
    let (partner_id, _) = seed(&state, "sched-partner", false).await;
    add_ws_member(&state, ws_id, partner_id, "member").await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [partner_id] })),
    )
    .await;
    let conv_id: Uuid = conv["id"].as_str().expect("id").parse().expect("uuid");

    let scheduled = queue_due(&state, ws_id, partner_id, None, Some(conv_id)).await;

    sqlx::query(
        "DELETE FROM conversation_participants WHERE conversation_id = $1 AND user_id = $2",
    )
    .bind(conv_id)
    .bind(partner_id)
    .execute(&state.pool)
    .await
    .expect("drop the participant");

    let failure = deliver_now(&state, &scheduled).await.expect_err("refused");
    assert_eq!(
        failure,
        crate::scheduled::executor::DeliveryFailure::NotAuthorized
    );

    let delivered: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM conversation_messages WHERE conversation_id = $1")
            .bind(conv_id)
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(delivered, 0);
}

#[sqlx::test(migrations = "../migrations")]
async fn a_refused_delivery_tells_the_author(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Notify WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "going-away", false).await;

    let scheduled = queue_due(&state, ws_id, owner_id, Some(ch_id), None).await;
    state
        .workspace_service
        .repo
        .archive_channel(ch_id)
        .await
        .expect("archive");
    let _ = deliver_now(&state, &scheduled).await;

    let notifications = state
        .notification_repo
        .list_for_user(owner_id, ws_id, 50, 0)
        .await
        .expect("list notifications");
    let told = notifications
        .iter()
        .any(|n| n.title == "Scheduled message was not delivered");
    assert!(
        told,
        "a message that evaporates without a word is worse than one that fails loudly"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn a_failed_message_stays_visible_to_its_author(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Visible WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "going-away", false).await;

    let scheduled = queue_due(&state, ws_id, owner_id, Some(ch_id), None).await;
    state
        .workspace_service
        .repo
        .archive_channel(ch_id)
        .await
        .expect("archive");
    let _ = deliver_now(&state, &scheduled).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let listed = body["data"]
        .as_array()
        .expect("data array")
        .iter()
        .find(|m| m["id"] == scheduled.id.to_string())
        .expect("the failed message is still listed");
    assert_eq!(listed["failure"], "channel_archived");
}

#[sqlx::test(migrations = "../migrations")]
async fn removal_cancels_pending_messages_for_that_scope(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "sched-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Cancel WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;
    let other_ch = seed_channel(&state, ws_id, owner_id, "elsewhere", false).await;

    let (author_id, _, author_token) = seed_and_login(&app, &state, "sched-author", false).await;
    add_ws_member(&state, ws_id, author_id, "member").await;
    for channel in [ch_id, other_ch] {
        state
            .workspace_service
            .repo
            .add_channel_member(
                channel,
                author_id,
                &crate::workspace::models::ChannelRole::Member,
            )
            .await
            .expect("add to channel");
    }

    let in_channel = queue_due(&state, ws_id, author_id, Some(ch_id), None).await;
    let elsewhere = queue_due(&state, ws_id, author_id, Some(other_ch), None).await;

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}/members/{author_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let canceled: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT canceled_at FROM scheduled_messages WHERE id = $1")
            .bind(in_channel.id)
            .fetch_one(&state.pool)
            .await
            .expect("row");
    assert!(canceled.is_some(), "leaving a channel cancels its queue");

    let untouched: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT canceled_at FROM scheduled_messages WHERE id = $1")
            .bind(elsewhere.id)
            .fetch_one(&state.pool)
            .await
            .expect("row");
    assert!(
        untouched.is_none(),
        "a message queued for another channel is not collateral"
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}/members/{author_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let now_canceled: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT canceled_at FROM scheduled_messages WHERE id = $1")
            .bind(elsewhere.id)
            .fetch_one(&state.pool)
            .await
            .expect("row");
    assert!(
        now_canceled.is_some(),
        "leaving the workspace cancels everything queued in it"
    );
    let _ = author_token;
}

#[sqlx::test(migrations = "../migrations")]
async fn a_reminder_is_dropped_when_its_target_lost_the_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "rem-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reminder WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "private-room", true).await;

    let (target_id, _) = seed(&state, "rem-target", false).await;
    add_ws_member(&state, ws_id, target_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(
            ch_id,
            target_id,
            &crate::workspace::models::ChannelRole::Member,
        )
        .await
        .expect("add to channel");

    let reminder = state
        .hook_repo
        .create_reminder(crate::hooks::repo::NewReminder {
            workspace_id: ws_id,
            created_by: owner_id,
            target_user_id: target_id,
            channel_id: Some(ch_id),
            message_id: None,
            content: "stand-up",
            remind_at: Utc::now() - Duration::seconds(5),
        })
        .await
        .expect("create reminder");

    assert!(
        crate::hooks::executor::reminder_is_deliverable(&state, &reminder).await,
        "a member of the channel still gets their reminder"
    );

    state
        .workspace_service
        .repo
        .remove_channel_member(ch_id, target_id)
        .await
        .expect("remove from channel");

    assert!(
        !crate::hooks::executor::reminder_is_deliverable(&state, &reminder).await,
        "a reminder must not point at a channel its target can no longer read"
    );
}
