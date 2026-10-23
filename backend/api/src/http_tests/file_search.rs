use super::common::*;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const BOUNDARY: &str = "----filesearchboundary";

async fn upload(app: &axum::Router, ws_id: Uuid, token: &str, name: &str, ctype: &str) -> Value {
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: {ctype}\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(b"contents");
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/files/upload/{ws_id}?draft=true"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "upload {name}");
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    value[0]["id"].clone()
}

async fn post_file(
    app: &axum::Router,
    ws_id: Uuid,
    channel_id: &str,
    token: &str,
    name: &str,
    ctype: &str,
) -> String {
    let file_id = upload(app, ws_id, token, name, ctype).await;
    let (status, msg) = send(
        app,
        "POST",
        &format!("/api/channels/{channel_id}/messages"),
        Some(token),
        Some(json!({ "content": "", "attachment_ids": [file_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "post {name}: {msg:?}");
    msg["id"].as_str().expect("message id").to_string()
}

async fn found(app: &axum::Router, token: &str, ws_id: Uuid, query: &str) -> Vec<String> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/search/files?workspace_id={ws_id}&{query}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{query}: {body:?}");
    body["data"]
        .as_array()
        .expect("data")
        .iter()
        .map(|hit| hit["filename"].as_str().expect("filename").to_string())
        .collect()
}

fn q(term: &str) -> String {
    format!("q={}", urlencode(term))
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_file_is_found_by_any_fragment_of_its_name(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "fsearch-frag", false).await;
    let ws_id = seed_workspace(&state, owner_id, "File Search WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "finance", false)
        .await
        .to_string();
    let message_id = post_file(
        &app,
        ws_id,
        &ch,
        &owner,
        "Q3 budget.xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    )
    .await;
    post_file(
        &app,
        ws_id,
        &ch,
        &owner,
        "Izveštaj_final.pdf",
        "application/pdf",
    )
    .await;
    post_file(&app, ws_id, &ch, &owner, "budget-2025.csv", "text/csv").await;

    for (term, first) in [
        ("budg", "budget-2025.csv"),
        ("BUDG", "budget-2025.csv"),
        ("q3 bud", "Q3 budget.xlsx"),
        ("Q3 budget.xlsx", "Q3 budget.xlsx"),
    ] {
        assert_eq!(
            found(&app, &owner, ws_id, &q(term))
                .await
                .first()
                .map(String::as_str),
            Some(first),
            "{term}: a name containing it, newest first among equals"
        );
    }
    let mut both = found(&app, &owner, ws_id, &q("budget")).await;
    both.sort();
    assert_eq!(both, vec!["Q3 budget.xlsx", "budget-2025.csv"]);
    assert_eq!(
        found(&app, &owner, ws_id, &q("izvestaj")).await,
        vec!["Izveštaj_final.pdf"],
        "diacritics fold"
    );
    assert!(
        found(&app, &owner, ws_id, &q("%")).await.is_empty(),
        "a wildcard is a character, not a pattern"
    );

    let (_, body) = send(
        &app,
        "GET",
        &format!("/api/search/files?workspace_id={ws_id}&q=xlsx"),
        Some(&owner),
        None,
    )
    .await;
    let hit = &body["data"][0];
    assert_eq!(hit["kind"], "spreadsheet");
    assert_eq!(hit["message_id"], json!(message_id));
    assert_eq!(hit["channel_id"], json!(ch));
    assert_eq!(hit["user_id"], json!(owner_id));
    assert!(hit["url"].as_str().is_some_and(|url| !url.is_empty()));

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/search/files?workspace_id={ws_id}&q=%20"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "no words and no filter"
    );
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/search/files?workspace_id={ws_id}&q=budget"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn file_search_finds_only_what_the_searcher_can_read(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "fsearch-vis-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "File Visibility WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "fsearch-vis-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "fsearch-vis-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    let (dm_peer_id, _, _) = seed_and_login(&app, &state, "fsearch-vis-peer", false).await;
    add_ws_member(&state, ws_id, dm_peer_id, "member").await;
    let (_, _, stranger) = seed_and_login(&app, &state, "fsearch-vis-stranger", false).await;

    let public = seed_channel(&state, ws_id, owner_id, "lobby", false)
        .await
        .to_string();
    let private = seed_channel(&state, ws_id, owner_id, "board", true)
        .await
        .to_string();
    let (status, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner),
        Some(json!({ "participant_ids": [dm_peer_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{conv:?}");
    let dm = conv["id"].as_str().expect("conversation id").to_string();

    post_file(
        &app,
        ws_id,
        &public,
        &owner,
        "plan-public.txt",
        "text/plain",
    )
    .await;
    post_file(
        &app,
        ws_id,
        &private,
        &owner,
        "plan-private.txt",
        "text/plain",
    )
    .await;
    post_file(&app, ws_id, &dm, &owner, "plan-dm.txt", "text/plain").await;
    let gone = post_file(
        &app,
        ws_id,
        &public,
        &owner,
        "plan-deleted.txt",
        "text/plain",
    )
    .await;
    upload(&app, ws_id, &owner, "plan-unsent.txt", "text/plain").await;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{gone}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let sorted = |mut names: Vec<String>| {
        names.sort();
        names
    };
    assert_eq!(
        sorted(found(&app, &owner, ws_id, "q=plan").await),
        vec!["plan-dm.txt", "plan-private.txt", "plan-public.txt"],
        "a deleted message's file and an unsent upload are not found, even by their uploader"
    );
    assert_eq!(
        found(&app, &member, ws_id, "q=plan").await,
        vec!["plan-public.txt"],
        "a member reads public channels, not a private one or someone else's DM"
    );
    assert!(
        found(&app, &guest, ws_id, "q=plan").await.is_empty(),
        "a guest is held to the channels they belong to"
    );
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{public}/members"),
        Some(&owner),
        Some(json!({ "user_id": guest_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        found(&app, &guest, ws_id, "q=plan").await,
        vec!["plan-public.txt"]
    );

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/search/files?workspace_id={ws_id}&q=plan"),
        Some(&stranger),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "not a workspace member");
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/search/files?workspace_id={ws_id}&channel_id={private}"),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "in: a private channel they are not in"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn file_search_filters_combine(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "fsearch-filter-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "File Filters WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "fsearch-filter-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let legal = seed_channel(&state, ws_id, owner_id, "legal", false)
        .await
        .to_string();
    let design = seed_channel(&state, ws_id, owner_id, "design", false)
        .await
        .to_string();

    post_file(&app, ws_id, &legal, &owner, "nda.pdf", "application/pdf").await;
    post_file(&app, ws_id, &legal, &owner, "terms.pdf", "application/pdf").await;
    post_file(
        &app,
        ws_id,
        &legal,
        &member,
        "redlines.pdf",
        "application/pdf",
    )
    .await;
    post_file(
        &app,
        ws_id,
        &legal,
        &owner,
        "terms.docx",
        "application/octet-stream",
    )
    .await;
    post_file(&app, ws_id, &design, &owner, "terms-mock.png", "image/png").await;

    assert_eq!(
        found(&app, &owner, ws_id, &format!("channel_id={legal}&kind=pdf")).await,
        vec!["redlines.pdf", "terms.pdf", "nda.pdf"],
        "an empty query with filters lists the newest first"
    );
    assert_eq!(
        found(
            &app,
            &owner,
            ws_id,
            &format!("channel_id={legal}&kind=pdf&user_id={member_id}")
        )
        .await,
        vec!["redlines.pdf"]
    );
    assert_eq!(
        found(&app, &owner, ws_id, "q=terms&kind=document").await,
        vec!["terms.docx"]
    );
    let mut terms = found(&app, &owner, ws_id, &format!("q=terms&channel_id={legal}")).await;
    terms.sort();
    assert_eq!(terms, vec!["terms.docx", "terms.pdf"]);
    assert_eq!(
        found(&app, &owner, ws_id, "q=terms&kind=image").await,
        vec!["terms-mock.png"]
    );

    let later = urlencode(&(chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339());
    let earlier = urlencode(&(chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339());
    assert!(
        found(&app, &owner, ws_id, &format!("q=terms&after={later}"))
            .await
            .is_empty()
    );
    assert_eq!(
        found(
            &app,
            &owner,
            ws_id,
            &format!("kind=image&after={earlier}&before={later}")
        )
        .await,
        vec!["terms-mock.png"]
    );
    assert!(
        found(&app, &owner, ws_id, &format!("kind=image&before={earlier}"))
            .await
            .is_empty()
    );

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/search/files?workspace_id={ws_id}&kind=spreadsheets"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "an unknown kind");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_name_and_kind_come_from_the_database(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "fsearch-column", false).await;
    let ws_id = seed_workspace(&state, owner_id, "File Column WS").await;
    let file_id = upload(&app, ws_id, &owner, "Ćevapi Menü.txt", "text/plain").await;
    let file_id: Uuid = serde_json::from_value(file_id).expect("uuid");

    let name: Option<String> = sqlx::query_scalar("SELECT search_name FROM files WHERE id = $1")
        .bind(file_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(name.as_deref(), Some("Cevapi Menu.txt"));
    sqlx::query("UPDATE files SET filename = 'Šifra.txt' WHERE id = $1")
        .bind(file_id)
        .execute(&state.pool)
        .await
        .unwrap();
    let name: Option<String> = sqlx::query_scalar("SELECT search_name FROM files WHERE id = $1")
        .bind(file_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(name.as_deref(), Some("Sifra.txt"), "a rename refreshes it");

    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../frontend/src/features/messaging/fileKindCases.json"
    ))
    .unwrap();
    for case in cases {
        let kind: String = sqlx::query_scalar("SELECT public.file_kind_of($1, $2)::text")
            .bind(case["mime"].as_str())
            .bind(case["name"].as_str())
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(kind, case["kind"].as_str().unwrap(), "{case}");
    }
}
