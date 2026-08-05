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
