use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, NaiveTime, SecondsFormat, SubsecRound, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::calendars::service;
use crate::state::AppState;

#[derive(Default)]
struct Fake {
    base: String,
    full: Vec<Value>,
    changes: Vec<Value>,
    gone: bool,
}

type Shared = Arc<Mutex<Fake>>;

fn lock(fake: &Shared) -> std::sync::MutexGuard<'_, Fake> {
    fake.lock().expect("fake")
}

fn rfc(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn bearer(headers: &HeaderMap, tokens: &[&str]) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| tokens.iter().any(|t| v == format!("Bearer {t}")))
}

async fn token(axum::Form(form): axum::Form<HashMap<String, String>>) -> (StatusCode, Json<Value>) {
    let get = |k: &str| form.get(k).map(String::as_str);
    match get("grant_type") {
        Some("authorization_code") if get("code") == Some("good-code") => (
            StatusCode::OK,
            Json(json!({ "access_token": "cal-1", "refresh_token": "cal-r", "expires_in": 3599 })),
        ),
        Some("refresh_token") if get("refresh_token") == Some("cal-r") => (
            StatusCode::OK,
            Json(json!({ "access_token": "cal-2", "refresh_token": "cal-r", "expires_in": 3599 })),
        ),
        _ => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_grant" })),
        ),
    }
}

async fn google_events(
    State(fake): State<Shared>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    if !bearer(&headers, &["cal-1", "cal-2"]) {
        return (StatusCode::UNAUTHORIZED, Json(json!({})));
    }
    if query.get("fields").map(String::as_str) == Some("summary") {
        return (StatusCode::OK, Json(json!({ "summary": "ana@acme.test" })));
    }
    if query.get("singleEvents").map(String::as_str) != Some("true") {
        return (StatusCode::BAD_REQUEST, Json(json!({})));
    }
    let fake = lock(&fake);
    match query.get("syncToken").map(String::as_str) {
        Some(_) if fake.gone => (StatusCode::GONE, Json(json!({}))),
        Some("s1") => (
            StatusCode::OK,
            Json(json!({ "items": fake.changes, "nextSyncToken": "s2" })),
        ),
        Some(_) => (
            StatusCode::OK,
            Json(json!({ "items": [], "nextSyncToken": "s2" })),
        ),
        None => {
            if !query.contains_key("timeMin") || !query.contains_key("timeMax") {
                return (StatusCode::BAD_REQUEST, Json(json!({})));
            }
            (
                StatusCode::OK,
                Json(json!({ "items": fake.full, "nextSyncToken": "s1" })),
            )
        }
    }
}

async fn graph_calendar(headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !bearer(&headers, &["cal-1", "cal-2"]) {
        return (StatusCode::UNAUTHORIZED, Json(json!({})));
    }
    (
        StatusCode::OK,
        Json(json!({ "owner": { "name": "Bob", "address": "bob@acme.test" } })),
    )
}

async fn graph_delta(
    State(fake): State<Shared>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    if !bearer(&headers, &["cal-1", "cal-2"]) {
        return (StatusCode::UNAUTHORIZED, Json(json!({})));
    }
    let prefer = headers
        .get("prefer")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !prefer.contains("outlook.timezone=\"UTC\"") {
        return (StatusCode::BAD_REQUEST, Json(json!({})));
    }
    let fake = lock(&fake);
    let link = |param: &str, value: &str| {
        format!("{}/v1.0/me/calendarView/delta?{param}={value}", fake.base)
    };
    match (
        query.get("$deltatoken").map(String::as_str),
        query.get("$skiptoken").map(String::as_str),
    ) {
        (Some(_), _) if fake.gone => (StatusCode::GONE, Json(json!({}))),
        (Some("d1"), _) => (
            StatusCode::OK,
            Json(json!({ "value": fake.changes, "@odata.deltaLink": link("$deltatoken", "d2") })),
        ),
        (Some(_), _) => (
            StatusCode::OK,
            Json(json!({ "value": [], "@odata.deltaLink": link("$deltatoken", "d2") })),
        ),
        (None, Some("p2")) => (
            StatusCode::OK,
            Json(json!({
                "value": fake.full.iter().skip(1).collect::<Vec<_>>(),
                "@odata.deltaLink": link("$deltatoken", "d1"),
            })),
        ),
        (None, _) => {
            if !query.contains_key("startDateTime") || !query.contains_key("endDateTime") {
                return (StatusCode::BAD_REQUEST, Json(json!({})));
            }
            (
                StatusCode::OK,
                Json(json!({
                    "value": fake.full.iter().take(1).collect::<Vec<_>>(),
                    "@odata.nextLink": link("$skiptoken", "p2"),
                })),
            )
        }
    }
}

async fn fake_calendars() -> Shared {
    let fake: Shared = Arc::default();
    let router = Router::new()
        .route("/token", post(token))
        .route("/organizations/oauth2/v2.0/token", post(token))
        .route("/calendar/v3/calendars/primary/events", get(google_events))
        .route("/v1.0/me/calendar", get(graph_calendar))
        .route("/v1.0/me/calendarView/delta", get(graph_delta))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    lock(&fake).base = format!("http://{addr}");
    fake
}

struct Setup {
    app: Router,
    state: Arc<AppState>,
    fake: Shared,
    owner: Uuid,
    owner_token: String,
    ws_id: Uuid,
}

async fn setup(pool: PgPool) -> Setup {
    let fake = fake_calendars().await;
    let base = lock(&fake).base.clone();
    let config = crate::config::AppConfig {
        google_accounts_url: base.clone(),
        google_oauth_url: base.clone(),
        google_api_url: base.clone(),
        microsoft_login_url: base.clone(),
        microsoft_graph_url: base,
        ..test_config()
    };
    let (app, state) = app_and_state_with(pool, config).await;
    let (owner, _email, owner_token) = seed_and_login(&app, &state, "cal-owner", false).await;
    let ws_id = seed_workspace(&state, owner, &format!("ws-{}", Uuid::new_v4())).await;
    sqlx::query("UPDATE users SET timezone = 'UTC' WHERE id = $1")
        .bind(owner)
        .execute(&state.pool)
        .await
        .expect("timezone");
    Setup {
        app,
        state,
        fake,
        owner,
        owner_token,
        ws_id,
    }
}

async fn set_up(s: &Setup, provider: &str) {
    let (status, body) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/{provider}", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "client_id": "cal-client", "client_secret": "shh", "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

async fn connect(s: &Setup, token: &str, provider: &str) -> (reqwest::Url, Value) {
    let (status, body) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/apps/{provider}/connect", s.ws_id),
        Some(token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let url = reqwest::Url::parse(body["authorize_url"].as_str().expect("url")).expect("parse");
    let oauth_state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .expect("state");
    let (status, body) = send(
        &s.app,
        "POST",
        "/api/apps/connections",
        Some(token),
        Some(json!({ "code": "good-code", "state": oauth_state })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    (url, body)
}

async fn calendars(s: &Setup, token: &str) -> Value {
    let (status, body) = send(
        &s.app,
        "GET",
        &format!("/api/workspaces/{}/calendars", s.ws_id),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"].clone()
}

async fn events(s: &Setup, user: Uuid) -> Vec<(String, Option<String>, bool)> {
    sqlx::query_as(
        "SELECT event_id, title, private FROM calendar_events WHERE user_id = $1 ORDER BY event_id",
    )
    .bind(user)
    .fetch_all(&s.state.pool)
    .await
    .expect("events")
}

type StatusRow = (
    Option<String>,
    Option<String>,
    Option<DateTime<Utc>>,
    Option<String>,
);

async fn status_of(s: &Setup, user: Uuid) -> StatusRow {
    sqlx::query_as(
        "SELECT status_emoji, status_text, status_expires_at, status_source
           FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(s.ws_id)
    .bind(user)
    .fetch_one(&s.state.pool)
    .await
    .expect("status")
}

async fn bot_messages(s: &Setup, kind: &str) -> Vec<(String, Value)> {
    sqlx::query_as(
        "SELECT m.content, m.metadata FROM messages m
           JOIN workspace_bots b ON b.user_id = m.user_id
          WHERE b.workspace_id = $1 AND b.kind::text = $2
          ORDER BY m.created_at",
    )
    .bind(s.ws_id)
    .bind(kind)
    .fetch_all(&s.state.pool)
    .await
    .expect("messages")
}

async fn due_now(s: &Setup) {
    sqlx::query("UPDATE calendar_syncs SET next_sync_at = NOW() - INTERVAL '1 year'")
        .execute(&s.state.pool)
        .await
        .expect("due");
}

fn meeting(id: &str, title: &str, from: DateTime<Utc>, minutes: i64) -> Value {
    json!({
        "id": id,
        "summary": title,
        "start": { "dateTime": rfc(from) },
        "end": { "dateTime": rfc(from + Duration::minutes(minutes)) },
        "attendees": [
            { "self": true, "responseStatus": "accepted" },
            { "responseStatus": "accepted" }
        ],
        "hangoutLink": "https://meet.google.com/abc-defg-hij"
    })
}

async fn tick(s: &Setup, now: DateTime<Utc>) -> service::Tick {
    service::tick(&s.state, now).await.expect("tick")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn google_calendar_reuses_drive_and_disconnecting_forgets_everything(pool: PgPool) {
    let s = setup(pool).await;
    let now = Utc::now();
    lock(&s.fake).full = vec![meeting("m1", "Weekly sync", now - Duration::minutes(5), 30)];

    let listed = calendars(&s, &s.owner_token).await;
    assert_eq!(listed[0]["provider"], json!("google_calendar"));
    assert_eq!(listed[0]["set_up"], json!(false));
    assert_eq!(listed[1]["provider"], json!("outlook_calendar"));

    set_up(&s, "google_drive").await;
    let reuse = json!({ "client_id": "", "enabled": true, "reuse_from": "google_drive" });
    let (status, _) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/outlook_calendar", s.ws_id),
        Some(&s.owner_token),
        Some(reuse.clone()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "only Google Calendar reuses Drive"
    );
    let (status, body) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/google_calendar", s.ws_id),
        Some(&s.owner_token),
        Some(reuse),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (url, body) = connect(&s, &s.owner_token, "google_calendar").await;
    assert!(url
        .query_pairs()
        .any(|(k, v)| k == "scope"
            && v == "https://www.googleapis.com/auth/calendar.events.readonly"));
    assert!(url
        .query_pairs()
        .any(|(k, v)| k == "client_id" && v == "cal-client"));
    assert_eq!(body["external_account"], json!("ana@acme.test"));

    let listed = calendars(&s, &s.owner_token).await;
    assert_eq!(listed[0]["set_up"], json!(true));
    assert_eq!(listed[0]["connected_account"], json!("ana@acme.test"));
    assert_eq!(listed[0]["settings"]["status"], json!(true));
    assert_eq!(listed[0]["settings"]["agenda_at"], json!("09:00"));
    assert_eq!(listed[1]["settings"], Value::Null);

    due_now(&s).await;
    let ticked = tick(&s, now).await;
    assert_eq!(ticked.synced, 1);
    assert_eq!(ticked.statuses, 1);
    assert_eq!(events(&s, s.owner).await.len(), 1);
    let (emoji, text, until, source) = status_of(&s, s.owner).await;
    assert_eq!(emoji.as_deref(), Some("🗓️"));
    assert_eq!(text.as_deref(), Some("In a meeting"));
    assert_eq!(source.as_deref(), Some("google_calendar"));
    assert!(until.is_some_and(|at| at > now));

    let (status, body) = send(
        &s.app,
        "DELETE",
        &format!(
            "/api/workspaces/{}/apps/google_calendar/connection",
            s.ws_id
        ),
        Some(&s.owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(events(&s, s.owner).await.is_empty());
    let syncs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM calendar_syncs WHERE user_id = $1")
        .bind(s.owner)
        .fetch_one(&s.state.pool)
        .await
        .expect("count");
    assert_eq!(syncs, 0);
    assert_eq!(status_of(&s, s.owner).await, (None, None, None, None));
    assert_eq!(tick(&s, now + Duration::minutes(1)).await.statuses, 0);
    assert_eq!(status_of(&s, s.owner).await, (None, None, None, None));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_changed_and_a_deleted_event_sync_incrementally_and_a_lost_token_resyncs(pool: PgPool) {
    let s = setup(pool).await;
    set_up(&s, "google_calendar").await;
    connect(&s, &s.owner_token, "google_calendar").await;
    let now = Utc::now();
    let a = meeting("a", "Planning", now + Duration::hours(2), 30);
    let b = meeting("b", "Retro", now + Duration::hours(3), 30);
    lock(&s.fake).full = vec![a.clone(), b];
    due_now(&s).await;
    tick(&s, now).await;
    assert_eq!(
        events(&s, s.owner).await,
        vec![
            ("a".into(), Some("Planning".into()), false),
            ("b".into(), Some("Retro".into()), false)
        ]
    );
    let token: Option<String> =
        sqlx::query_scalar("SELECT sync_token FROM calendar_syncs WHERE user_id = $1")
            .bind(s.owner)
            .fetch_one(&s.state.pool)
            .await
            .expect("token");
    assert_eq!(token.as_deref(), Some("s1"));

    let mut moved = a.clone();
    moved["summary"] = json!("Planning, moved");
    lock(&s.fake).changes = vec![moved, json!({ "id": "b", "status": "cancelled" })];
    assert_eq!(
        tick(&s, now + Duration::minutes(1)).await.synced,
        0,
        "nothing is due before five minutes"
    );
    assert_eq!(tick(&s, now + Duration::minutes(6)).await.synced, 1);
    assert_eq!(
        events(&s, s.owner).await,
        vec![("a".into(), Some("Planning, moved".into()), false)]
    );

    lock(&s.fake).gone = true;
    tick(&s, now + Duration::minutes(12)).await;
    assert_eq!(
        events(&s, s.owner).await.len(),
        2,
        "a refused token is a full sync of the window"
    );
    let (token, error): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT sync_token, last_error FROM calendar_syncs WHERE user_id = $1")
            .bind(s.owner)
            .fetch_one(&s.state.pool)
            .await
            .expect("sync");
    assert_eq!(token.as_deref(), Some("s1"));
    assert_eq!(error, None);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_status_is_never_written_over_the_persons_own_or_after_they_clear_it(pool: PgPool) {
    let s = setup(pool).await;
    set_up(&s, "google_calendar").await;
    connect(&s, &s.owner_token, "google_calendar").await;
    let now = Utc::now();
    let first = (now + Duration::minutes(10)).trunc_subsecs(0);
    let second = (now + Duration::minutes(60)).trunc_subsecs(0);
    lock(&s.fake).full = vec![
        json!({
            "id": "solo", "summary": "Focus",
            "start": { "dateTime": rfc(now + Duration::minutes(2)) },
            "end": { "dateTime": rfc(now + Duration::minutes(8)) }
        }),
        meeting("one", "Standup", first, 30),
        meeting("two", "Design review", second, 30),
    ];
    due_now(&s).await;
    tick(&s, now).await;
    tick(&s, now + Duration::minutes(3)).await;
    assert_eq!(
        status_of(&s, s.owner).await,
        (None, None, None, None),
        "an event with nobody else invited is not a meeting"
    );

    tick(&s, first + Duration::minutes(1)).await;
    let (_, text, until, _) = status_of(&s, s.owner).await;
    assert_eq!(text.as_deref(), Some("In a meeting"));
    assert_eq!(until, Some(first + Duration::minutes(30)));

    let (status, _) = send(
        &s.app,
        "DELETE",
        &format!("/api/workspaces/{}/profile/status", s.ws_id),
        Some(&s.owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    tick(&s, first + Duration::minutes(2)).await;
    assert_eq!(
        status_of(&s, s.owner).await,
        (None, None, None, None),
        "a status the person cleared stays cleared for that meeting"
    );

    tick(&s, second + Duration::minutes(1)).await;
    assert_eq!(
        status_of(&s, s.owner).await.1.as_deref(),
        Some("In a meeting")
    );

    let (status, _) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/profile/status", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "emoji": "🍔", "text": "Lunch" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    tick(&s, second + Duration::minutes(2)).await;
    tick(&s, second + Duration::minutes(40)).await;
    assert_eq!(
        status_of(&s, s.owner).await,
        (Some("🍔".into()), Some("Lunch".into()), None, None)
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_reminder_comes_a_minute_before_with_join_and_a_private_one_says_busy(pool: PgPool) {
    let s = setup(pool).await;
    set_up(&s, "google_calendar").await;
    connect(&s, &s.owner_token, "google_calendar").await;
    let (status, _) = send(
        &s.app,
        "PATCH",
        &format!("/api/workspaces/{}/calendars/google_calendar", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "agenda": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let now = Utc::now();
    let standup = now + Duration::minutes(10);
    let doctor = now + Duration::minutes(60);
    let maybe = now + Duration::minutes(120);
    let mut private = meeting("doctor", "Doctor", doctor, 30);
    private["visibility"] = json!("private");
    private.as_object_mut().map(|o| o.remove("hangoutLink"));
    let mut tentative = meeting("maybe", "Offsite planning", maybe, 30);
    tentative["attendees"][0]["responseStatus"] = json!("tentative");
    lock(&s.fake).full = vec![
        meeting("standup", "Standup", standup, 15),
        private,
        tentative,
    ];
    due_now(&s).await;
    tick(&s, now).await;
    assert_eq!(
        events(&s, s.owner).await[0],
        ("doctor".into(), None, true),
        "a private event's title is never stored"
    );

    assert_eq!(tick(&s, standup - Duration::seconds(90)).await.reminders, 0);
    assert_eq!(tick(&s, standup - Duration::seconds(50)).await.reminders, 1);
    assert_eq!(tick(&s, standup - Duration::seconds(35)).await.reminders, 0);
    let sent = bot_messages(&s, "google_calendar").await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "Standup starts in 1 minute");
    let blocks = &sent[0].1["blocks"];
    assert!(
        blocks
            .to_string()
            .contains("https://meet.google.com/abc-defg-hij"),
        "{blocks}"
    );
    let bot: (String, bool) = sqlx::query_as(
        "SELECT u.display_name, u.is_bot FROM workspace_bots b JOIN users u ON u.id = b.user_id
          WHERE b.workspace_id = $1 AND b.kind = 'google_calendar'",
    )
    .bind(s.ws_id)
    .fetch_one(&s.state.pool)
    .await
    .expect("bot");
    assert_eq!(bot, ("Google Calendar".into(), true));
    let direct: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages m JOIN channels c ON c.id = m.channel_id
          JOIN workspace_bots b ON b.user_id = m.user_id
         WHERE b.workspace_id = $1 AND c.channel_type = 'dm'",
    )
    .bind(s.ws_id)
    .fetch_one(&s.state.pool)
    .await
    .expect("direct");
    assert_eq!(direct, 1, "the reminder is a DM from the calendar's bot");

    assert_eq!(tick(&s, doctor - Duration::seconds(30)).await.reminders, 1);
    tick(&s, doctor + Duration::minutes(1)).await;
    assert_eq!(status_of(&s, s.owner).await.1.as_deref(), Some("Busy"));
    assert_eq!(tick(&s, maybe - Duration::seconds(30)).await.reminders, 0);

    let sent = bot_messages(&s, "google_calendar").await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].0, "Busy starts in 1 minute");
    assert!(!sent
        .iter()
        .any(|(content, metadata)| content.contains("Doctor")
            || metadata.to_string().contains("Doctor")));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_agenda_comes_at_the_chosen_time_once_a_day(pool: PgPool) {
    let s = setup(pool).await;
    set_up(&s, "google_calendar").await;
    connect(&s, &s.owner_token, "google_calendar").await;
    let day = (Utc::now() + Duration::days(1)).date_naive();
    let at = |h: u32, m: u32| {
        day.and_time(NaiveTime::from_hms_opt(h, m, 0).expect("time"))
            .and_utc()
    };
    let mut private = meeting("doctor", "Doctor", at(11, 0), 60);
    private["visibility"] = json!("private");
    lock(&s.fake).full = vec![
        meeting("retro", "Retro", at(9, 0), 30),
        private,
        json!({ "id": "trip", "summary": "Lisbon trip",
                "start": { "date": day.to_string() },
                "end": { "date": day.succ_opt().expect("next").to_string() } }),
    ];

    let (status, body) = send(
        &s.app,
        "PATCH",
        &format!("/api/workspaces/{}/calendars/google_calendar", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "agenda_at": "8:00" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    let (status, body) = send(
        &s.app,
        "PATCH",
        &format!("/api/workspaces/{}/calendars/google_calendar", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "agenda_at": "08:00" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["settings"]["agenda_at"], json!("08:00"));

    assert_eq!(tick(&s, at(7, 59)).await.agendas, 0);
    assert_eq!(tick(&s, at(8, 1)).await.agendas, 1);
    assert_eq!(tick(&s, at(8, 30)).await.agendas, 0, "once a day");
    let sent = bot_messages(&s, "google_calendar").await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "You have 3 events today");
    let blocks = sent[0].1["blocks"].to_string();
    assert!(blocks.contains("Lisbon trip"), "{blocks}");
    assert!(blocks.contains("All day"), "{blocks}");
    assert!(blocks.contains("09:00 – 09:30"), "{blocks}");
    assert!(
        blocks.contains("Busy") && !blocks.contains("Doctor"),
        "{blocks}"
    );

    let (status, _) = send(
        &s.app,
        "PATCH",
        &format!("/api/workspaces/{}/calendars/google_calendar", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "agenda": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        tick(&s, at(8, 1) + Duration::days(1)).await.agendas,
        0,
        "turned off"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_outlook_out_of_office_day_shows_the_person_away_until_it_is_removed(pool: PgPool) {
    let s = setup(pool).await;
    set_up(&s, "outlook_calendar").await;
    let (url, body) = connect(&s, &s.owner_token, "outlook_calendar").await;
    assert!(url.path().ends_with("/organizations/oauth2/v2.0/authorize"));
    assert!(url
        .query_pairs()
        .any(|(k, v)| k == "scope" && v == "Calendars.Read offline_access"));
    assert_eq!(body["external_account"], json!("Bob (bob@acme.test)"));

    let now = Utc::now();
    let today = now.date_naive();
    let midnight = |d: chrono::NaiveDate| format!("{}T00:00:00.0000000", d);
    let next = today.succ_opt().expect("next");
    lock(&s.fake).full = vec![
        json!({
            "id": "AAMk-standup",
            "subject": "Standup",
            "start": { "dateTime": (now + Duration::hours(3)).format("%Y-%m-%dT%H:%M:%S%.f").to_string(), "timeZone": "UTC" },
            "end": { "dateTime": (now + Duration::hours(4)).format("%Y-%m-%dT%H:%M:%S%.f").to_string(), "timeZone": "UTC" },
            "responseStatus": { "response": "accepted" },
            "attendees": [{ "emailAddress": { "address": "ana@acme.test" } }],
            "onlineMeeting": { "joinUrl": "https://teams.microsoft.com/l/meetup-join/1" }
        }),
        json!({
            "id": "AAMk-ooo",
            "subject": "Away",
            "isAllDay": true,
            "isOrganizer": true,
            "showAs": "oof",
            "start": { "dateTime": midnight(today), "timeZone": "UTC" },
            "end": { "dateTime": midnight(next), "timeZone": "UTC" }
        }),
    ];
    due_now(&s).await;
    tick(&s, now).await;
    assert_eq!(
        events(&s, s.owner).await.len(),
        2,
        "both delta pages are read"
    );
    let (emoji, text, until, source) = status_of(&s, s.owner).await;
    assert_eq!(emoji.as_deref(), Some("⛔"));
    assert_eq!(text.as_deref(), Some("Out of office"));
    assert_eq!(source.as_deref(), Some("outlook_calendar"));
    let tomorrow = next.and_hms_opt(0, 0, 0).expect("midnight").and_utc();
    assert_eq!(until, Some(tomorrow));
    let away: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT calendar_away_until FROM users WHERE id = $1")
            .bind(s.owner)
            .fetch_one(&s.state.pool)
            .await
            .expect("away");
    assert_eq!(away, Some(tomorrow));

    let away_until = || async {
        sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
            "SELECT calendar_away_until FROM users WHERE id = $1",
        )
        .bind(s.owner)
        .fetch_one(&s.state.pool)
        .await
        .expect("away")
    };
    let settings = format!("/api/workspaces/{}/calendars/outlook_calendar", s.ws_id);
    let switch = |on: bool| {
        send(
            &s.app,
            "PATCH",
            &settings,
            Some(&s.owner_token),
            Some(json!({ "status": on })),
        )
    };
    assert_eq!(switch(false).await.0, StatusCode::OK);
    assert_eq!(status_of(&s, s.owner).await, (None, None, None, None));
    assert_eq!(away_until().await, None);
    tick(&s, now + Duration::seconds(20)).await;
    assert_eq!(status_of(&s, s.owner).await.0, None, "stays off");
    assert_eq!(away_until().await, None);
    assert_eq!(switch(true).await.0, StatusCode::OK);
    tick(&s, now + Duration::seconds(40)).await;
    assert_eq!(
        status_of(&s, s.owner).await.0.as_deref(),
        Some("⛔"),
        "turned on again, the calendar's own status comes back"
    );
    assert_eq!(away_until().await, Some(tomorrow));

    lock(&s.fake).changes = vec![json!({ "id": "AAMk-ooo", "@removed": { "reason": "deleted" } })];
    due_now(&s).await;
    tick(&s, now + Duration::minutes(1)).await;
    assert_eq!(events(&s, s.owner).await.len(), 1);
    assert_eq!(status_of(&s, s.owner).await, (None, None, None, None));
    let away: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT calendar_away_until FROM users WHERE id = $1")
            .bind(s.owner)
            .fetch_one(&s.state.pool)
            .await
            .expect("away");
    assert_eq!(away, None);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn erasing_a_person_removes_their_calendar(pool: PgPool) {
    let s = setup(pool).await;
    set_up(&s, "google_calendar").await;
    let (member, _email, member_token) =
        seed_and_login(&s.app, &s.state, "cal-member", false).await;
    add_ws_member(&s.state, s.ws_id, member, "member").await;
    connect(&s, &member_token, "google_calendar").await;
    let now = Utc::now();
    lock(&s.fake).full = vec![
        meeting("a", "Planning", now + Duration::hours(2), 30),
        meeting("b", "Retro", now + Duration::hours(3), 30),
    ];
    due_now(&s).await;
    tick(&s, now).await;
    assert_eq!(events(&s, member).await.len(), 2);

    let (outsider, _email, outsider_token) =
        seed_and_login(&s.app, &s.state, "cal-out", false).await;
    let (status, _) = send(
        &s.app,
        "GET",
        &format!("/api/workspaces/{}/calendars", s.ws_id),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{outsider}");

    let (_, _, instance_admin) = seed_and_login(&s.app, &s.state, "cal-instance", true).await;
    let (status, body) = send(
        &s.app,
        "DELETE",
        &format!("/api/admin/users/{member}/data"),
        Some(&instance_admin),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["account"]["calendar_events"], json!(2));
    assert!(events(&s, member).await.is_empty());
    let connections: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM app_connections WHERE user_id = $1")
            .bind(member)
            .fetch_one(&s.state.pool)
            .await
            .expect("count");
    assert_eq!(connections, 0);
}
