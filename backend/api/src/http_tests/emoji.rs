use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::*;

const BOUNDARY: &str = "----emojitest";

/// A one-pixel PNG. Small enough to inline, real enough that the content type is
/// not a lie.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

fn multipart(name: &str, content_type: &str, image: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\n{name}\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"e.png\"\r\nContent-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(image);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

async fn upload(
    app: &axum::Router,
    token: &str,
    ws_id: Uuid,
    name: &str,
    content_type: &str,
    image: &[u8],
) -> (StatusCode, serde_json::Value) {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/workspaces/{ws_id}/emojis"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(multipart(name, content_type, image)))
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("send");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[sqlx::test(migrations = "../migrations")]
async fn an_emoji_uploads_once_and_lists_with_a_url(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "emoji-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Emoji WS").await;

    let (status, body) = upload(&app, &token, ws_id, "shipit", "image/png", PNG).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["name"], "shipit");
    assert!(body["url"].as_str().expect("url").contains("emoji/"));

    let (status, again) = upload(&app, &token, ws_id, "shipit", "image/png", PNG).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a shortcode is a name, not a list: {again:?}"
    );

    let (status, listed) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/emojis"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["data"].as_array().expect("data").len(), 1);
}

#[sqlx::test(migrations = "../migrations")]
async fn the_same_name_is_free_in_another_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "emoji-owner", false).await;
    let first = seed_workspace(&state, owner_id, "First").await;
    let second = seed_workspace(&state, owner_id, "Second").await;

    let (status, _) = upload(&app, &token, first, "shipit", "image/png", PNG).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = upload(&app, &token, second, "shipit", "image/png", PNG).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn an_emoji_is_not_a_file_share(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "emoji-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Emoji WS").await;

    let (status, _) = upload(
        &app,
        &token,
        ws_id,
        "toobig",
        "image/png",
        &vec![0u8; 300 * 1024],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "over the size limit"
    );

    let (status, _) = upload(&app, &token, ws_id, "notanimage", "application/pdf", PNG).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "not an image type"
    );

    let (status, _) = upload(&app, &token, ws_id, "smile", "image/png", PNG).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "shadowing a standard shortcode"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn removing_one_is_the_uploader_or_an_admin(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "emoji-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Emoji WS").await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "emoji-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let (other_id, _, other_token) = seed_and_login(&app, &state, "emoji-other", false).await;
    add_ws_member(&state, ws_id, other_id, "member").await;

    let (status, created) = upload(&app, &member_token, ws_id, "shipit", "image/png", PNG).await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    let emoji_id = created["id"].as_str().expect("id");

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}/emojis/{emoji_id}"),
        Some(&other_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "not yours and you are not an admin"
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}/emojis/{emoji_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an admin can remove shared vocabulary"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn emojis_belong_to_the_workspace_that_owns_them(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "emoji-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Emoji WS").await;
    let (outsider_id, _, outsider_token) = seed_and_login(&app, &state, "emoji-out", false).await;
    let _ = outsider_id;

    let (status, _) = upload(&app, &owner_token, ws_id, "shipit", "image/png", PNG).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/emojis"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/emojis"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
