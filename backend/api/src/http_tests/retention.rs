use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::config::AppConfig;

async fn age_message(state: &crate::state::AppState, msg_id: Uuid, days: i64) {
    sqlx::query("UPDATE messages SET created_at = NOW() - ($2 || ' days')::interval WHERE id = $1")
        .bind(msg_id)
        .bind(days.to_string())
        .execute(&state.pool)
        .await
        .expect("age the message");
}

async fn post(app: &axum::Router, token: &str, ch: Uuid, content: &str) -> Uuid {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(token),
        Some(json!({ "content": content })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["id"].as_str().expect("id").parse().expect("uuid")
}

async fn set_policy(
    state: &crate::state::AppState,
    ws: Uuid,
    owner_id: Uuid,
    message_days: Option<i32>,
) {
    state
        .retention_repo
        .upsert(
            ws,
            &crate::retention::repo::UpdateRetentionRequest {
                message_days,
                file_days: None,
                audit_days: Some(730),
                notification_days: Some(90),
                channel_overrides: false,
            },
            owner_id,
        )
        .await
        .expect("set policy");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_the_older_side_of_the_boundary_is_removed(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ret-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Retention WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let old = post(&app, &token, ch, "long ago").await;
    let recent = post(&app, &token, ch, "yesterday").await;
    age_message(&state, old, 400).await;
    age_message(&state, recent, 2).await;

    set_policy(&state, ws, owner_id, Some(30)).await;
    crate::retention::job::run_once(&state).await;

    let survivors: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM messages WHERE channel_id = $1")
        .bind(ch)
        .fetch_all(&state.pool)
        .await
        .expect("list");
    assert!(!survivors.contains(&old), "past the boundary goes");
    assert!(survivors.contains(&recent), "inside the boundary stays");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn keeping_forever_is_the_default_and_deletes_nothing(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ret-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Retention WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let ancient = post(&app, &token, ch, "from the beginning").await;
    age_message(&state, ancient, 5000).await;

    // A policy row exists but names no message retention: NULL means forever.
    set_policy(&state, ws, owner_id, None).await;
    crate::retention::job::run_once(&state).await;

    let still_there: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE id = $1")
        .bind(ancient)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(
        still_there, 1,
        "turning on deletion has to be a deliberate act"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn dry_run_reports_without_deleting(pool: PgPool) {
    let config = AppConfig {
        retention_dry_run: true,
        ..test_config()
    };
    let (app, state) = app_and_state_with(pool, config).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ret-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Retention WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let old = post(&app, &token, ch, "long ago").await;
    age_message(&state, old, 400).await;
    set_policy(&state, ws, owner_id, Some(30)).await;

    let counts = crate::retention::job::run_once(&state).await;

    assert!(counts.messages > 0, "it reports what it would remove");
    let still_there: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE id = $1")
        .bind(old)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(still_there, 1, "and removes none of it");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn expired_tokens_are_cleaned_without_any_policy(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, _, _) = seed_and_login(&app, &state, "ret-user", false).await;

    sqlx::query(
        "INSERT INTO password_reset_tokens (jti, user_id, expires_at) VALUES ($1, $2, NOW() - INTERVAL '1 day')",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .execute(&state.pool)
    .await
    .expect("seed a spent reset token");

    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, NOW() - INTERVAL '1 day')",
    )
    .bind(user_id)
    .bind(format!("expired-{}", Uuid::new_v4()))
    .execute(&state.pool)
    .await
    .expect("seed an expired refresh token");

    crate::retention::job::run_once(&state).await;

    let reset: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(reset, 0, "a consumed reset token has no reason to exist");

    let refresh: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1 AND expires_at < NOW()",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(refresh, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_audit_trail_outlives_the_messages_it_describes(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ret-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Retention WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let msg = post(&app, &token, ch, "will be purged").await;
    age_message(&state, msg, 400).await;

    // An audit entry from the same era, well inside the two-year default.
    sqlx::query(
        "INSERT INTO audit_log (workspace_id, user_id, action, resource_type, created_at) \
         VALUES ($1, $2, 'message.deleted', 'message', NOW() - INTERVAL '400 days')",
    )
    .bind(ws)
    .bind(owner_id)
    .execute(&state.pool)
    .await
    .expect("seed an audit row");

    set_policy(&state, ws, owner_id, Some(30)).await;
    crate::retention::job::run_once(&state).await;

    let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE id = $1")
        .bind(msg)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(messages, 0);

    let audit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE workspace_id = $1")
        .bind(ws)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert!(
        audit > 0,
        "the log is what answers questions about data that is already gone"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_policy_is_owner_only_and_the_change_is_audited(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "ret-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Retention WS").await;

    let (admin_id, admin_email) = seed(&state, "ret-admin", false).await;
    add_ws_member(&state, ws, admin_id, "admin").await;
    let admin = login(&app, &admin_email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}/retention"),
        Some(&admin),
        Some(json!({ "message_days": 30 })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin cannot switch on irreversible deletion"
    );

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}/retention"),
        Some(&owner),
        Some(json!({ "message_days": 30 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["policy"]["message_days"], 30);

    let (status, read) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/retention"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "an admin may read it");
    assert_eq!(read["policy"]["message_days"], 30);

    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'retention.changed' AND workspace_id = $1",
    )
    .bind(ws)
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(audited, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_preview_counts_what_a_period_would_remove_now(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "ret-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Retention WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    for days in [40, 60] {
        let id = post(&app, &owner, ch, "old").await;
        age_message(&state, id, days).await;
    }
    post(&app, &owner, ch, "fresh").await;

    let preview =
        |days: &str| format!("/api/workspaces/{ws}/retention/preview?message_days={days}");

    let (status, body) = send(&app, "GET", &preview("30"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["messages"], 2);
    assert_eq!(body["files"], 0);

    let (_, body) = send(&app, "GET", &preview("50"), Some(&owner), None).await;
    assert_eq!(body["messages"], 1, "only what is past the boundary");

    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages m JOIN channels c ON c.id = m.channel_id WHERE c.workspace_id = $1",
    )
    .bind(ws)
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert!(remaining >= 3, "a preview deletes nothing");

    let (status, _) = send(&app, "GET", &preview("0"), Some(&owner), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (admin_id, admin_email) = seed(&state, "ret-admin", false).await;
    add_ws_member(&state, ws, admin_id, "admin").await;
    let admin = login(&app, &admin_email, PASSWORD).await;
    let (status, _) = send(&app, "GET", &preview("30"), Some(&admin), None).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the preview belongs to the owner's form"
    );
}

async fn allow_channel_periods(
    state: &crate::state::AppState,
    ws: Uuid,
    owner_id: Uuid,
    message_days: Option<i32>,
    channel_overrides: bool,
) {
    state
        .retention_repo
        .upsert(
            ws,
            &crate::retention::repo::UpdateRetentionRequest {
                message_days,
                file_days: None,
                audit_days: Some(730),
                notification_days: Some(90),
                channel_overrides,
            },
            owner_id,
        )
        .await
        .expect("set policy");
}

async fn channel_period(
    state: &crate::state::AppState,
    ch: Uuid,
    owner_id: Uuid,
    days: Option<i32>,
) {
    state
        .retention_repo
        .set_channel_override(ch, days, owner_id)
        .await
        .expect("set channel period");
}

async fn aged_post(
    app: &axum::Router,
    state: &crate::state::AppState,
    token: &str,
    ch: Uuid,
    days: i64,
) -> Uuid {
    let id = post(app, token, ch, &format!("{days} days old")).await;
    age_message(state, id, days).await;
    id
}

async fn exists(state: &crate::state::AppState, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM messages WHERE id = $1)")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .expect("exists")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_period_purges_before_and_after_the_workspaces(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ret-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Retention WS").await;
    let short = seed_channel(&state, ws, owner_id, "random", false).await;
    let long = seed_channel(&state, ws, owner_id, "legal", false).await;
    let plain = seed_channel(&state, ws, owner_id, "general", false).await;

    let short_20 = aged_post(&app, &state, &token, short, 20).await;
    let long_45 = aged_post(&app, &state, &token, long, 45).await;
    let long_90 = aged_post(&app, &state, &token, long, 90).await;
    let plain_20 = aged_post(&app, &state, &token, plain, 20).await;
    let plain_45 = aged_post(&app, &state, &token, plain, 45).await;

    allow_channel_periods(&state, ws, owner_id, Some(30), true).await;
    channel_period(&state, short, owner_id, Some(10)).await;
    channel_period(&state, long, owner_id, Some(60)).await;
    crate::retention::job::run_once(&state).await;

    assert!(
        !exists(&state, short_20).await,
        "a shorter period goes first"
    );
    assert!(
        exists(&state, long_45).await,
        "a longer one outlives the workspace's"
    );
    assert!(!exists(&state, long_90).await, "but not its own");
    assert!(
        exists(&state, plain_20).await,
        "a channel without one follows the workspace"
    );
    assert!(!exists(&state, plain_45).await);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_kept_forever_is_kept_and_a_workspace_kept_forever_still_purges_a_channel(
    pool: PgPool,
) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ret-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Retention WS").await;
    let kept = seed_channel(&state, ws, owner_id, "legal", false).await;
    let trimmed = seed_channel(&state, ws, owner_id, "random", false).await;
    let plain = seed_channel(&state, ws, owner_id, "general", false).await;

    let kept_old = aged_post(&app, &state, &token, kept, 400).await;
    let trimmed_old = aged_post(&app, &state, &token, trimmed, 20).await;
    let plain_old = aged_post(&app, &state, &token, plain, 400).await;

    allow_channel_periods(&state, ws, owner_id, Some(30), true).await;
    channel_period(&state, kept, owner_id, None).await;
    crate::retention::job::run_once(&state).await;
    assert!(exists(&state, kept_old).await, "forever means forever");
    assert!(!exists(&state, plain_old).await);

    allow_channel_periods(&state, ws, owner_id, None, true).await;
    channel_period(&state, trimmed, owner_id, Some(10)).await;
    let counts = crate::retention::job::run_once(&state).await;
    assert!(
        !exists(&state, trimmed_old).await,
        "the channel's period applies on its own"
    );
    assert_eq!(counts.messages, 1);
    assert!(exists(&state, kept_old).await);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn channel_periods_are_ignored_while_overrides_are_off(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ret-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Retention WS").await;
    let ch = seed_channel(&state, ws, owner_id, "random", false).await;

    let twenty = aged_post(&app, &state, &token, ch, 20).await;
    channel_period(&state, ch, owner_id, Some(10)).await;
    allow_channel_periods(&state, ws, owner_id, Some(30), false).await;
    crate::retention::job::run_once(&state).await;

    assert!(exists(&state, twenty).await, "the row is kept and ignored");
    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/retention"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["overrides_allowed"], false);
    assert_eq!(body["workspace_message_days"], 30);
    assert_eq!(body["channel"]["message_days"], 10);

    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/channels/{ch}/retention"),
        Some(&token),
        Some(json!({ "message_days": 5 })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "nothing to set while the owner says no"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_moderator_sets_the_period_and_the_channel_and_audit_log_say_so(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "ret-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Retention WS").await;
    let ch = seed_channel(&state, ws, owner_id, "random", false).await;
    let (member_id, _, member) = seed_and_login(&app, &state, "ret-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/join"),
        Some(&member),
        None,
    )
    .await;
    allow_channel_periods(&state, ws, owner_id, Some(365), true).await;

    let path = format!("/api/channels/{ch}/retention");
    let (status, _) = send(
        &app,
        "PUT",
        &path,
        Some(&member),
        Some(json!({ "message_days": 7 })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a member who does not moderate is refused"
    );
    let (status, _) = send(&app, "DELETE", &path, Some(&member), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "PUT",
        &path,
        Some(&owner),
        Some(json!({ "message_days": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, body) = send(
        &app,
        "PUT",
        &path,
        Some(&owner),
        Some(json!({ "message_days": 7 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["channel"]["message_days"], 7);
    let (status, body) = send(
        &app,
        "PUT",
        &path,
        Some(&owner),
        Some(json!({ "message_days": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(
        body["channel"]["message_days"].is_null(),
        "null keeps forever"
    );
    let (status, body) = send(&app, "DELETE", &path, Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["channel"].is_null(), "back to the workspace's");

    let said: Vec<String> = sqlx::query_scalar(
        "SELECT content FROM messages WHERE channel_id = $1 AND metadata->>'kind' = 'retention_changed' ORDER BY created_at",
    )
    .bind(ch)
    .fetch_all(&state.pool)
    .await
    .expect("system messages");
    assert_eq!(
        said,
        [
            "set this channel to keep messages for 7 days",
            "set this channel to keep messages forever",
            "set this channel to follow the workspace's message retention",
        ]
    );

    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'retention.changed' AND details->>'channel_id' = $1",
    )
    .bind(ch.to_string())
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(audited, 3);
}
