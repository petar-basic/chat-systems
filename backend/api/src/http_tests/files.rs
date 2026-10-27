use super::common::*;
use crate::workspace::models::ChannelRole;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

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
    let uri = format!("/api/files/upload/{ws_id}?draft=true");
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn upload_without_token_is_unauthorized(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, _token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "files-upload-noauth-ws").await;

    let (status, _body) =
        upload_request(&app, ws_id, None, "hello.txt", "text/plain", b"hello").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn private_channel_attachment_is_gated_to_channel_members(pool: PgPool) {
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
        Some(serde_json::json!({ "content": "", "attachment_ids": [up_body[0]["id"]] })),
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

async fn upload_one(app: &axum::Router, token: &str, ws_id: Uuid, name: &str) -> (String, String) {
    let boundary = "X-BOUNDARY";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--{boundary}--\r\n"
    );
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/files/upload/{ws_id}?draft=true"))
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .header(
            axum::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(axum::body::Body::from(body))
        .unwrap();
    let response = tower::ServiceExt::oneshot(app.clone(), request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "upload should succeed");
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let key = value[0]["url"]
        .as_str()
        .expect("upload response carries a url")
        .split("/api/files/download/")
        .nth(1)
        .expect("url contains a storage key")
        .to_string();
    let id = value[0]["id"].as_str().expect("file id").to_string();
    (id, key)
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_dm_attachment_is_readable_by_participants_and_nobody_else(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "dm-file-alice", false).await;
    let ws_id = seed_workspace(&state, alice_id, "DM Files WS").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "dm-file-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    let (_carol_id, _, carol) = {
        let (id, _, token) = seed_and_login(&app, &state, "dm-file-carol", false).await;
        add_ws_member(&state, ws_id, id, "member").await;
        (id, (), token)
    };

    let (file_id, key) = upload_one(&app, &alice, ws_id, "secret.txt").await;

    let (status, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&alice),
        Some(json!({ "participant_ids": [bob_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create conversation: {conv:?}");
    let conv_id = conv["id"].as_str().expect("conversation id");

    let (status, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{conv_id}/messages"),
        Some(&alice),
        Some(json!({ "content": "here it is", "attachment_ids": [file_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "send dm: {msg:?}");

    let download = format!("/api/files/download/{key}");
    let (status, _b) = send(&app, "GET", &download, Some(&bob), None).await;
    assert_eq!(status, StatusCode::OK, "the other participant can read it");

    let (status, _b) = send(&app, "GET", &download, Some(&carol), None).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a workspace member who is not in the conversation must not read it"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_unposted_upload_belongs_to_whoever_uploaded_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "draft-alice", false).await;
    let ws_id = seed_workspace(&state, alice_id, "Draft Files WS").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "draft-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;

    let (_, key) = upload_one(&app, &alice, ws_id, "draft.txt").await;
    let download = format!("/api/files/download/{key}");

    let (status, _b) = send(&app, "GET", &download, Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK, "the uploader can read their draft");

    let (status, _b) = send(&app, "GET", &download, Some(&bob), None).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an upload nobody posted is not workspace-readable"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_avatar_stays_readable_to_the_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "avatar-alice", false).await;
    let ws_id = seed_workspace(&state, alice_id, "Avatar WS").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "avatar-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    let (_, _, outsider) = seed_and_login(&app, &state, "avatar-outsider", false).await;

    let (_, key) = upload_one(&app, &alice, ws_id, "face.png").await;
    let download = format!("/api/files/download/{key}");

    // Before it is anybody's avatar it is just a draft.
    let (status, _b) = send(&app, "GET", &download, Some(&bob), None).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an unposted upload stays private"
    );

    let (status, body) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&alice),
        Some(json!({ "avatar_url": download.clone() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "set avatar: {body:?}");

    let (status, _b) = send(&app, "GET", &download, Some(&bob), None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an avatar has to render for everyone who can see the person"
    );

    let (status, _b) = send(&app, "GET", &download, Some(&outsider), None).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "but not for somebody outside the workspace"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_upload_over_the_cap_is_refused_and_leaves_nothing_behind(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "cap-alice", false).await;
    let ws_id = seed_workspace(&state, alice_id, "Upload Cap WS").await;

    // `test_config` caps uploads at 4 KiB so this stays a unit-sized test.
    let oversized = vec![b'x'; (state.config.max_upload_bytes as usize) + 1];
    let boundary = "X-CAP-BOUNDARY";
    let body = multipart(boundary, "big.bin", "application/octet-stream", &oversized);
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/files/upload/{ws_id}"))
        .header(header::AUTHORIZATION, format!("Bearer {alice}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "a file over the cap must be refused"
    );

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE workspace_id = $1")
        .bind(ws_id)
        .fetch_one(&state.pool)
        .await
        .expect("count files");
    assert_eq!(rows, 0, "a refused upload must not leave a row behind");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_moderator_can_take_down_somebody_elses_file(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "mod-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Moderation WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (poster_id, poster_email) = seed(&state, "mod-poster", false).await;
    add_ws_member(&state, ws_id, poster_id, "member").await;
    let poster = login(&app, &poster_email, PASSWORD).await;

    let (status, uploaded) =
        upload_request(&app, ws_id, Some(&poster), "bad.txt", "text/plain", b"bad").await;
    assert_eq!(status, StatusCode::OK, "upload: {uploaded:?}");
    let file_id = uploaded[0]["id"].as_str().expect("file id").to_string();

    let (status, posted) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&poster),
        Some(json!({ "content": "here", "attachment_ids": [file_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let msg_id = posted["id"].as_str().expect("message id").to_string();

    let (bystander_id, bystander_email) = seed(&state, "mod-bystander", false).await;
    add_ws_member(&state, ws_id, bystander_id, "member").await;
    let bystander = login(&app, &bystander_email, PASSWORD).await;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/files/{file_id}"),
        Some(&bystander),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a plain member does not moderate files"
    );

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/files/{file_id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "workspace admin moderates: {body:?}"
    );

    let audited: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'file.deleted' AND resource_id = $1",
    )
    .bind(uuid::Uuid::parse_str(&file_id).expect("uuid"))
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(audited.0, 1, "taking down somebody's file is audited");

    let left: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT metadata->'attachments' FROM messages WHERE id = $1")
            .bind(uuid::Uuid::parse_str(&msg_id).expect("uuid"))
            .fetch_one(&state.pool)
            .await
            .expect("metadata");
    assert_eq!(left, None, "the message no longer shows the file");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn deleting_a_message_takes_its_attachment_with_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "cascade-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Cascade WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (status, uploaded) =
        upload_request(&app, ws_id, Some(&owner), "doc.txt", "text/plain", b"doc").await;
    assert_eq!(status, StatusCode::OK, "upload: {uploaded:?}");
    let url = uploaded[0]["url"].as_str().expect("url").to_string();
    let key = storage_key_from_url(&url);

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "see", "attachment_ids": [uploaded[0]["id"]] })),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("message id").to_string();

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE storage_key = $1")
        .bind(&key)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(rows, 0, "the attachment goes with the message");

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/files/download/{key}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a deleted attachment must not stay downloadable by key"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn editing_a_message_releases_the_attachment_it_drops(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "release-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Release WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (_, kept) =
        upload_request(&app, ws_id, Some(&owner), "keep.txt", "text/plain", b"keep").await;
    let (_, dropped) =
        upload_request(&app, ws_id, Some(&owner), "drop.txt", "text/plain", b"drop").await;
    let kept_url = kept[0]["url"].as_str().expect("url").to_string();
    let dropped_url = dropped[0]["url"].as_str().expect("url").to_string();
    let dropped_key = storage_key_from_url(&dropped_url);
    let kept_key = storage_key_from_url(&kept_url);

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(
            json!({ "content": "two files", "attachment_ids": [kept[0]["id"], dropped[0]["id"]] }),
        ),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("message id").to_string();
    assert_eq!(
        msg["metadata"]["attachments"].as_array().map(Vec::len),
        Some(2)
    );

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/messages/{msg_id}"),
        Some(&owner),
        Some(json!({ "content": "one file", "attachment_ids": [kept[0]["id"]] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "edit: {body:?}");
    assert_eq!(body["metadata"]["attachments"][0]["id"], kept[0]["id"]);
    assert_eq!(
        body["metadata"]["attachments"].as_array().map(Vec::len),
        Some(1)
    );

    let dropped_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE storage_key = $1")
        .bind(&dropped_key)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(dropped_rows, 0, "an unlinked attachment is released");

    let kept_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE storage_key = $1")
        .bind(&kept_key)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(kept_rows, 1, "the attachment still referenced stays");
}

async fn draft(app: &axum::Router, ws_id: Uuid, token: &str, name: &str) -> serde_json::Value {
    let (status, body) =
        upload_request(app, ws_id, Some(token), name, "text/plain", name.as_bytes()).await;
    assert_eq!(status, StatusCode::OK, "upload: {body:?}");
    body[0]["id"].clone()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn files_and_text_go_out_as_one_message(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "one-msg-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "One Message WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "main", false).await;
    let first = draft(&app, ws_id, &owner, "a.txt").await;
    let second = draft(&app, ws_id, &owner, "b.txt").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "  " })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "no text and no files"
    );

    let (status, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "", "attachment_ids": [second, first] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{msg:?}");
    let names: Vec<&str> = msg["metadata"]["attachments"]
        .as_array()
        .expect("attachments")
        .iter()
        .map(|a| a["filename"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        vec!["b.txt", "a.txt"],
        "in the order they were attached"
    );

    let (_, page) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(
        page["data"][0]["metadata"]["attachments"]
            .as_array()
            .map(Vec::len),
        Some(2),
        "a listed message carries its files"
    );

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "again", "attachment_ids": [first] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a sent file cannot be attached a second time"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_your_own_unsent_drafts_can_be_attached(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "own-drafts-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Own Drafts WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "main", false).await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "own-drafts-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, bob_id, &ChannelRole::Member)
        .await
        .expect("join");

    let owners = draft(&app, ws_id, &owner, "mine.txt").await;
    let post = |token: String, body: serde_json::Value| {
        let app = app.clone();
        async move {
            send(
                &app,
                "POST",
                &format!("/api/channels/{ch}/messages"),
                Some(&token),
                Some(body),
            )
            .await
            .0
        }
    };

    assert_eq!(
        post(
            bob.clone(),
            json!({ "content": "x", "attachment_ids": [owners] })
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "somebody else's draft"
    );
    assert_eq!(
        post(
            owner.clone(),
            json!({ "content": "x", "attachment_ids": [owners, owners] })
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the same file twice"
    );
    let eleven: Vec<serde_json::Value> = (0..11).map(|_| json!(Uuid::new_v4())).collect();
    assert_eq!(
        post(
            owner.clone(),
            json!({ "content": "x", "attachment_ids": eleven })
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "more than ten"
    );

    let boundary = "plain-upload";
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/files/upload/{ws_id}"))
        .header(header::AUTHORIZATION, format!("Bearer {owner}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(multipart(
            boundary,
            "avatar.png",
            "image/png",
            b"png",
        )))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let plain: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        post(
            owner.clone(),
            json!({ "content": "x", "attachment_ids": [plain[0]["id"]] })
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "an upload that is not a draft — an avatar, an emoji — is not an attachment"
    );

    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE message_id IS NOT NULL")
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(left, 0, "a refused send links nothing");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn forwarding_copies_the_files_into_the_new_message(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "fwd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Forward WS").await;
    let source_ch = seed_channel(&state, ws_id, owner_id, "source", true).await;
    let target_ch = seed_channel(&state, ws_id, owner_id, "target", false).await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "fwd-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(target_ch, bob_id, &ChannelRole::Member)
        .await
        .expect("join");

    let file = draft(&app, ws_id, &owner, "plan.txt").await;
    let (_, original) = send(
        &app,
        "POST",
        &format!("/api/channels/{source_ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "the plan", "attachment_ids": [file] })),
    )
    .await;
    let original_id = original["id"].as_str().expect("id").to_string();

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{target_ch}/messages"),
        Some(&bob),
        Some(json!({ "content": "fwd", "forwarded_from": original_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "forwarding needs read access to the original"
    );

    let (status, forwarded) = send(
        &app,
        "POST",
        &format!("/api/channels/{target_ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "", "forwarded_from": original_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{forwarded:?}");
    assert_eq!(
        forwarded["metadata"]["forwarded_from"]["message_id"],
        original_id
    );
    let copy = &forwarded["metadata"]["attachments"][0];
    assert_eq!(copy["filename"], "plan.txt");
    assert_ne!(copy["id"], file, "a copy, not the same file");
    assert_ne!(
        copy["url"], original["metadata"]["attachments"][0]["url"],
        "stored under its own key"
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{original_id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let url = copy["url"].as_str().expect("url");
    let download = &url[url.find("/api/files/download/").expect("marker")..];
    let (status, _) = send(&app, "GET", download, Some(&bob), None).await;
    assert_eq!(status, StatusCode::OK, "the forward keeps its own copy");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn abandoned_drafts_are_swept(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "sweep-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Sweep WS").await;
    let stale = draft(&app, ws_id, &owner, "stale.txt").await;
    let fresh = draft(&app, ws_id, &owner, "fresh.txt").await;
    sqlx::query("UPDATE files SET draft_until = NOW() - INTERVAL '1 minute' WHERE id = $1")
        .bind(Uuid::parse_str(stale.as_str().expect("id")).expect("uuid"))
        .execute(&state.pool)
        .await
        .expect("age");

    assert_eq!(crate::files::service::sweep_expired_drafts(&state).await, 1);
    let left: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM files")
        .fetch_all(&state.pool)
        .await
        .expect("files");
    assert_eq!(
        left,
        vec![Uuid::parse_str(fresh.as_str().expect("id")).expect("uuid")]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_scheduled_message_keeps_its_files_until_it_is_sent(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "sched-files-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Scheduled Files WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "main", false).await;
    let file = draft(&app, ws_id, &owner, "later.txt").await;
    let send_at = chrono::Utc::now() + chrono::Duration::days(3);

    let (status, scheduled) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&owner),
        Some(json!({ "channel_id": ch, "content": "", "send_at": send_at, "attachment_ids": [file] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{scheduled:?}");
    let held: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT draft_until FROM files WHERE id = $1")
            .bind(Uuid::parse_str(file.as_str().expect("id")).expect("uuid"))
            .fetch_one(&state.pool)
            .await
            .expect("draft_until");
    assert!(held > send_at, "the draft outlives the send time");

    sqlx::query("UPDATE scheduled_messages SET send_at = NOW() - INTERVAL '1 second'")
        .execute(&state.pool)
        .await
        .expect("make due");
    for due in state.scheduled_repo.claim_due().await.expect("claim") {
        crate::scheduled::executor::deliver_now(&state, &due)
            .await
            .expect("deliver");
    }
    let (_, page) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(
        page["data"][0]["metadata"]["attachments"][0]["filename"],
        "later.txt"
    );
}

fn phone_photo(width: u32, height: u32) -> Vec<u8> {
    use img_parts::ImageEXIF;
    let image = image::RgbImage::from_pixel(width, height, image::Rgb([20, 120, 220]));
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .expect("encode");
    let mut exif = Vec::new();
    exif.extend_from_slice(b"II");
    exif.extend_from_slice(&42u16.to_le_bytes());
    exif.extend_from_slice(&8u32.to_le_bytes());
    exif.extend_from_slice(&2u16.to_le_bytes());
    exif.extend_from_slice(&0x0112u16.to_le_bytes());
    exif.extend_from_slice(&3u16.to_le_bytes());
    exif.extend_from_slice(&1u32.to_le_bytes());
    exif.extend_from_slice(&6u16.to_le_bytes());
    exif.extend_from_slice(&[0, 0]);
    exif.extend_from_slice(&0x010Eu16.to_le_bytes());
    exif.extend_from_slice(&2u16.to_le_bytes());
    exif.extend_from_slice(&(PHOTO_SECRET.len() as u32).to_le_bytes());
    exif.extend_from_slice(&38u32.to_le_bytes());
    exif.extend_from_slice(&0u32.to_le_bytes());
    exif.extend_from_slice(PHOTO_SECRET);
    let mut jpeg = img_parts::jpeg::Jpeg::from_bytes(out.into_inner().into()).expect("jpeg");
    jpeg.set_exif(Some(exif.into()));
    jpeg.encoder().bytes().to_vec()
}

const PHOTO_SECRET: &[u8] = b"GPS 44.8125N 20.4612E";

async fn raw_get(
    app: &axum::Router,
    uri: &str,
    token: &str,
) -> (StatusCode, header::HeaderMap, Vec<u8>) {
    let request = Request::builder()
        .method("GET")
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, headers, bytes)
}

fn download_path(url: &str) -> &str {
    &url[url.find("/api/files/download/").expect("marker")..]
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_phone_photo_is_stored_upright_without_its_location_and_with_a_thumbnail(pool: PgPool) {
    let mut config = test_config();
    config.max_upload_bytes = 4 * 1024 * 1024;
    let (app, state) = app_and_state_with(pool, config).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "photo-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Photo WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "private-photos", true).await;
    let (_, _, outsider) = {
        let (id, _, token) = seed_and_login(&app, &state, "photo-outsider", false).await;
        add_ws_member(&state, ws_id, id, "member").await;
        (id, (), token)
    };

    let photo = phone_photo(1600, 900);
    let (status, uploaded) = upload_request(
        &app,
        ws_id,
        Some(&owner),
        "IMG_0042.jpg",
        "application/octet-stream",
        &photo,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{uploaded:?}");
    let file = &uploaded[0];
    assert_eq!(
        file["mime_type"], "image/jpeg",
        "the type comes from the bytes"
    );
    assert_eq!(
        (file["width"].as_i64(), file["height"].as_i64()),
        (Some(900), Some(1600)),
        "the size is the upright one"
    );
    let thumbnail_url = file["thumbnail_url"]
        .as_str()
        .expect("a thumbnail")
        .to_string();

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(json!({ "content": "", "attachment_ids": [file["id"]] })),
    )
    .await;
    assert_eq!(msg["metadata"]["attachments"][0]["width"], 900);
    assert_eq!(
        msg["metadata"]["attachments"][0]["thumbnail_url"],
        thumbnail_url
    );

    let original_url = file["url"].as_str().expect("url");
    let (status, headers, original) = raw_get(&app, download_path(original_url), &owner).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !original
            .windows(PHOTO_SECRET.len())
            .any(|w| w == PHOTO_SECRET),
        "the location never reaches storage"
    );
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "private, max-age=31536000, immutable"
    );
    let upright = image::load_from_memory(&original).expect("still an image");
    assert_eq!(
        (upright.width(), upright.height()),
        (1600, 900),
        "pixels untouched"
    );

    let (status, _, thumbnail) = raw_get(&app, download_path(&thumbnail_url), &owner).await;
    assert_eq!(status, StatusCode::OK);
    let thumbnail = image::load_from_memory(&thumbnail).expect("thumbnail");
    assert_eq!((thumbnail.width(), thumbnail.height()), (720, 1280));

    let (status, _, _) = raw_get(&app, download_path(&thumbnail_url), &outsider).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a thumbnail is as private as its file"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_small_image_is_its_own_thumbnail_and_a_pdf_is_left_alone(pool: PgPool) {
    let mut config = test_config();
    config.max_upload_bytes = 4 * 1024 * 1024;
    let (app, state) = app_and_state_with(pool, config).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "small-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Small WS").await;

    let (_, small) = upload_request(
        &app,
        ws_id,
        Some(&owner),
        "icon.jpg",
        "image/jpeg",
        &phone_photo(64, 32),
    )
    .await;
    assert!(small[0]["thumbnail_url"].is_null());
    assert_eq!(small[0]["width"], 32);

    let (_, pdf) = upload_request(
        &app,
        ws_id,
        Some(&owner),
        "plan.pdf",
        "application/pdf",
        b"%PDF-1.7 not really",
    )
    .await;
    assert_eq!(pdf[0]["mime_type"], "application/pdf");
    assert!(pdf[0]["width"].is_null());

    let (_, broken) = upload_request(
        &app,
        ws_id,
        Some(&owner),
        "broken.jpg",
        "image/jpeg",
        b"\xFF\xD8\xFF\xE0 this is not a real jpeg",
    )
    .await;
    assert_eq!(
        broken[0]["size_bytes"], 28,
        "an image that does not decode is still stored as it came"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn deleting_an_image_takes_its_thumbnail_with_it(pool: PgPool) {
    let mut config = test_config();
    config.max_upload_bytes = 4 * 1024 * 1024;
    let (app, state) = app_and_state_with(pool, config).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "thumb-del-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Thumb Delete WS").await;

    let (_, uploaded) = upload_request(
        &app,
        ws_id,
        Some(&owner),
        "wide.jpg",
        "image/jpeg",
        &phone_photo(1600, 900),
    )
    .await;
    let thumbnail_key = uploaded[0]["thumbnail_url"]
        .as_str()
        .expect("thumbnail")
        .split("/api/files/download/")
        .nth(1)
        .expect("key")
        .to_string();
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/files/{}", uploaded[0]["id"].as_str().expect("id")),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        state.file_storage.download(&thumbnail_key).await.is_err(),
        "the thumbnail object is gone too"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_file_keeps_the_name_it_was_given(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "names-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Names WS").await;

    let (status, uploaded) = upload_request(
        &app,
        ws_id,
        Some(&owner),
        "Izveštaj o radu.pdf",
        "application/pdf",
        b"%PDF-1.7",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{uploaded:?}");
    assert_eq!(uploaded[0]["filename"], "Izveštaj o radu.pdf");
    let url = uploaded[0]["url"].as_str().expect("url");
    let key = storage_key_from_url(url);
    assert!(
        key.ends_with("/Izve_taj_o_radu.pdf"),
        "the storage key stays ASCII: {key}"
    );

    let (status, headers, _) = raw_get(&app, download_path(url), &owner).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"Izve_taj o radu.pdf\"; filename*=UTF-8''Izve%C5%A1taj%20o%20radu.pdf"
    );

    let (_, traversal) = upload_request(
        &app,
        ws_id,
        Some(&owner),
        "../../etc/passwd",
        "text/plain",
        b"x",
    )
    .await;
    assert_eq!(traversal[0]["filename"], "passwd");
}

async fn ranged_get(
    app: &axum::Router,
    uri: &str,
    token: &str,
    range: Option<&str>,
) -> (StatusCode, header::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder()
        .method("GET")
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    if let Some(range) = range {
        builder = builder.header(header::RANGE, range);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, headers, bytes)
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_download_serves_the_range_it_is_asked_for(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "range-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Range WS").await;
    let (_, uploaded) = upload_request(
        &app,
        ws_id,
        Some(&owner),
        "clip.mp4",
        "video/mp4",
        b"0123456789",
    )
    .await;
    let path = download_path(uploaded[0]["url"].as_str().expect("url")).to_string();

    let (status, headers, body) = ranged_get(&app, &path, &owner, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"0123456789");
    assert_eq!(headers[header::ACCEPT_RANGES], "bytes");
    assert_eq!(headers[header::CONTENT_LENGTH], "10");

    for (range, expected, content_range) in [
        ("bytes=2-5", &b"2345"[..], "bytes 2-5/10"),
        ("bytes=7-", &b"789"[..], "bytes 7-9/10"),
        ("bytes=-3", &b"789"[..], "bytes 7-9/10"),
        ("bytes=8-50", &b"89"[..], "bytes 8-9/10"),
    ] {
        let (status, headers, body) = ranged_get(&app, &path, &owner, Some(range)).await;
        assert_eq!(status, StatusCode::PARTIAL_CONTENT, "{range}");
        assert_eq!(body, expected, "{range}");
        assert_eq!(headers[header::CONTENT_RANGE], content_range, "{range}");
        assert_eq!(
            headers[header::CONTENT_LENGTH],
            expected.len().to_string().as_str(),
            "{range}"
        );
    }

    let (status, headers, body) = ranged_get(&app, &path, &owner, Some("bytes=10-")).await;
    assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(headers[header::CONTENT_RANGE], "bytes */10");
    assert!(body.is_empty());

    let (status, _, body) = ranged_get(&app, &path, &owner, Some("bytes=0-1,4-5")).await;
    assert_eq!(status, StatusCode::OK, "several ranges get the whole file");
    assert_eq!(body, b"0123456789");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_lists_its_files_by_kind_newest_first(pool: PgPool) {
    let mut config = test_config();
    config.max_upload_bytes = 4 * 1024 * 1024;
    let (app, state) = app_and_state_with(pool, config).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "chfiles-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Channel Files WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "design", true).await;
    let (_, _, outsider) = {
        let (id, _, token) = seed_and_login(&app, &state, "chfiles-outsider", false).await;
        add_ws_member(&state, ws_id, id, "member").await;
        (id, (), token)
    };

    let mut post = Vec::new();
    for (name, ctype, bytes) in [
        ("first.pdf", "application/pdf", b"%PDF-1.7".to_vec()),
        ("shot.jpg", "image/jpeg", phone_photo(40, 20)),
        ("clip.mp4", "video/mp4", b"not really a video".to_vec()),
        ("notes.txt", "text/plain", b"notes".to_vec()),
    ] {
        let (_, uploaded) = upload_request(&app, ws_id, Some(&owner), name, ctype, &bytes).await;
        let (_, msg) = send(
            &app,
            "POST",
            &format!("/api/channels/{ch}/messages"),
            Some(&owner),
            Some(json!({ "content": name, "attachment_ids": [uploaded[0]["id"]] })),
        )
        .await;
        post.push(msg["id"].as_str().expect("id").to_string());
    }
    let list = |query: &str| {
        let app = app.clone();
        let token = owner.clone();
        let uri = format!("/api/channels/{ch}/files{query}");
        async move {
            let (status, body) = send(&app, "GET", &uri, Some(&token), None).await;
            assert_eq!(status, StatusCode::OK, "{body:?}");
            body["data"]
                .as_array()
                .expect("data")
                .iter()
                .map(|f| f["filename"].as_str().expect("name").to_string())
                .collect::<Vec<_>>()
        }
    };

    assert_eq!(
        list("").await,
        vec!["notes.txt", "clip.mp4", "shot.jpg", "first.pdf"]
    );
    assert_eq!(list("?kind=image").await, vec!["shot.jpg"]);
    assert_eq!(list("?kind=video").await, vec!["clip.mp4"]);
    assert_eq!(list("?kind=document").await, vec!["notes.txt", "first.pdf"]);

    let (_, page) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/files?limit=2"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(page["data"][0]["message_id"], json!(post[3]));
    let cursor = page["data"][1]["id"].as_str().expect("id");
    assert_eq!(
        list(&format!("?limit=2&cursor={cursor}")).await,
        vec!["shot.jpg", "first.pdf"]
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{}", post[0]),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list("?kind=document").await, vec!["notes.txt"]);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/files"),
        Some(&outsider),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a private channel's files stay private"
    );
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/files"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{}/files", Uuid::new_v4()),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/files?kind=spreadsheet"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
