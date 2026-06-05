use super::common::*;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use sqlx::PgPool;
use tower::ServiceExt;

fn multipart(boundary: &str, filename: &str, ctype: &str, data: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {ctype}\r\n\r\n"
        )
        .as_bytes(),
    );
    b.extend_from_slice(data);
    b.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    b
}

const BOUNDARY: &str = "----testboundaryXYZ";

async fn upload_request(
    app: &axum::Router,
    ws_id: uuid::Uuid,
    token: Option<&str>,
    filename: &str,
    ctype: &str,
    data: &[u8],
) -> (StatusCode, serde_json::Value) {
    let body = multipart(BOUNDARY, filename, ctype, data);
    let uri = format!("/api/files/upload/{ws_id}");
    let mut builder = Request::builder().method("POST").uri(uri).header(
        header::CONTENT_TYPE,
        format!("multipart/form-data; boundary={BOUNDARY}"),
    );
    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let request = builder.body(Body::from(body)).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, value)
}

async fn raw_get_status(app: &axum::Router, uri: &str, token: Option<&str>) -> StatusCode {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let request = builder.body(Body::empty()).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    response.status()
}

fn storage_key_from_url(url: &str) -> String {
    let marker = "/api/files/download/";
    let idx = url.find(marker).expect("download marker in url") + marker.len();
    url[idx..].to_string()
}

#[sqlx::test(migrations = "../migrations")]
async fn upload_as_member_returns_file_id(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-upload-ws").await;

    let (status, body) = upload_request(
        &app,
        ws_id,
        Some(&token),
        "hello.txt",
        "text/plain",
        b"hello world",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "upload as member: {body:?}");
    let first = &body[0];
    assert!(first["id"].is_string(), "file id present: {body:?}");
    assert!(first["url"].is_string(), "url present: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn upload_without_token_is_unauthorized(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, _token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-upload-noauth-ws").await;

    let (status, _body) =
        upload_request(&app, ws_id, None, "hello.txt", "text/plain", b"hello").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn upload_to_workspace_not_member_is_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _oemail, _otoken) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-upload-foreign-ws").await;
    let (_outsider_id, _email, outsider_token) =
        seed_and_login(&app, &state, "outsider", false).await;

    let (status, _body) = upload_request(
        &app,
        ws_id,
        Some(&outsider_token),
        "hello.txt",
        "text/plain",
        b"hello",
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn download_by_storage_key_as_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-download-ws").await;

    let (status, body) = upload_request(
        &app,
        ws_id,
        Some(&token),
        "doc.txt",
        "text/plain",
        b"payload",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "upload: {body:?}");
    let key = storage_key_from_url(body[0]["url"].as_str().expect("url"));

    let uri = format!("/api/files/download/{key}");
    let dl_status = raw_get_status(&app, &uri, Some(&token)).await;
    assert_eq!(dl_status, StatusCode::OK, "download key={key}");
}

#[sqlx::test(migrations = "../migrations")]
async fn download_without_token_is_unauthorized(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-download-noauth-ws").await;

    let (status, body) = upload_request(
        &app,
        ws_id,
        Some(&token),
        "doc.txt",
        "text/plain",
        b"payload",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "upload: {body:?}");
    let key = storage_key_from_url(body[0]["url"].as_str().expect("url"));

    let uri = format!("/api/files/download/{key}");
    let dl_status = raw_get_status(&app, &uri, None).await;
    assert_eq!(dl_status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn download_cross_tenant_is_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _oemail, owner_token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-download-cross-ws").await;

    let (status, body) = upload_request(
        &app,
        ws_id,
        Some(&owner_token),
        "secret.txt",
        "text/plain",
        b"secret",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "upload: {body:?}");
    let key = storage_key_from_url(body[0]["url"].as_str().expect("url"));

    let (_outsider_id, _email, outsider_token) =
        seed_and_login(&app, &state, "outsider", false).await;
    let uri = format!("/api/files/download/{key}");
    let dl_status = raw_get_status(&app, &uri, Some(&outsider_token)).await;
    assert_eq!(dl_status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn get_file_meta_as_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-meta-ws").await;

    let (status, body) =
        upload_request(&app, ws_id, Some(&token), "meta.txt", "text/plain", b"meta").await;
    assert_eq!(status, StatusCode::OK, "upload: {body:?}");
    let file_id = body[0]["id"].as_str().expect("file id").to_string();

    let (meta_status, meta) = send(
        &app,
        "GET",
        &format!("/api/files/{file_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(meta_status, StatusCode::OK, "get meta: {meta:?}");
    assert!(meta["file"].is_object(), "file object present: {meta:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn get_file_meta_without_token_is_unauthorized(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-meta-noauth-ws").await;

    let (status, body) =
        upload_request(&app, ws_id, Some(&token), "meta.txt", "text/plain", b"meta").await;
    assert_eq!(status, StatusCode::OK, "upload: {body:?}");
    let file_id = body[0]["id"].as_str().expect("file id").to_string();

    let (meta_status, _meta) =
        send(&app, "GET", &format!("/api/files/{file_id}"), None, None).await;
    assert_eq!(meta_status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn get_file_meta_cross_tenant_is_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _oemail, owner_token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-meta-cross-ws").await;

    let (status, body) = upload_request(
        &app,
        ws_id,
        Some(&owner_token),
        "meta.txt",
        "text/plain",
        b"meta",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "upload: {body:?}");
    let file_id = body[0]["id"].as_str().expect("file id").to_string();

    let (_outsider_id, _email, outsider_token) =
        seed_and_login(&app, &state, "outsider", false).await;
    let (meta_status, _meta) = send(
        &app,
        "GET",
        &format!("/api/files/{file_id}"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(meta_status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn get_file_meta_missing_is_not_found(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;

    let missing = uuid::Uuid::new_v4();
    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/files/{missing}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_workspace_files_as_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-list-ws").await;

    let (up_status, up_body) =
        upload_request(&app, ws_id, Some(&token), "a.txt", "text/plain", b"a").await;
    assert_eq!(up_status, StatusCode::OK, "upload: {up_body:?}");

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/files/workspace/{ws_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "list: {body:?}");
    let data = body["data"].as_array().expect("data array");
    assert_eq!(data.len(), 1, "one uploaded file listed: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn list_workspace_files_without_token_is_unauthorized(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, _token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-list-noauth-ws").await;

    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/files/workspace/{ws_id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_workspace_files_cross_tenant_is_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _oemail, _otoken) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-list-cross-ws").await;

    let (_outsider_id, _email, outsider_token) =
        seed_and_login(&app, &state, "outsider", false).await;
    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/files/workspace/{ws_id}"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_own_file_succeeds(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-delete-own-ws").await;

    let (up_status, up_body) =
        upload_request(&app, ws_id, Some(&token), "del.txt", "text/plain", b"del").await;
    assert_eq!(up_status, StatusCode::OK, "upload: {up_body:?}");
    let file_id = up_body[0]["id"].as_str().expect("file id").to_string();

    let (status, _body) = send(
        &app,
        "DELETE",
        &format!("/api/files/{file_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (after_status, _after) = send(
        &app,
        "GET",
        &format!("/api/files/{file_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(after_status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_someone_elses_file_is_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, owner_token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-delete-other-ws").await;

    let (other_id, _oemail, other_token) = seed_and_login(&app, &state, "member", false).await;
    add_ws_member(&state, ws_id, other_id, "member").await;

    let (up_status, up_body) = upload_request(
        &app,
        ws_id,
        Some(&owner_token),
        "owned.txt",
        "text/plain",
        b"owned",
    )
    .await;
    assert_eq!(up_status, StatusCode::OK, "upload: {up_body:?}");
    let file_id = up_body[0]["id"].as_str().expect("file id").to_string();

    let (status, _body) = send(
        &app,
        "DELETE",
        &format!("/api/files/{file_id}"),
        Some(&other_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_without_token_is_unauthorized(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-delete-noauth-ws").await;

    let (up_status, up_body) =
        upload_request(&app, ws_id, Some(&token), "del.txt", "text/plain", b"del").await;
    assert_eq!(up_status, StatusCode::OK, "upload: {up_body:?}");
    let file_id = up_body[0]["id"].as_str().expect("file id").to_string();

    let (status, _body) = send(&app, "DELETE", &format!("/api/files/{file_id}"), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_missing_file_is_not_found(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;

    let missing = uuid::Uuid::new_v4();
    let (status, _body) = send(
        &app,
        "DELETE",
        &format!("/api/files/{missing}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../migrations")]
async fn private_channel_attachment_is_gated_to_channel_members(pool: PgPool) {
    use crate::workspace::models::ChannelRole;

    let (app, state) = app_and_state(pool).await;
    let (owner_id, _oemail, owner_token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-private-ws").await;

    let (member_id, _memail, member_token) = seed_and_login(&app, &state, "wsmember", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let (chmem_id, _cemail, chmem_token) = seed_and_login(&app, &state, "chmember", false).await;
    add_ws_member(&state, ws_id, chmem_id, "member").await;

    let ch_id = seed_channel(&state, ws_id, owner_id, "secret", true).await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, chmem_id, &ChannelRole::Member)
        .await
        .expect("add channel member");

    let (up_status, up_body) = upload_request(
        &app,
        ws_id,
        Some(&owner_token),
        "leak.txt",
        "text/plain",
        b"top secret",
    )
    .await;
    assert_eq!(up_status, StatusCode::OK, "upload: {up_body:?}");
    let url = up_body[0]["url"].as_str().expect("url").to_string();
    let key = storage_key_from_url(&url);

    let (send_status, send_body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&owner_token),
        Some(serde_json::json!({ "content": format!("[file: leak.txt]({url})") })),
    )
    .await;
    assert_eq!(send_status, StatusCode::OK, "send: {send_body:?}");

    let dl_uri = format!("/api/files/download/{key}");

    let owner_dl = raw_get_status(&app, &dl_uri, Some(&owner_token)).await;
    assert_eq!(
        owner_dl,
        StatusCode::OK,
        "uploader/channel-admin can download"
    );

    let chmem_dl = raw_get_status(&app, &dl_uri, Some(&chmem_token)).await;
    assert_eq!(chmem_dl, StatusCode::OK, "channel member can download");

    let member_dl = raw_get_status(&app, &dl_uri, Some(&member_token)).await;
    assert_eq!(
        member_dl,
        StatusCode::FORBIDDEN,
        "workspace member outside the private channel must be denied"
    );
}
