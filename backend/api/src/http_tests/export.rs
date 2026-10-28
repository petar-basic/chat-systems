use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

async fn run_pending_export(state: &crate::state::AppState) -> Uuid {
    let job = state
        .export_repo
        .claim_next()
        .await
        .expect("claim")
        .expect("a pending job");
    let id = job.id;
    // The worker's own body, called directly so the test does not race a poll.
    crate::export::job::run_for_test(state, job)
        .await
        .expect("export runs");
    id
}

async fn manifest_of(state: &crate::state::AppState, id: Uuid) -> serde_json::Value {
    state
        .export_repo
        .find(id)
        .await
        .expect("find")
        .expect("job")
        .manifest
        .expect("manifest")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_workspace_export_counts_what_it_wrote(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "exp-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Export WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    for i in 0..3 {
        send(
            &app,
            "POST",
            &format!("/api/channels/{ch}/messages"),
            Some(&owner),
            Some(json!({ "content": format!("line {i}") })),
        )
        .await;
    }

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/exports"),
        Some(&owner),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let id = run_pending_export(&state).await;
    let manifest = manifest_of(&state, id).await;

    let rows = manifest["files"]["messages.jsonl"]["rows"]
        .as_u64()
        .expect("row count");
    let actual: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages m JOIN channels c ON c.id = m.channel_id WHERE c.workspace_id = $1",
    )
    .bind(ws)
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(
        rows, actual as u64,
        "the manifest counts what the database has"
    );

    let digest = manifest["files"]["messages.jsonl"]["sha256"]
        .as_str()
        .expect("digest");
    assert_eq!(digest.len(), 64, "and carries a checksum for it");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn private_conversations_stay_out_unless_asked_for(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "exp-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Export WS").await;
    let (other_id, _) = seed(&state, "exp-other", false).await;
    add_ws_member(&state, ws, other_id, "member").await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/conversations"),
        Some(&owner),
        Some(json!({ "participant_ids": [other_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id").to_string();
    send(
        &app,
        "POST",
        &format!("/api/channels/{conv_id}/messages"),
        Some(&owner),
        Some(json!({ "content": "between us" })),
    )
    .await;

    send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/exports"),
        Some(&owner),
        Some(json!({})),
    )
    .await;
    let without = manifest_of(&state, run_pending_export(&state).await).await;
    assert_eq!(
        without["files"]["messages.jsonl"]["rows"], 0,
        "an owner clicking export does not sweep up everyone's DMs"
    );

    send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/exports"),
        Some(&owner),
        Some(json!({ "include_dms": true })),
    )
    .await;
    let with = manifest_of(&state, run_pending_export(&state).await).await;
    assert_eq!(with["files"]["messages.jsonl"]["rows"], 1);

    let opted_in: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'export.requested' \
           AND (details->>'include_dms')::boolean = true",
    )
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(opted_in, 1, "and opting in is itself on the record");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_user_export_contains_only_that_persons_authorship(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "exp-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Export WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let (subject_id, subject_email) = seed(&state, "exp-subject", false).await;
    add_ws_member(&state, ws, subject_id, "member").await;
    let subject = login(&app, &subject_email, PASSWORD).await;

    send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "not theirs" })),
    )
    .await;
    send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&subject),
        Some(json!({ "content": "theirs" })),
    )
    .await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/users/{subject_id}/exports"),
        Some(&subject),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "you may export your own data");

    let manifest = manifest_of(&state, run_pending_export(&state).await).await;
    assert_eq!(manifest["files"]["messages.jsonl"]["rows"], 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn exports_are_owner_only_and_downloads_work_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "exp-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Export WS").await;

    let (admin_id, admin_email) = seed(&state, "exp-admin", false).await;
    add_ws_member(&state, ws, admin_id, "admin").await;
    let admin = login(&app, &admin_email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/exports"),
        Some(&admin),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin cannot export the whole workspace"
    );

    send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/exports"),
        Some(&owner),
        Some(json!({})),
    )
    .await;
    let id = run_pending_export(&state).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/exports/{id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let url = body["download_url"].as_str().expect("a download link");
    let token = url.rsplit('/').next().expect("token");

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/exports/download/{token}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the link works");

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/exports/download/{token}"),
        None,
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "and only once — a pasted link is dead on arrival"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn anonymising_keeps_the_conversation_readable(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _owner) = seed_and_login(&app, &state, "exp-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Export WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let (subject_id, subject_email) = seed(&state, "exp-subject", false).await;
    add_ws_member(&state, ws, subject_id, "member").await;
    let subject = login(&app, &subject_email, PASSWORD).await;
    send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&subject),
        Some(json!({ "content": "still readable" })),
    )
    .await;

    let (_, _, instance_admin) = seed_and_login(&app, &state, "exp-instance", true).await;
    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/admin/users/{subject_id}/data"),
        Some(&instance_admin),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE user_id = $1")
        .bind(subject_id)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(
        messages, 1,
        "hard-deleting one participant makes every conversation they were in unreadable"
    );

    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(subject_id)
        .fetch_one(&state.pool)
        .await
        .expect("email");
    assert!(email.ends_with("@invalid"), "the person is gone: {email}");

    let (status, _) = send(&app, "GET", "/api/users/me", Some(&subject), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "and their sessions with them"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn hard_delete_removes_the_messages(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "exp-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Export WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let (subject_id, subject_email) = seed(&state, "exp-subject", false).await;
    add_ws_member(&state, ws, subject_id, "member").await;
    let subject = login(&app, &subject_email, PASSWORD).await;
    send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&subject),
        Some(json!({ "content": "will be gone" })),
    )
    .await;

    let (_, _, instance_admin) = seed_and_login(&app, &state, "exp-instance", true).await;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/admin/users/{subject_id}/data"),
        Some(&instance_admin),
        Some(json!({ "hard_delete": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE user_id = $1")
        .bind(subject_id)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(messages, 0);

    let audited: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = 'user.data_erased'")
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(audited, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_admin_lists_the_workspaces_exports_and_nobody_elses(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "exp-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Export WS").await;
    let other = seed_workspace(&state, owner_id, "Other WS").await;

    let (admin_id, admin_email) = seed(&state, "exp-admin", false).await;
    add_ws_member(&state, ws, admin_id, "admin").await;
    let admin = login(&app, &admin_email, PASSWORD).await;
    let (member_id, member_email) = seed(&state, "exp-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member = login(&app, &member_email, PASSWORD).await;

    for target in [ws, other] {
        let (status, body) = send(
            &app,
            "POST",
            &format!("/api/workspaces/{target}/exports"),
            Some(&owner),
            Some(json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }
    run_pending_export(&state).await;

    let list = format!("/api/workspaces/{ws}/exports");
    let (status, body) = send(&app, "GET", &list, Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let rows = body["data"].as_array().expect("rows");
    assert_eq!(rows.len(), 1, "the other workspace's export is not here");
    assert_eq!(rows[0]["export"]["workspace_id"], json!(ws));

    let (status, body) = send(&app, "GET", &list, Some(&admin), None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let rows = body["data"].as_array().expect("rows");
    assert_eq!(rows.len(), 1, "an admin sees that it exists");
    assert!(
        rows[0]["download_url"].is_null(),
        "but the link goes only to whoever asked for it"
    );

    let (status, _) = send(&app, "GET", &list, Some(&member), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{other}/exports"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "not a member there");

    let (_, body) = send(&app, "GET", &list, Some(&owner), None).await;
    assert!(
        body["data"][0]["download_url"]
            .as_str()
            .is_some_and(|url| url.contains("/api/exports/download/")),
        "the requester gets the link once it is ready: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_person_export_an_admin_started_is_listed_with_the_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "exp-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Export WS").await;
    let (root_id, root_email) = seed(&state, "exp-root", true).await;
    add_ws_member(&state, ws, root_id, "admin").await;
    let root = login(&app, &root_email, PASSWORD).await;
    let (member_id, _) = seed(&state, "exp-subject", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let (stranger_id, _) = seed(&state, "exp-stranger", false).await;

    for subject in [member_id, stranger_id] {
        let (status, body) = send(
            &app,
            "POST",
            &format!("/api/users/{subject}/exports"),
            Some(&root),
            Some(json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/exports"),
        Some(&root),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let rows = body["data"].as_array().expect("rows");
    assert_eq!(rows.len(), 1, "only the member of this workspace: {body:?}");
    assert_eq!(rows[0]["export"]["subject_user_id"], json!(member_id));
}

async fn attach_file(
    state: &crate::state::AppState,
    ws: Uuid,
    user_id: Uuid,
    message_id: &str,
    name: &str,
    age_days: i32,
) {
    sqlx::query(
        "INSERT INTO files (message_id, user_id, workspace_id, filename, storage_key, mime_type, size_bytes, created_at)
         VALUES ($1::uuid, $2, $3, $4, $4, 'text/plain', 1, NOW() - make_interval(days => $5))",
    )
    .bind(message_id)
    .bind(user_id)
    .bind(ws)
    .bind(name)
    .bind(age_days)
    .execute(&state.pool)
    .await
    .expect("file");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn files_follow_the_same_dm_rule_and_dates_as_messages(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "exp-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Export WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;
    let (other_id, _) = seed(&state, "exp-other", false).await;
    add_ws_member(&state, ws, other_id, "member").await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/conversations"),
        Some(&owner),
        Some(json!({ "participant_ids": [other_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id").to_string();
    let (_, in_dm) = send(
        &app,
        "POST",
        &format!("/api/channels/{conv_id}/messages"),
        Some(&owner),
        Some(json!({ "content": "the contract, privately" })),
    )
    .await;
    let (_, in_channel) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "the plan" })),
    )
    .await;
    let in_dm = in_dm["id"].as_str().expect("id");
    let in_channel = in_channel["id"].as_str().expect("id");
    attach_file(&state, ws, owner_id, in_dm, "private.pdf", 0).await;
    attach_file(&state, ws, owner_id, in_channel, "plan.pdf", 0).await;
    attach_file(&state, ws, owner_id, in_channel, "old-plan.pdf", 30).await;

    send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/exports"),
        Some(&owner),
        Some(json!({})),
    )
    .await;
    let without = manifest_of(&state, run_pending_export(&state).await).await;
    assert_eq!(
        without["files"]["files.jsonl"]["rows"], 2,
        "a file posted in a DM is as private as the DM"
    );

    send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/exports"),
        Some(&owner),
        Some(json!({ "include_dms": true })),
    )
    .await;
    let with = manifest_of(&state, run_pending_export(&state).await).await;
    assert_eq!(with["files"]["files.jsonl"]["rows"], 3);

    let since = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/exports"),
        Some(&owner),
        Some(json!({ "since": since })),
    )
    .await;
    let recent = manifest_of(&state, run_pending_export(&state).await).await;
    assert_eq!(
        recent["files"]["files.jsonl"]["rows"], 1,
        "the date range applies to files too"
    );
}
