use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::common::*;

#[sqlx::test(migrations = "../migrations")]
async fn deleting_a_message_lands_in_the_workspace_trail(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "audit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Audit WS").await;
    let ch = seed_channel(&state, ws, owner_id, "general", false).await;

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&token),
        Some(json!({ "content": "regrettable" })),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("message id").to_string();

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/audit-log"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let entry = body["data"]
        .as_array()
        .expect("data array")
        .iter()
        .find(|e| e["action"] == "message.deleted")
        .expect("the deletion is recorded");
    assert_eq!(entry["resource_id"], msg_id);
    assert_eq!(entry["user_id"], owner_id.to_string());
    assert_eq!(entry["details"]["channel_id"], ch.to_string());
}

#[sqlx::test(migrations = "../migrations")]
async fn a_moderated_deletion_names_the_moderator_and_the_author(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "audit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Audit WS").await;
    let ch = seed_channel(&state, ws, owner_id, "general", false).await;

    let (author_id, author_email) = seed(&state, "audit-author", false).await;
    add_ws_member(&state, ws, author_id, "member").await;
    let author_token = login(&app, &author_email, PASSWORD).await;

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&author_token),
        Some(json!({ "content": "off topic" })),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("message id").to_string();

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/audit-log?action=message.deleted"),
        Some(&owner_token),
        None,
    )
    .await;
    let entry = &body["data"][0];
    assert_eq!(entry["user_id"], owner_id.to_string(), "actor is the mod");
    assert_eq!(entry["details"]["author_id"], author_id.to_string());
    assert_eq!(entry["details"]["moderated"], true);
}

#[sqlx::test(migrations = "../migrations")]
async fn the_trail_records_the_originating_address(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "audit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Audit WS").await;
    let ch = seed_channel(&state, ws, owner_id, "temporary", false).await;

    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri(format!("/api/channels/{ch}"))
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .header("x-forwarded-for", "203.0.113.42")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(app.clone(), request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let recorded: Option<String> = sqlx::query_scalar(
        "SELECT host(ip_address) FROM audit_log WHERE action = 'channel.archived'",
    )
    .fetch_one(&state.pool)
    .await
    .expect("audit row");
    assert_eq!(
        recorded.as_deref(),
        Some("203.0.113.42"),
        "ip_address must be populated from the forwarded address, not left null"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn only_workspace_admins_can_read_the_trail(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "audit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Audit WS").await;

    let (member_id, member_email) = seed(&state, "audit-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member_token = login(&app, &member_email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/audit-log"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (_outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "audit-outsider", false).await;
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/audit-log"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/audit-log"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn the_workspace_trail_never_shows_another_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "audit-owner", false).await;
    let mine = seed_workspace(&state, owner_id, "Mine").await;
    let theirs = seed_workspace(&state, owner_id, "Theirs").await;
    let their_channel = seed_channel(&state, theirs, owner_id, "secret", false).await;

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{their_channel}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{mine}/audit-log"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let leaked = body["data"]
        .as_array()
        .expect("data array")
        .iter()
        .any(|e| e["resource_id"] == their_channel.to_string());
    assert!(!leaked, "entries must not cross workspaces: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn instance_admins_read_across_workspaces(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "audit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Audit WS").await;
    let ch = seed_channel(&state, ws, owner_id, "temporary", false).await;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, _, admin_token) = seed_and_login(&app, &state, "audit-instance", true).await;
    let (status, body) = send(
        &app,
        "GET",
        "/api/admin/audit-log",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let found = body["data"]
        .as_array()
        .expect("data array")
        .iter()
        .any(|e| e["action"] == "channel.archived");
    assert!(found, "instance admin sees every workspace: {body:?}");

    let (status, _) = send(
        &app,
        "GET",
        "/api/admin/audit-log",
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a workspace owner is not an instance admin"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn the_trail_survives_a_hard_workspace_delete(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "audit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Doomed WS").await;

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws}?hard=true"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "hard delete: {body:?}");

    let recorded: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'workspace.deleted' AND resource_id = $1",
    )
    .bind(ws)
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(
        recorded.0, 1,
        "the record of the deletion must outlive the workspace"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn role_changes_record_both_ends(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "audit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Audit WS").await;

    let (member_id, _) = seed(&state, "audit-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}/members/{member_id}/role"),
        Some(&token),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/audit-log?action=workspace.role_changed"),
        Some(&token),
        None,
    )
    .await;
    let entry = &body["data"][0];
    assert_eq!(entry["resource_id"], member_id.to_string());
    assert_eq!(entry["details"]["from"], "member");
    assert_eq!(entry["details"]["to"], "admin");
}

#[sqlx::test(migrations = "../migrations")]
async fn the_trail_pages_backwards_without_skipping_or_repeating(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "audit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Paging WS").await;

    for i in 0..5 {
        let ch = seed_channel(&state, ws, owner_id, &format!("temp-{i}"), false).await;
        let (status, _) = send(
            &app,
            "DELETE",
            &format!("/api/channels/{ch}"),
            Some(&token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let (_, first) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/audit-log?action=channel.archived&limit=2"),
        Some(&token),
        None,
    )
    .await;
    let page_one = first["data"].as_array().expect("data array").clone();
    assert_eq!(page_one.len(), 2);

    let last = page_one.last().expect("cursor row");
    let before = last["created_at"].as_str().expect("created_at");
    let before_id = last["id"].as_str().expect("id");

    let (_, second) = send(
        &app,
        "GET",
        &format!(
            "/api/workspaces/{ws}/audit-log?action=channel.archived&limit=2\
             &before={before}&before_id={before_id}"
        ),
        Some(&token),
        None,
    )
    .await;
    let page_two = second["data"].as_array().expect("data array");
    assert_eq!(page_two.len(), 2);
    for row in page_two {
        assert!(
            !page_one.iter().any(|seen| seen["id"] == row["id"]),
            "a cursor page must not repeat a row: {second:?}"
        );
    }
}
