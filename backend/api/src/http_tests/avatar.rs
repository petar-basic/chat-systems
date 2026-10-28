use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::*;

const BOUNDARY: &str = "----avatartest";

const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

async fn upload_photo(app: &axum::Router, token: &str, bytes: &[u8]) -> (StatusCode, Value) {
    upload_photo_to(app, token, "/api/users/me/avatar", bytes).await
}

async fn upload_photo_to(
    app: &axum::Router,
    token: &str,
    uri: &str,
    bytes: &[u8],
) -> (StatusCode, Value) {
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"me.png\"\r\nContent-Type: image/png\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("send");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn download_path(url: &str) -> String {
    url[url.find("/api/files/download/").expect("download url")..].to_string()
}

async fn fetch(app: &axum::Router, path: &str, token: &str) -> StatusCode {
    let (status, _) = send(app, "GET", path, Some(token), None).await;
    status
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_account_photo_gets_its_own_place_and_replacing_it_removes_the_old_one(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "photo-own", false).await;

    let (status, me) = upload_photo(&app, &alice, PNG).await;
    assert_eq!(status, StatusCode::OK, "{me:?}");
    let first = me["avatar_url"].as_str().expect("avatar url").to_string();
    assert!(
        first.contains(&format!("/api/files/download/avatars/{alice_id}/")),
        "{first}"
    );
    assert_eq!(
        fetch(&app, &download_path(&first), &alice).await,
        StatusCode::OK
    );

    let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE user_id = $1")
        .bind(alice_id)
        .fetch_one(&state.pool)
        .await
        .expect("count files");
    assert_eq!(files, 0, "a photo is not a workspace file");

    let (status, me) = upload_photo(&app, &alice, PNG).await;
    assert_eq!(status, StatusCode::OK, "{me:?}");
    let second = me["avatar_url"].as_str().expect("avatar url").to_string();
    assert_ne!(first, second);
    assert_eq!(
        fetch(&app, &download_path(&first), &alice).await,
        StatusCode::NOT_FOUND,
        "the replaced photo is gone"
    );

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&alice),
        Some(json!({ "avatar_url": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        fetch(&app, &download_path(&second), &alice).await,
        StatusCode::NOT_FOUND,
        "a removed photo is gone"
    );

    let (status, _) = upload_photo(&app, &alice, b"not an image at all").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_photo_shows_in_every_shared_workspace_and_to_nobody_else(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "photo-alice", false).await;
    seed_workspace(&state, alice_id, "First").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "photo-bob", false).await;
    let second = seed_workspace(&state, bob_id, "Second").await;
    add_ws_member(&state, second, alice_id, "member").await;
    let (stranger_id, _, stranger) = seed_and_login(&app, &state, "photo-stranger", false).await;
    seed_workspace(&state, stranger_id, "Elsewhere").await;

    let (status, me) = upload_photo(&app, &alice, PNG).await;
    assert_eq!(status, StatusCode::OK, "{me:?}");
    let path = download_path(me["avatar_url"].as_str().expect("avatar url"));

    assert_eq!(
        fetch(&app, &path, &bob).await,
        StatusCode::OK,
        "bob shares only the second workspace and still sees it"
    );
    assert_eq!(
        fetch(&app, &path, &stranger).await,
        StatusCode::FORBIDDEN,
        "a stranger does not"
    );

    let members: Vec<Value> = {
        let (status, body) = send(
            &app,
            "GET",
            &format!("/api/workspaces/{second}/members"),
            Some(&bob),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body["data"].as_array().cloned().unwrap_or_default()
    };
    let shown = members
        .iter()
        .find(|m| m["user_id"] == json!(alice_id))
        .and_then(|m| m["avatar_url"].as_str())
        .expect("alice in the second workspace");
    assert_eq!(download_path(shown), path);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn retention_leaves_an_account_photo_alone(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "photo-kept", false).await;
    let ws_id = seed_workspace(&state, alice_id, "Short memory").await;
    state
        .retention_repo
        .upsert(
            ws_id,
            &crate::retention::repo::UpdateRetentionRequest {
                message_days: None,
                file_days: Some(1),
                audit_days: Some(730),
                notification_days: Some(90),
                channel_overrides: false,
            },
            alice_id,
        )
        .await
        .expect("set policy");

    let (status, me) = upload_photo(&app, &alice, PNG).await;
    assert_eq!(status, StatusCode::OK, "{me:?}");
    let path = download_path(me["avatar_url"].as_str().expect("avatar url"));

    let key = format!("{ws_id}/{}/old.txt", Uuid::new_v4());
    state
        .file_storage
        .upload(&key, b"old".to_vec(), "text/plain")
        .await
        .expect("store old file");
    sqlx::query(
        "INSERT INTO files (user_id, workspace_id, filename, storage_key, mime_type, size_bytes, created_at)
         VALUES ($1, $2, 'old.txt', $3, 'text/plain', 3, NOW() - interval '10 days')",
    )
    .bind(alice_id)
    .bind(ws_id)
    .bind(&key)
    .execute(&state.pool)
    .await
    .expect("old file row");

    let counts = crate::retention::job::run_once(&state).await;
    assert_eq!(counts.files, 1, "retention ran and took the old file");
    assert_eq!(
        fetch(&app, &path, &alice).await,
        StatusCode::OK,
        "the photo is still there"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_migration_moves_a_photo_out_of_its_workspace_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, _) = seed_and_login(&app, &state, "photo-moved", false).await;
    let first = seed_workspace(&state, alice_id, "Where it was uploaded").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "photo-viewer", false).await;
    let second = seed_workspace(&state, bob_id, "Somewhere else").await;
    add_ws_member(&state, second, alice_id, "member").await;

    let old_key = format!("{first}/{}/face.png", Uuid::new_v4());
    state
        .file_storage
        .upload(&old_key, PNG.to_vec(), "image/png")
        .await
        .expect("store old photo");
    let old_url = format!("/api/files/download/{old_key}");
    sqlx::query("UPDATE users SET avatar_url = $2 WHERE id = $1")
        .bind(alice_id)
        .bind(&old_url)
        .execute(&state.pool)
        .await
        .expect("old avatar");
    assert_eq!(fetch(&app, &old_url, &bob).await, StatusCode::NOT_FOUND);

    crate::profile::avatar::migrate(&state).await;

    let moved: String = sqlx::query_scalar("SELECT avatar_url FROM users WHERE id = $1")
        .bind(alice_id)
        .fetch_one(&state.pool)
        .await
        .expect("avatar url");
    assert!(
        moved.contains(&format!("/api/files/download/avatars/{alice_id}/")),
        "{moved}"
    );
    assert!(moved.ends_with(".png"), "{moved}");
    assert_eq!(
        fetch(&app, &download_path(&moved), &bob).await,
        StatusCode::OK,
        "the second workspace sees the moved photo"
    );

    crate::profile::avatar::migrate(&state).await;
    let again: String = sqlx::query_scalar("SELECT avatar_url FROM users WHERE id = $1")
        .bind(alice_id)
        .fetch_one(&state.pool)
        .await
        .expect("avatar url");
    assert_eq!(again, moved, "a second run has nothing to do");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_workspace_photo_outlives_retention_and_shows_to_that_workspace_only(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "ws-photo-alice", false).await;
    let ws_id = seed_workspace(&state, alice_id, "Short memory").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "ws-photo-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    let (carol_id, _, carol) = seed_and_login(&app, &state, "ws-photo-carol", false).await;
    let elsewhere = seed_workspace(&state, carol_id, "Elsewhere").await;
    add_ws_member(&state, elsewhere, alice_id, "member").await;
    state
        .retention_repo
        .upsert(
            ws_id,
            &crate::retention::repo::UpdateRetentionRequest {
                message_days: None,
                file_days: Some(1),
                audit_days: Some(730),
                notification_days: Some(90),
                channel_overrides: false,
            },
            alice_id,
        )
        .await
        .expect("set policy");

    let route = format!("/api/workspaces/{ws_id}/profile/avatar");
    let (status, profile) = upload_photo_to(&app, &alice, &route, PNG).await;
    assert_eq!(status, StatusCode::OK, "{profile:?}");
    let first = profile["avatar_url"]
        .as_str()
        .expect("avatar url")
        .to_string();
    assert!(
        first.contains(&format!("/api/files/download/avatars/{alice_id}/{ws_id}/")),
        "{first}"
    );
    let path = download_path(&first);
    let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE user_id = $1")
        .bind(alice_id)
        .fetch_one(&state.pool)
        .await
        .expect("count files");
    assert_eq!(files, 0, "a photo is not a workspace file");

    assert_eq!(
        fetch(&app, &path, &bob).await,
        StatusCode::OK,
        "a member sees it"
    );
    assert_eq!(
        fetch(&app, &path, &carol).await,
        StatusCode::FORBIDDEN,
        "somebody who shares another workspace with alice does not"
    );

    sqlx::query("UPDATE files SET created_at = NOW() - interval '10 days' WHERE workspace_id = $1")
        .bind(ws_id)
        .execute(&state.pool)
        .await
        .expect("age the workspace's files");
    crate::retention::job::run_once(&state).await;
    assert_eq!(
        fetch(&app, &path, &bob).await,
        StatusCode::OK,
        "retention leaves it alone"
    );

    let (status, profile) = upload_photo_to(&app, &alice, &route, PNG).await;
    assert_eq!(status, StatusCode::OK, "{profile:?}");
    assert_eq!(
        fetch(&app, &path, &alice).await,
        StatusCode::NOT_FOUND,
        "the replaced photo is gone"
    );
    let second = download_path(profile["avatar_url"].as_str().expect("avatar url"));

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}/profile"),
        Some(&alice),
        Some(json!({ "avatar_url": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        fetch(&app, &second, &alice).await,
        StatusCode::NOT_FOUND,
        "a removed photo is gone"
    );

    let (status, _) = upload_photo_to(&app, &carol, &route, PNG).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "only a member has a profile here"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_migration_moves_a_workspace_photo_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, _) = seed_and_login(&app, &state, "ws-photo-moved", false).await;
    let ws_id = seed_workspace(&state, alice_id, "Where it was uploaded").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "ws-photo-viewer", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;

    let old_key = format!("{ws_id}/{}/face.png", Uuid::new_v4());
    state
        .file_storage
        .upload(&old_key, PNG.to_vec(), "image/png")
        .await
        .expect("store old photo");
    let old_url = format!("/api/files/download/{old_key}");
    sqlx::query(
        "UPDATE workspace_members SET avatar_url = $3 WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(ws_id)
    .bind(alice_id)
    .bind(&old_url)
    .execute(&state.pool)
    .await
    .expect("old workspace photo");

    crate::profile::avatar::migrate(&state).await;

    let photo = || async {
        sqlx::query_scalar::<_, String>(
            "SELECT avatar_url FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
        )
        .bind(ws_id)
        .bind(alice_id)
        .fetch_one(&state.pool)
        .await
        .expect("avatar url")
    };
    let moved = photo().await;
    assert!(
        moved.contains(&format!("/api/files/download/avatars/{alice_id}/{ws_id}/")),
        "{moved}"
    );
    assert!(moved.ends_with(".png"), "{moved}");
    assert_eq!(
        fetch(&app, &download_path(&moved), &bob).await,
        StatusCode::OK
    );

    crate::profile::avatar::migrate(&state).await;
    assert_eq!(photo().await, moved, "a second run has nothing to do");
}
