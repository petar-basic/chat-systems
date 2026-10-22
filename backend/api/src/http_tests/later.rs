use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::workspace::models::ChannelRole;

struct World {
    app: axum::Router,
    state: std::sync::Arc<crate::state::AppState>,
    owner_id: Uuid,
    owner: String,
    ws_id: Uuid,
    ch_id: Uuid,
}

async fn world(pool: PgPool) -> World {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "later-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Later WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;
    World {
        app,
        state,
        owner_id,
        owner,
        ws_id,
        ch_id,
    }
}

async fn post_message(w: &World, token: &str, channel: Uuid, content: &str) -> String {
    let (status, message) = send(
        &w.app,
        "POST",
        &format!("/api/channels/{channel}/messages"),
        Some(token),
        Some(json!({ "content": content })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{message:?}");
    message["id"].as_str().expect("id").to_string()
}

async fn later(w: &World, token: &str, status: &str) -> Vec<Value> {
    let (code, body) = send(
        &w.app,
        "GET",
        &format!("/api/workspaces/{}/later?status={status}", w.ws_id),
        Some(token),
        None,
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{body:?}");
    body["data"].as_array().expect("data").clone()
}

async fn create(w: &World, token: &str, body: Value) -> (StatusCode, Value) {
    send(
        &w.app,
        "POST",
        &format!("/api/workspaces/{}/later", w.ws_id),
        Some(token),
        Some(body),
    )
    .await
}

async fn patch(w: &World, token: &str, id: &str, body: Value) -> StatusCode {
    send(
        &w.app,
        "PATCH",
        &format!("/api/later/{id}"),
        Some(token),
        Some(body),
    )
    .await
    .0
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_saved_message_moves_through_in_progress_completed_and_archived(pool: PgPool) {
    let w = world(pool).await;
    let message_id = post_message(&w, &w.owner, w.ch_id, "the runbook link").await;

    let (status, first) = create(&w, &w.owner, json!({ "message_id": message_id })).await;
    assert_eq!(status, StatusCode::OK, "{first:?}");
    let (_, second) = create(&w, &w.owner, json!({ "message_id": message_id })).await;
    assert_eq!(first["id"], second["id"], "saving twice is one item");
    let id = first["id"].as_str().expect("id").to_string();

    let open = later(&w, &w.owner, "in_progress").await;
    assert_eq!(open.len(), 1);
    assert_eq!(open[0]["message"]["content"], "the runbook link");
    assert_eq!(open[0]["message"]["channel_id"], w.ch_id.to_string());
    assert_eq!(open[0]["status"], "in_progress");

    assert_eq!(
        patch(&w, &w.owner, &id, json!({ "status": "completed" })).await,
        StatusCode::OK
    );
    assert!(later(&w, &w.owner, "in_progress").await.is_empty());
    let done = later(&w, &w.owner, "completed").await;
    assert_eq!(done.len(), 1);
    assert!(done[0]["completed_at"].is_string());

    assert_eq!(
        patch(&w, &w.owner, &id, json!({ "status": "archived" })).await,
        StatusCode::OK
    );
    assert_eq!(later(&w, &w.owner, "archived").await.len(), 1);
    assert!(later(&w, &w.owner, "archived").await[0]["completed_at"].is_null());

    let (_, again) = create(&w, &w.owner, json!({ "message_id": message_id })).await;
    assert_eq!(again["id"], first["id"]);
    assert_eq!(
        later(&w, &w.owner, "in_progress").await.len(),
        1,
        "saving it again brings it back"
    );

    let (status, _) = send(
        &w.app,
        "DELETE",
        &format!("/api/later/{id}"),
        Some(&w.owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(later(&w, &w.owner, "in_progress").await.is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_reminder_sorts_by_its_time_and_can_be_snoozed_or_cleared(pool: PgPool) {
    let w = world(pool).await;
    let message_id = post_message(&w, &w.owner, w.ch_id, "review the PR").await;
    let soon = Utc::now() + Duration::minutes(20);
    let later_on = Utc::now() + Duration::days(1);

    let (status, reminder) = create(
        &w,
        &w.owner,
        json!({ "text": "stand-up", "remind_at": later_on, "channel_id": w.ch_id }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reminder:?}");
    let (_, saved) = create(
        &w,
        &w.owner,
        json!({ "message_id": message_id, "remind_at": soon }),
    )
    .await;

    let open = later(&w, &w.owner, "in_progress").await;
    assert_eq!(open[0]["id"], saved["id"], "the sooner one comes first");
    assert_eq!(open[1]["text"], "stand-up");

    let id = saved["id"].as_str().expect("id");
    let past = Utc::now() - Duration::minutes(1);
    assert_eq!(
        patch(&w, &w.owner, id, json!({ "remind_at": past })).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        patch(&w, &w.owner, id, json!({ "clear_reminder": true })).await,
        StatusCode::OK
    );
    let open = later(&w, &w.owner, "in_progress").await;
    assert_eq!(
        open[0]["text"], "stand-up",
        "an item without a time sorts last"
    );
    assert!(open[1]["remind_at"].is_null());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_due_reminder_fires_once_and_stays_until_ticked(pool: PgPool) {
    let w = world(pool).await;
    let (_, created) = create(
        &w,
        &w.owner,
        json!({ "text": "ship it", "remind_at": Utc::now() + Duration::minutes(5) }),
    )
    .await;
    sqlx::query("UPDATE later_items SET remind_at = NOW() - INTERVAL '1 second'")
        .execute(&w.state.pool)
        .await
        .expect("make due");

    let due = w.state.later_repo.claim_due().await.expect("claim");
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].text.as_deref(), Some("ship it"));
    assert!(
        w.state
            .later_repo
            .claim_due()
            .await
            .expect("claim")
            .is_empty(),
        "a reminder fires once"
    );

    let open = later(&w, &w.owner, "in_progress").await;
    assert_eq!(open[0]["id"], created["id"]);
    assert!(
        open[0]["reminded_at"].is_string(),
        "fired, still in progress"
    );

    let id = created["id"].as_str().expect("id");
    let snooze = Utc::now() + Duration::hours(1);
    assert_eq!(
        patch(&w, &w.owner, id, json!({ "remind_at": snooze })).await,
        StatusCode::OK
    );
    assert!(later(&w, &w.owner, "in_progress").await[0]["reminded_at"].is_null());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_reminder_is_not_delivered_into_a_channel_its_target_left(pool: PgPool) {
    let w = world(pool).await;
    let private = seed_channel(&w.state, w.ws_id, w.owner_id, "private-room", true).await;
    let (target_id, _) = seed(&w.state, "later-target", false).await;
    add_ws_member(&w.state, w.ws_id, target_id, "member").await;
    w.state
        .workspace_service
        .repo
        .add_channel_member(private, target_id, &ChannelRole::Member)
        .await
        .expect("join");
    let due = crate::later::models::DueReminder {
        id: Uuid::new_v4(),
        workspace_id: w.ws_id,
        user_id: target_id,
        channel_id: Some(private),
        message_id: None,
        text: Some("stand-up".into()),
    };
    assert!(crate::later::reminders::is_deliverable(&w.state, &due).await);
    w.state
        .workspace_service
        .repo
        .remove_channel_member(private, target_id)
        .await
        .expect("leave");
    assert!(!crate::later::reminders::is_deliverable(&w.state, &due).await);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_an_admin_sets_a_reminder_for_somebody_else(pool: PgPool) {
    let w = world(pool).await;
    let (member_id, _, member) = seed_and_login(&w.app, &w.state, "later-member", false).await;
    add_ws_member(&w.state, w.ws_id, member_id, "member").await;
    let at = Utc::now() + Duration::hours(1);

    let (status, _) = create(
        &w,
        &member,
        json!({ "text": "pay up", "remind_at": at, "for_user_id": w.owner_id }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = create(
        &w,
        &w.owner,
        json!({ "text": "update the doc", "remind_at": at, "for_user_id": member_id }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let theirs = later(&w, &member, "in_progress").await;
    assert_eq!(theirs.len(), 1);
    assert_eq!(theirs[0]["created_by"], w.owner_id.to_string());
    assert!(later(&w, &w.owner, "in_progress").await.is_empty());

    let (outsider_id, _) = seed(&w.state, "later-outsider", false).await;
    let (status, _) = create(
        &w,
        &w.owner,
        json!({ "text": "x", "remind_at": at, "for_user_id": outsider_id }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn nobody_sees_or_changes_anyone_elses_list(pool: PgPool) {
    let w = world(pool).await;
    let message_id = post_message(&w, &w.owner, w.ch_id, "mine").await;
    let (_, item) = create(&w, &w.owner, json!({ "message_id": message_id })).await;
    let id = item["id"].as_str().expect("id");

    let (member_id, _, member) = seed_and_login(&w.app, &w.state, "later-snoop", false).await;
    add_ws_member(&w.state, w.ws_id, member_id, "member").await;
    assert!(later(&w, &member, "in_progress").await.is_empty());
    assert_eq!(
        patch(&w, &member, id, json!({ "status": "completed" })).await,
        StatusCode::NOT_FOUND
    );
    let (status, _) = send(
        &w.app,
        "DELETE",
        &format!("/api/later/{id}"),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, _, outsider) = seed_and_login(&w.app, &w.state, "later-outsider2", false).await;
    let (status, _) = send(
        &w.app,
        "GET",
        &format!("/api/workspaces/{}/later", w.ws_id),
        Some(&outsider),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &w.app,
        "GET",
        &format!("/api/workspaces/{}/later", w.ws_id),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn you_cannot_save_what_you_cannot_read(pool: PgPool) {
    let w = world(pool).await;
    let private = seed_channel(&w.state, w.ws_id, w.owner_id, "secret", true).await;
    let message_id = post_message(&w, &w.owner, private, "classified").await;
    let (member_id, _, member) = seed_and_login(&w.app, &w.state, "later-reader", false).await;
    add_ws_member(&w.state, w.ws_id, member_id, "member").await;

    let (status, _) = create(&w, &member, json!({ "message_id": message_id })).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = create(&w, &member, json!({})).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "save a message or write a reminder"
    );
    let (status, _) = create(&w, &member, json!({ "text": "no time" })).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a reminder needs a time"
    );
}
