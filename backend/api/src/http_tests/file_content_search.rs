use super::common::*;
use super::file_search::{found, post_bytes, q};
use crate::config::AppConfig;
use crate::files::extractor;
use crate::state::AppState;
use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

const PDF: &str = "application/pdf";
const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const PPTX: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/fixtures/file_text/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("fixture")
}

async fn app_with_room(pool: PgPool) -> (axum::Router, std::sync::Arc<AppState>) {
    let config = AppConfig {
        max_upload_bytes: 1024 * 1024,
        ..test_config()
    };
    app_and_state_with(pool, config).await
}

async fn drain(state: &AppState) {
    loop {
        let jobs = extractor::claim(state).await.expect("claim");
        if jobs.is_empty() {
            return;
        }
        for job in jobs {
            extractor::process(state, job).await;
        }
    }
}

async fn hits(app: &axum::Router, token: &str, ws_id: Uuid, term: &str) -> Vec<Value> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/search/files?workspace_id={ws_id}&{}", q(term)),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{term}: {body:?}");
    body["data"].as_array().expect("data").clone()
}

fn highlighted(hit: &Value) -> Vec<String> {
    hit["snippet"]
        .as_array()
        .expect("a snippet")
        .iter()
        .filter(|part| part["hit"] == json!(true))
        .map(|part| part["text"].as_str().expect("text").to_string())
        .collect()
}

async fn status_of(state: &AppState, filename: &str) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT t.status::text FROM file_texts t JOIN files f ON f.id = t.file_id WHERE f.filename = $1",
    )
    .bind(filename)
    .fetch_one(&state.pool)
    .await
    .expect("a status row")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn text_inside_documents_is_found_with_a_snippet(pool: PgPool) {
    let (app, state) = app_with_room(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "fcontent-find", false).await;
    let ws_id = seed_workspace(&state, owner_id, "File Content WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "legal", false)
        .await
        .to_string();
    for (name, ctype, bytes) in [
        ("Agreement_v7.pdf", PDF, fixture("contract.pdf")),
        ("ugovor.docx", DOCX, fixture("contract.docx")),
        ("Q3.xlsx", XLSX, fixture("budget.xlsx")),
        ("deck.pptx", PPTX, fixture("roadmap.pptx")),
        (
            "notes.md",
            "text/markdown",
            "Sastanak u Subotici".as_bytes().to_vec(),
        ),
    ] {
        post_bytes(&app, ws_id, &ch, &owner, name, ctype, &bytes).await;
    }

    assert!(
        found(&app, &owner, ws_id, &q("Šćepanović"))
            .await
            .is_empty(),
        "nothing is searchable before the worker reads it"
    );
    drain(&state).await;

    let mut contract = found(&app, &owner, ws_id, &q("Šćepanović")).await;
    contract.sort();
    assert_eq!(contract, vec!["Agreement_v7.pdf", "ugovor.docx"]);
    assert_eq!(
        found(&app, &owner, ws_id, &q("scepanovic cacak"))
            .await
            .len(),
        2,
        "accents are optional and every word must appear"
    );
    for (term, name) in [
        ("konferencija", "Q3.xlsx"),
        ("Kragujevac", "deck.pptx"),
        ("subotici", "notes.md"),
    ] {
        assert_eq!(
            found(&app, &owner, ws_id, &q(term)).await,
            vec![name],
            "{term}"
        );
    }

    let hit = hits(&app, &owner, ws_id, "cacak").await;
    assert_eq!(hit.len(), 2);
    for hit in &hit {
        assert_eq!(
            highlighted(hit),
            vec!["Čačak"],
            "the original spelling is marked"
        );
        let text: String = hit["snippet"]
            .as_array()
            .unwrap()
            .iter()
            .map(|part| part["text"].as_str().unwrap())
            .collect();
        assert!(text.contains("Šćepanović, Čačak"), "{text}");
        assert!(!text.contains('\u{2}') && !text.contains('\u{3}'), "{text}");
    }
    assert_eq!(status_of(&state, "Agreement_v7.pdf").await, "done");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_name_match_ranks_above_a_content_match(pool: PgPool) {
    let (app, state) = app_with_room(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "fcontent-rank", false).await;
    let ws_id = seed_workspace(&state, owner_id, "File Rank WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "travel", false)
        .await
        .to_string();
    post_bytes(
        &app,
        ws_id,
        &ch,
        &owner,
        "kragujevac-trip.txt",
        "text/plain",
        b"hotel",
    )
    .await;
    post_bytes(
        &app,
        ws_id,
        &ch,
        &owner,
        "deck.pptx",
        PPTX,
        &fixture("roadmap.pptx"),
    )
    .await;
    drain(&state).await;

    let ranked = hits(&app, &owner, ws_id, "kragujevac").await;
    let names: Vec<&str> = ranked
        .iter()
        .map(|hit| hit["filename"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["kragujevac-trip.txt", "deck.pptx"]);
    assert!(
        ranked[0]["snippet"].is_null(),
        "a name match has no snippet"
    );
    assert_eq!(highlighted(&ranked[1]), vec!["Kragujevac"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn content_is_searched_under_the_same_visibility(pool: PgPool) {
    let (app, state) = app_with_room(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "fcontent-vis-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "File Content Vis WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "fcontent-vis-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let other_ws = seed_workspace(&state, owner_id, "Elsewhere WS").await;
    let public = seed_channel(&state, ws_id, owner_id, "lobby", false)
        .await
        .to_string();
    let private = seed_channel(&state, ws_id, owner_id, "board", true)
        .await
        .to_string();
    let elsewhere = seed_channel(&state, other_ws, owner_id, "lobby", false)
        .await
        .to_string();
    post_bytes(
        &app,
        ws_id,
        &public,
        &owner,
        "open.txt",
        "text/plain",
        b"vranje plan",
    )
    .await;
    post_bytes(
        &app,
        ws_id,
        &private,
        &owner,
        "closed.txt",
        "text/plain",
        b"vranje plan",
    )
    .await;
    post_bytes(
        &app,
        other_ws,
        &elsewhere,
        &owner,
        "away.txt",
        "text/plain",
        b"vranje plan",
    )
    .await;
    drain(&state).await;

    let mut all = found(&app, &owner, ws_id, &q("vranje")).await;
    all.sort();
    assert_eq!(
        all,
        vec!["closed.txt", "open.txt"],
        "one workspace at a time"
    );
    assert_eq!(
        found(&app, &member, ws_id, &q("vranje")).await,
        vec!["open.txt"]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn unreadable_files_are_recorded_once_and_deleting_a_file_drops_its_text(pool: PgPool) {
    let (app, state) = app_with_room(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "fcontent-status", false).await;
    let ws_id = seed_workspace(&state, owner_id, "File Status WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "inbox", false)
        .await
        .to_string();
    for (name, ctype, bytes) in [
        ("locked.pdf", PDF, fixture("encrypted.pdf")),
        ("broken.pdf", PDF, fixture("truncated.pdf")),
        ("blank.pdf", PDF, fixture("blank.pdf")),
        ("bomb.xlsx", XLSX, fixture("bomb.xlsx")),
        ("photo.zip", "application/zip", b"PK".to_vec()),
        ("keep.txt", "text/plain", b"zrenjanin".to_vec()),
    ] {
        post_bytes(&app, ws_id, &ch, &owner, name, ctype, &bytes).await;
    }
    sqlx::query("UPDATE files SET size_bytes = 200 * 1024 * 1024 WHERE filename = 'blank.pdf'")
        .execute(&state.pool)
        .await
        .unwrap();
    drain(&state).await;

    for (name, status) in [
        ("locked.pdf", "unsupported"),
        ("broken.pdf", "failed"),
        ("blank.pdf", "too_large"),
        ("bomb.xlsx", "too_large"),
        ("photo.zip", "unsupported"),
        ("keep.txt", "done"),
    ] {
        assert_eq!(status_of(&state, name).await, status, "{name}");
    }
    assert!(
        extractor::claim(&state).await.unwrap().is_empty(),
        "a file that cannot be read is not tried again"
    );

    let file_id: Uuid = sqlx::query_scalar("SELECT id FROM files WHERE filename = 'keep.txt'")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/files/{file_id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_texts WHERE file_id = $1")
        .bind(file_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
    assert!(found(&app, &owner, ws_id, &q("zrenjanin")).await.is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn two_claimers_never_take_the_same_file(pool: PgPool) {
    let (app, state) = app_with_room(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "fcontent-claim", false).await;
    let ws_id = seed_workspace(&state, owner_id, "File Claim WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "bulk", false)
        .await
        .to_string();
    let total = extractor::BATCH as usize + 10;
    for i in 0..total {
        post_bytes(
            &app,
            ws_id,
            &ch,
            &owner,
            &format!("f{i}.txt"),
            "text/plain",
            b"x",
        )
        .await;
    }

    let (first, second) = tokio::join!(extractor::claim(&state), extractor::claim(&state));
    let (first, second) = (first.unwrap(), second.unwrap());
    let third = extractor::claim(&state).await.unwrap();
    let mut seen = HashSet::new();
    for job in first.iter().chain(&second).chain(&third) {
        assert!(
            seen.insert(job.file_id),
            "{} was claimed twice",
            job.filename
        );
    }
    assert_eq!(seen.len(), total, "every file is claimed by someone");
    assert!(extractor::claim(&state).await.unwrap().is_empty());
    let victim = first.iter().chain(&second).next().expect("a claim").file_id;

    sqlx::query(
        "UPDATE file_texts SET claimed_at = NOW() - INTERVAL '1 hour'
          WHERE file_id = $1",
    )
    .bind(victim)
    .execute(&state.pool)
    .await
    .unwrap();
    let reclaimed = extractor::claim(&state).await.unwrap();
    assert_eq!(
        reclaimed.iter().map(|job| job.file_id).collect::<Vec<_>>(),
        vec![victim],
        "a claim abandoned by a crashed worker is picked up again"
    );

    sqlx::query(
        "UPDATE file_texts SET claimed_at = NOW() - INTERVAL '1 hour', attempts = 3
          WHERE file_id = $1",
    )
    .bind(victim)
    .execute(&state.pool)
    .await
    .unwrap();
    assert!(extractor::claim(&state).await.unwrap().is_empty());
    let status: String =
        sqlx::query_scalar("SELECT status::text FROM file_texts WHERE file_id = $1")
            .bind(victim)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(
        status, "failed",
        "a file that keeps killing the worker is given up on"
    );
}
