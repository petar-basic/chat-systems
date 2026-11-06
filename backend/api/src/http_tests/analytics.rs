use std::sync::Arc;

use axum::http::StatusCode;
use axum::Router;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::analytics::consumer::count;
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

struct Team {
    app: Router,
    state: Arc<AppState>,
    ws_id: Uuid,
    owner: (Uuid, String),
    member: (Uuid, String),
    guest: (Uuid, String),
    public: Uuid,
    private: Uuid,
}

async fn team(pool: PgPool) -> Team {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "stats-owner", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "stats-member", false).await;
    let (guest_id, _, guest_token) = seed_and_login(&app, &state, "stats-guest", false).await;
    let ws_id = seed_workspace(&state, owner_id, &format!("ws-{}", Uuid::new_v4())).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    let public = seed_channel(
        &state,
        ws_id,
        owner_id,
        &format!("pub-{}", Uuid::new_v4()),
        false,
    )
    .await;
    let private = seed_channel(
        &state,
        ws_id,
        owner_id,
        &format!("prv-{}", Uuid::new_v4()),
        true,
    )
    .await;
    for user in [member_id, guest_id] {
        state
            .workspace_service
            .repo
            .add_channel_member(public, user, &ChannelRole::Member)
            .await
            .expect("join public");
    }
    Team {
        app,
        state,
        ws_id,
        owner: (owner_id, owner_token),
        member: (member_id, member_token),
        guest: (guest_id, guest_token),
        public,
        private,
    }
}

async fn post(app: &Router, token: &str, channel: Uuid, content: &str) -> Uuid {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/channels/{channel}/messages"),
        Some(token),
        Some(json!({ "content": content })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["id"].as_str().expect("id").parse().expect("uuid")
}

async fn react(app: &Router, token: &str, message: Uuid) {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/messages/{message}/reactions"),
        Some(token),
        Some(json!({ "emoji": "👍" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

async fn read(app: &Router, token: &str, channel: Uuid, message: Uuid) {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/channels/{channel}/read"),
        Some(token),
        Some(json!({ "message_id": message })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

async fn count_everything(state: &AppState) {
    let rows = sqlx::query_as::<_, (Uuid, String, Value, DateTime<Utc>)>(
        "SELECT event_id, event_type, payload, created_at FROM event_outbox ORDER BY id",
    )
    .fetch_all(&state.pool)
    .await
    .expect("outbox");
    for (id, event_type, payload, timestamp) in rows {
        let event = shared_events::Event {
            id,
            event_type,
            payload,
            timestamp,
            stream_id: None,
        };
        count(&state.analytics_repo, &event).await.expect("count");
    }
}

async fn analytics(team: &Team, token: &str, query: &str) -> (StatusCode, Value) {
    send(
        &team.app,
        "GET",
        &format!("/api/workspaces/{}/analytics?{query}", team.ws_id),
        Some(token),
        None,
    )
    .await
}

async fn members(team: &Team, token: &str, query: &str) -> (StatusCode, Value) {
    send(
        &team.app,
        "GET",
        &format!("/api/workspaces/{}/analytics/members?{query}", team.ws_id),
        Some(token),
        None,
    )
    .await
}

fn today(body: &Value) -> &Value {
    body["daily"]
        .as_array()
        .and_then(|days| days.last())
        .expect("today")
}

fn row_of(body: &Value, user: Uuid) -> &Value {
    body["data"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["user_id"] == user.to_string())
        .expect("member row")
}

async fn people_messages(state: &AppState, ws_id: Uuid, kinds: &[&str]) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages m JOIN channels c ON c.id = m.channel_id JOIN users u ON u.id = m.user_id
          WHERE c.workspace_id = $1 AND c.channel_type::text = ANY($2)
            AND m.metadata->>'kind' IS NULL AND NOT (m.metadata ? 'bot') AND NOT u.is_bot",
    )
    .bind(ws_id)
    .bind(kinds)
    .fetch_one(&state.pool)
    .await
    .expect("count messages")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_rollup_counts_each_event_once_and_agrees_with_the_database(pool: PgPool) {
    let team = team(pool).await;
    let (owner, owner_token) = &team.owner;
    let (member, member_token) = &team.member;
    let (guest, guest_token) = &team.guest;

    let first = post(&team.app, owner_token, team.public, "one").await;
    post(&team.app, owner_token, team.public, "two").await;
    post(&team.app, owner_token, team.private, "three").await;
    post(&team.app, member_token, team.public, "four").await;
    let (status, dm) = send(
        &team.app,
        "POST",
        &format!("/api/workspaces/{}/conversations", team.ws_id),
        Some(owner_token),
        Some(json!({ "participant_ids": [member] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{dm:?}");
    let dm: Uuid = dm["id"].as_str().expect("dm").parse().expect("uuid");
    post(&team.app, owner_token, dm, "five").await;
    react(&team.app, member_token, first).await;
    read(&team.app, guest_token, team.public, first).await;

    count_everything(&team.state).await;
    count_everything(&team.state).await;

    let (status, body) = analytics(&team, owner_token, "days=7").await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let day = today(&body);
    assert_eq!(
        day["public_messages"],
        people_messages(&team.state, team.ws_id, &["public"]).await
    );
    assert_eq!(day["public_messages"], 3);
    assert_eq!(day["private_messages"], 1);
    assert_eq!(day["direct_messages"], 1);
    assert_eq!(day["reactions"], 1);
    assert_eq!(day["active_members"], 3);
    assert_eq!(day["members"], 3);
    assert_eq!(day["joined"], 3);
    assert_eq!(body["active_members"], 3);
    assert_eq!(body["members"], 3);

    let (status, people) = members(&team, owner_token, "days=7").await;
    assert_eq!(status, StatusCode::OK, "{people:?}");
    let today = Utc::now().date_naive().to_string();
    assert_eq!(row_of(&people, *owner)["messages"], 4);
    assert_eq!(row_of(&people, *owner)["days_active"], 1);
    assert_eq!(row_of(&people, *owner)["last_active"], today);
    assert_eq!(row_of(&people, *member)["messages"], 1);
    assert_eq!(row_of(&people, *guest)["messages"], 0);
    assert_eq!(row_of(&people, *guest)["days_active"], 1);
    assert_eq!(row_of(&people, *guest)["role"], "guest");

    let text = people.to_string();
    for written in ["one", "two", "three", "four", "five"] {
        assert!(!text.contains(&format!("\"{written}\"")), "{text}");
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_backfill_counts_what_came_before_once(pool: PgPool) {
    let team = team(pool).await;
    let (owner, owner_token) = &team.owner;
    let (_, member_token) = &team.member;

    let first = post(&team.app, owner_token, team.public, "old").await;
    let older = post(&team.app, owner_token, team.public, "older").await;
    react(&team.app, member_token, first).await;
    sqlx::query("UPDATE messages SET created_at = NOW() - INTERVAL '40 days' WHERE id = $1")
        .bind(older)
        .execute(&team.state.pool)
        .await
        .expect("age a message");
    sqlx::query("UPDATE analytics_backfill SET counted_from = NOW()")
        .execute(&team.state.pool)
        .await
        .expect("move the line");

    assert!(team
        .state
        .analytics_repo
        .backfill()
        .await
        .expect("backfill"));
    assert!(!team.state.analytics_repo.backfill().await.expect("again"));
    count_everything(&team.state).await;

    let (_, week) = analytics(&team, owner_token, "days=7").await;
    assert_eq!(today(&week)["public_messages"], 1);
    assert_eq!(today(&week)["reactions"], 1);

    let (_, month) = members(&team, owner_token, "days=30").await;
    assert_eq!(row_of(&month, *owner)["messages"], 1);
    let (_, quarter) = members(&team, owner_token, "days=90").await;
    assert_eq!(row_of(&quarter, *owner)["messages"], 2);
    assert_eq!(row_of(&quarter, *owner)["days_active"], 2);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_window_is_seven_thirty_or_ninety_days(pool: PgPool) {
    let team = team(pool).await;
    let (_, owner_token) = &team.owner;
    let today = Utc::now().date_naive();

    for days in [7_i64, 30, 90] {
        let (status, body) = analytics(&team, owner_token, &format!("days={days}")).await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        assert_eq!(body["daily"].as_array().expect("daily").len() as i64, days);
        assert_eq!(body["from"], (today - Duration::days(days - 1)).to_string());
        assert_eq!(body["to"], today.to_string());
    }

    let (status, _) = analytics(&team, owner_token, "days=14").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_a_workspace_admin_sees_analytics(pool: PgPool) {
    let team = team(pool).await;
    let (member, member_token) = &team.member;
    let (_, guest_token) = &team.guest;

    for token in [member_token, guest_token] {
        let (status, _) = analytics(&team, token, "days=7").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = members(&team, token, "days=7").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    add_ws_member(&team.state, team.ws_id, *member, "admin").await;
    let (status, _) = analytics(&team, member_token, "days=7").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = members(&team, member_token, "days=7").await;
    assert_eq!(status, StatusCode::OK);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn guests_can_be_left_out(pool: PgPool) {
    let team = team(pool).await;
    let (_, owner_token) = &team.owner;
    let (guest, guest_token) = &team.guest;

    post(&team.app, owner_token, team.public, "from a member").await;
    post(&team.app, guest_token, team.public, "from a guest").await;
    count_everything(&team.state).await;

    let (_, everyone) = analytics(&team, owner_token, "days=7").await;
    assert_eq!(today(&everyone)["public_messages"], 2);
    assert_eq!(today(&everyone)["members"], 3);

    let (_, no_guests) = analytics(&team, owner_token, "days=7&guests=false").await;
    assert_eq!(today(&no_guests)["public_messages"], 1);
    assert_eq!(today(&no_guests)["members"], 2);
    assert_eq!(no_guests["active_members"], 1);

    let (_, people) = members(&team, owner_token, "days=7&guests=false").await;
    let rows = people["data"].as_array().expect("rows");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row["user_id"] != guest.to_string()));
}

fn channel_row(body: &Value, channel: Uuid) -> &Value {
    body["data"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["channel_id"] == channel.to_string())
        .expect("channel row")
}

async fn age_channel(state: &AppState, channel: Uuid, days: i32) {
    sqlx::query("UPDATE channels SET created_at = NOW() - make_interval(days => $2) WHERE id = $1")
        .bind(channel)
        .bind(days)
        .execute(&state.pool)
        .await
        .expect("age channel");
    sqlx::query(
        "UPDATE messages SET created_at = NOW() - make_interval(days => $2) WHERE channel_id = $1",
    )
    .bind(channel)
    .bind(days)
    .execute(&state.pool)
    .await
    .expect("age messages");
}

async fn quiet(team: &Team, token: &str, days: i32) -> (StatusCode, Value) {
    send(
        &team.app,
        "GET",
        &format!(
            "/api/workspaces/{}/analytics/quiet-channels?days={days}",
            team.ws_id
        ),
        Some(token),
        None,
    )
    .await
}

async fn archive(team: &Team, token: &str, channels: &[Uuid]) -> (StatusCode, Value) {
    send(
        &team.app,
        "POST",
        &format!(
            "/api/workspaces/{}/analytics/quiet-channels/archive",
            team.ws_id
        ),
        Some(token),
        Some(json!({ "channel_ids": channels })),
    )
    .await
}

fn ids(body: &Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["channel_id"].as_str().expect("id").to_string())
        .collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn each_channel_counts_its_posters_and_readers_and_adds_up_to_the_workspace(pool: PgPool) {
    let team = team(pool).await;
    let (_, owner_token) = &team.owner;
    let (_, member_token) = &team.member;
    let (_, guest_token) = &team.guest;

    let first = post(&team.app, owner_token, team.public, "one").await;
    post(&team.app, owner_token, team.public, "two").await;
    post(&team.app, member_token, team.public, "three").await;
    post(&team.app, owner_token, team.private, "four").await;
    read(&team.app, guest_token, team.public, first).await;
    count_everything(&team.state).await;
    count_everything(&team.state).await;

    let (status, channels) = send(
        &team.app,
        "GET",
        &format!("/api/workspaces/{}/analytics/channels?days=7", team.ws_id),
        Some(owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{channels:?}");
    let public = channel_row(&channels, team.public);
    assert_eq!(public["messages"], 3);
    assert_eq!(public["posters"], 2);
    assert_eq!(public["readers"], 1);
    assert_eq!(public["members"], 3);
    assert_eq!(public["channel_type"], "public");
    assert!(public["last_message_at"].is_string());
    let private = channel_row(&channels, team.private);
    assert_eq!(private["messages"], 1);
    assert_eq!(private["channel_type"], "private");

    let summed: i64 = channels["data"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["messages"].as_i64().expect("messages"))
        .sum();
    let (_, workspace) = analytics(&team, owner_token, "days=7").await;
    let day = today(&workspace);
    assert_eq!(
        summed,
        day["public_messages"].as_i64().expect("public")
            + day["private_messages"].as_i64().expect("private")
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_is_quiet_after_its_last_message_ages_past_the_threshold(pool: PgPool) {
    let team = team(pool).await;
    let (owner, owner_token) = &team.owner;
    let old = seed_channel(&team.state, team.ws_id, *owner, "old-one", false).await;
    let older = seed_channel(&team.state, team.ws_id, *owner, "older-one", true).await;
    post(&team.app, owner_token, old, "a while ago").await;
    post(&team.app, owner_token, older, "long ago").await;
    post(&team.app, owner_token, team.public, "today").await;
    age_channel(&team.state, old, 45).await;
    age_channel(&team.state, older, 100).await;
    age_channel(&team.state, team.private, 200).await;
    sqlx::query(
        "INSERT INTO messages (channel_id, user_id, content, metadata)
         VALUES ($1, $2, 'joined', '{\"kind\": \"channel_join\"}')",
    )
    .bind(older)
    .bind(owner)
    .execute(&team.state.pool)
    .await
    .expect("system message");

    let (status, month) = quiet(&team, owner_token, 30).await;
    assert_eq!(status, StatusCode::OK, "{month:?}");
    assert_eq!(
        ids(&month),
        [team.private, older, old].map(|id| id.to_string())
    );
    let (_, quarter) = quiet(&team, owner_token, 90).await;
    assert_eq!(
        ids(&quarter),
        [team.private, older].map(|id| id.to_string())
    );
    let (_, half) = quiet(&team, owner_token, 180).await;
    assert_eq!(ids(&half), [team.private.to_string()]);
    assert!(half["data"][0]["last_message_at"].is_null());

    let (status, _) = quiet(&team, owner_token, 14).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn quiet_channels_archive_together_each_one_audited(pool: PgPool) {
    let team = team(pool).await;
    let (owner, owner_token) = &team.owner;
    let (_, member_token) = &team.member;
    let old = seed_channel(&team.state, team.ws_id, *owner, "dusty", false).await;
    age_channel(&team.state, old, 120).await;
    age_channel(&team.state, team.private, 120).await;

    let (status, _) = archive(&team, member_token, &[old]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let general: Uuid =
        sqlx::query_scalar("SELECT id FROM channels WHERE workspace_id = $1 AND is_default")
            .bind(team.ws_id)
            .fetch_one(&team.state.pool)
            .await
            .expect("default channel");
    let (status, _) = archive(&team, owner_token, &[old, general]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = archive(&team, owner_token, &[old, Uuid::new_v4()]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, still) = quiet(&team, owner_token, 90).await;
    assert_eq!(ids(&still).len(), 2, "a refused batch archives nothing");

    let (status, body) = archive(&team, owner_token, &[old, team.private, old]).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["archived"].as_array().expect("archived").len(), 2);

    let archived: Vec<bool> =
        sqlx::query_scalar("SELECT is_archived FROM channels WHERE id = ANY($1) ORDER BY id")
            .bind(vec![old, team.private])
            .fetch_all(&team.state.pool)
            .await
            .expect("archived");
    assert_eq!(archived, [true, true]);
    let audited: Vec<Uuid> = sqlx::query_scalar(
        "SELECT resource_id FROM audit_log WHERE action = 'channel.archived' AND workspace_id = $1 ORDER BY resource_id",
    )
    .bind(team.ws_id)
    .fetch_all(&team.state.pool)
    .await
    .expect("audit");
    let mut expected = vec![old, team.private];
    expected.sort();
    assert_eq!(audited, expected);
    let notes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE channel_id = ANY($1) AND metadata->>'kind' = 'channel_archived'",
    )
    .bind(vec![old, team.private])
    .fetch_one(&team.state.pool)
    .await
    .expect("notes");
    assert_eq!(notes, 2);

    let (_, after) = quiet(&team, owner_token, 90).await;
    assert!(ids(&after).is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_admin_sees_their_own_channel_and_no_other(pool: PgPool) {
    let team = team(pool).await;
    let (owner_token, member) = (&team.owner.1, &team.member);
    let (_, guest_token) = &team.guest;
    team.state
        .workspace_service
        .repo
        .update_channel_member_role(team.public, member.0, &ChannelRole::Admin)
        .await
        .expect("make channel admin");
    post(&team.app, owner_token, team.public, "hello").await;
    count_everything(&team.state).await;

    let (status, mine) = send(
        &team.app,
        "GET",
        &format!("/api/channels/{}/analytics", team.public),
        Some(&member.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mine:?}");
    assert_eq!(mine["channel"]["channel_id"], team.public.to_string());
    assert_eq!(mine["channel"]["messages"], 1);
    assert_eq!(
        mine["from"],
        (Utc::now().date_naive() - Duration::days(29)).to_string()
    );

    for (token, channel) in [(&member.1, team.private), (guest_token, team.public)] {
        let (status, _) = send(
            &team.app,
            "GET",
            &format!("/api/channels/{channel}/analytics"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, _) = send(
        &team.app,
        "GET",
        &format!("/api/workspaces/{}/analytics/channels?days=30", team.ws_id),
        Some(&member.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
