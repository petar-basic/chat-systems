use axum::http::StatusCode;
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

struct Team {
    ws: Uuid,
    owner: String,
    ana_id: Uuid,
    ana: String,
    ben_id: Uuid,
    ben: String,
    general: Uuid,
    secret: Uuid,
    lobby: Uuid,
}

async fn team(app: &Router, state: &AppState) -> Team {
    let (owner_id, _, owner) = seed_and_login(app, state, "rs-owner", false).await;
    let ws = seed_workspace(state, owner_id, "Searches").await;
    let (ana_id, _, ana) = seed_and_login(app, state, "rs-ana", false).await;
    add_ws_member(state, ws, ana_id, "member").await;
    let (ben_id, _, ben) = seed_and_login(app, state, "rs-ben", false).await;
    add_ws_member(state, ws, ben_id, "member").await;
    let general = seed_channel(state, ws, owner_id, "rs-general", false).await;
    let secret = seed_channel(state, ws, owner_id, "rs-secret", true).await;
    let lobby = seed_channel(state, ws, owner_id, "rs-lobby", false).await;
    for (channel, user) in [(general, ana_id), (general, ben_id), (secret, ana_id)] {
        state
            .workspace_service
            .repo
            .add_channel_member(channel, user, &ChannelRole::Member)
            .await
            .expect("join");
    }
    Team {
        ws,
        owner,
        ana_id,
        ana,
        ben_id,
        ben,
        general,
        secret,
        lobby,
    }
}

fn search(query: &str, tab: &str, filters: Value) -> Value {
    json!({ "query": query, "tab": tab, "filters": filters })
}

async fn record(app: &Router, token: &str, ws: Uuid, body: Value) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/v1/workspaces/{ws}/searches/recent"),
        Some(token),
        Some(body),
    )
    .await
}

async fn recorded(app: &Router, token: &str, ws: Uuid, body: Value) -> Value {
    let (status, saved) = record(app, token, ws, body).await;
    assert_eq!(status, StatusCode::OK, "{saved:?}");
    saved
}

async fn recent(app: &Router, token: &str, ws: Uuid) -> Vec<Value> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/v1/workspaces/{ws}/searches/recent"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"].as_array().expect("data").clone()
}

fn queries(searches: &[Value]) -> Vec<&str> {
    searches
        .iter()
        .map(|s| s["query"].as_str().expect("query"))
        .collect()
}

async fn stored(state: &AppState, users: Vec<Uuid>) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM recent_searches WHERE user_id = ANY($1)")
        .bind(users)
        .fetch_one(&state.pool)
        .await
        .expect("count")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_search_is_kept_once_newest_first_and_twenty_at_most(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let in_general = json!({ "in": t.general });

    let first = recorded(
        &app,
        &t.ana,
        t.ws,
        search("deploy", "messages", in_general.clone()),
    )
    .await;
    assert_eq!(first["filters"], in_general);
    for _ in 0..9 {
        recorded(
            &app,
            &t.ana,
            t.ws,
            search("  Deploy ", "messages", in_general.clone()),
        )
        .await;
    }
    let listed = recent(&app, &t.ana, t.ws).await;
    assert_eq!(
        queries(&listed),
        ["Deploy"],
        "ten of the same search show once"
    );
    assert_eq!(listed[0]["id"], first["id"]);

    recorded(&app, &t.ana, t.ws, search("deploy", "messages", json!({}))).await;
    recorded(
        &app,
        &t.ana,
        t.ws,
        search("deploy", "files", in_general.clone()),
    )
    .await;
    recorded(&app, &t.ana, t.ws, search("release", "messages", json!({}))).await;
    assert_eq!(
        recent(&app, &t.ana, t.ws).await.len(),
        4,
        "other filters or another tab are another search"
    );
    recorded(
        &app,
        &t.ana,
        t.ws,
        search("deploy", "messages", in_general.clone()),
    )
    .await;
    assert_eq!(recent(&app, &t.ana, t.ws).await[0]["filters"], in_general);

    for n in 0..25 {
        recorded(
            &app,
            &t.ana,
            t.ws,
            search(&format!("q{n}"), "messages", json!({})),
        )
        .await;
    }
    let listed = recent(&app, &t.ana, t.ws).await;
    assert_eq!(listed.len(), 20);
    assert_eq!(listed[0]["query"], "q24");
    assert_eq!(listed[19]["query"], "q5");
    assert_eq!(stored(&state, vec![t.ana_id]).await, 20);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_search_keeps_only_the_filters_its_tab_uses(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;

    let saved = recorded(
        &app,
        &t.ana,
        t.ws,
        search(
            "",
            "messages",
            json!({ "has": "link", "kind": "pdf", "on": "2026-11-10", "after": "2026-11-01" }),
        ),
    )
    .await;
    assert_eq!(
        saved["filters"],
        json!({ "has": "link", "on": "2026-11-10" })
    );
    let saved = recorded(
        &app,
        &t.ana,
        t.ws,
        search(
            "report",
            "files",
            json!({ "has": "link", "kind": "pdf", "before": "2026-11-01" }),
        ),
    )
    .await;
    assert_eq!(
        saved["filters"],
        json!({ "kind": "pdf", "before": "2026-11-01" })
    );

    for body in [
        search("   ", "messages", json!({})),
        search("", "messages", json!({ "kind": "pdf" })),
        search(&"x".repeat(501), "messages", json!({})),
        search("deploy", "messages", json!({ "channel": t.general })),
        search("deploy", "everything", json!({})),
        search("deploy", "messages", json!({ "on": "yesterday" })),
    ] {
        let (status, answer) = record(&app, &t.ana, t.ws, body.clone()).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{body} → {answer:?}"
        );
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn recent_searches_answer_for_their_owner_only(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let (_, _, outsider) = seed_and_login(&app, &state, "rs-outsider", false).await;
    let (elsewhere_owner, _, _) = seed_and_login(&app, &state, "rs-elsewhere", false).await;
    let elsewhere = seed_workspace(&state, elsewhere_owner, "Elsewhere").await;
    let foreign = seed_channel(&state, elsewhere, elsewhere_owner, "rs-foreign", false).await;

    let kept = recorded(&app, &t.ana, t.ws, search("deploy", "messages", json!({}))).await;
    let dropped = recorded(
        &app,
        &t.ana,
        t.ws,
        search("plans", "messages", json!({ "in": t.secret })),
    )
    .await;
    assert!(
        recent(&app, &t.ben, t.ws).await.is_empty(),
        "nobody sees another's searches"
    );

    let ana_one = format!(
        "/api/v1/workspaces/{}/searches/recent/{}",
        t.ws,
        kept["id"].as_str().expect("id")
    );
    let (status, _) = send(&app, "DELETE", &ana_one, Some(&t.ben), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/v1/workspaces/{}/searches/recent", t.ws),
        Some(&t.ben),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        recent(&app, &t.ana, t.ws).await.len(),
        2,
        "Ben's clear leaves Ana's"
    );

    for (method, body) in [
        ("GET", None),
        ("POST", Some(search("deploy", "messages", json!({})))),
        ("DELETE", None),
    ] {
        let (status, _) = send(
            &app,
            method,
            &format!("/api/v1/workspaces/{}/searches/recent", t.ws),
            Some(&outsider),
            body,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{method} by someone outside the workspace"
        );
    }

    for place in [t.secret, t.lobby, foreign, Uuid::new_v4()] {
        let (status, _) = record(
            &app,
            &t.ben,
            t.ws,
            search("plans", "messages", json!({ "in": place })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "a place Ben is not in is not one he can keep"
        );
    }

    let (status, _) = send(&app, "DELETE", &ana_one, Some(&t.ana), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(queries(&recent(&app, &t.ana, t.ws).await), ["plans"]);
    let (status, _) = send(&app, "DELETE", &ana_one, Some(&t.ana), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/v1/workspaces/{}/searches/recent", t.ws),
        Some(&t.ana),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(recent(&app, &t.ana, t.ws).await.is_empty());
    assert_ne!(dropped["id"], kept["id"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_lost_after_a_search_drops_out_of_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    recorded(
        &app,
        &t.ana,
        t.ws,
        search(
            "plans",
            "messages",
            json!({ "in": t.secret, "has": "file" }),
        ),
    )
    .await;
    recorded(
        &app,
        &t.ana,
        t.ws,
        search("lunch", "messages", json!({ "in": t.general })),
    )
    .await;

    for channel in [t.secret, t.general] {
        sqlx::query("DELETE FROM channel_members WHERE channel_id = $1 AND user_id = $2")
            .bind(channel)
            .bind(t.ana_id)
            .execute(&state.pool)
            .await
            .expect("leave");
    }

    let listed = recent(&app, &t.ana, t.ws).await;
    assert_eq!(queries(&listed), ["lunch", "plans"]);
    assert_eq!(
        listed[0]["filters"],
        json!({}),
        "a public channel left is out of the sidebar, so it leaves the search too"
    );
    assert_eq!(
        listed[1]["filters"],
        json!({ "has": "file" }),
        "the private channel is gone, the rest of the search is not"
    );

    let again = recorded(
        &app,
        &t.ana,
        t.ws,
        search("plans", "messages", json!({ "has": "file" })),
    )
    .await;
    assert_eq!(
        queries(&recent(&app, &t.ana, t.ws).await),
        ["plans", "lunch"],
        "running it again from the list shows it once"
    );
    let (status, _) = send(
        &app,
        "DELETE",
        &format!(
            "/api/v1/workspaces/{}/searches/recent/{}",
            t.ws,
            again["id"].as_str().expect("id")
        ),
        Some(&t.ana),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(queries(&recent(&app, &t.ana, t.ws).await), ["lunch"]);
    assert_eq!(
        stored(&state, vec![t.ana_id]).await,
        1,
        "removing it also removes the copy that still names the channel"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_person_export_carries_recent_searches_and_erasing_or_leaving_removes_them(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    recorded(
        &app,
        &t.ana,
        t.ws,
        search("deploy", "messages", json!({ "in": t.general })),
    )
    .await;
    recorded(
        &app,
        &t.ana,
        t.ws,
        search("report", "files", json!({ "kind": "pdf" })),
    )
    .await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/v1/users/{}/exports", t.ana_id),
        Some(&t.ana),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let job = state
        .export_repo
        .claim_next()
        .await
        .expect("claim")
        .expect("a pending job");
    let job_id = job.id;
    crate::export::job::run_for_test(&state, job)
        .await
        .expect("export runs");
    let manifest = state
        .export_repo
        .find(job_id)
        .await
        .expect("find")
        .expect("job")
        .manifest
        .expect("manifest");
    assert_eq!(manifest["files"]["recent_searches.jsonl"]["rows"], 2);
    let exported = state.search_repo.owned_by(t.ana_id).await.expect("owned");
    assert_eq!(exported[0]["query"], "deploy");
    assert_eq!(exported[0]["filters"], json!({ "in": t.general }));
    assert_eq!(exported[1]["tab"], "files");

    let (_, _, instance_admin) = seed_and_login(&app, &state, "rs-instance", true).await;
    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/v1/admin/users/{}/data", t.ana_id),
        Some(&instance_admin),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["account"]["searches"], 2);

    recorded(&app, &t.ben, t.ws, search("standup", "messages", json!({}))).await;
    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/v1/workspaces/{}/members/{}", t.ws, t.ben_id),
        Some(&t.owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(stored(&state, vec![t.ana_id, t.ben_id]).await, 0);
}
