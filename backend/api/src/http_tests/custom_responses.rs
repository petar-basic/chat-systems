use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use shared_events::Event;

use super::common::*;
use crate::custom_responses::consumer;
use crate::state::AppState;

struct Team {
    ws: Uuid,
    owner_id: Uuid,
    admin: String,
    ana_id: Uuid,
    ana: String,
    ben_id: Uuid,
    help: Uuid,
}

async fn team(app: &axum::Router, state: &AppState) -> Team {
    let (owner_id, _, _) = seed_and_login(app, state, "cr-owner", false).await;
    let ws = seed_workspace(state, owner_id, "Custom Responses").await;
    let (admin_id, _, admin) = seed_and_login(app, state, "cr-admin", false).await;
    add_ws_member(state, ws, admin_id, "admin").await;
    let (ana_id, _, ana) = seed_and_login(app, state, "cr-ana", false).await;
    add_ws_member(state, ws, ana_id, "member").await;
    let (ben_id, _, _) = seed_and_login(app, state, "cr-ben", false).await;
    add_ws_member(state, ws, ben_id, "member").await;
    let help = open_channel(state, ws, owner_id, "it-help", &[ana_id, ben_id]).await;
    Team {
        ws,
        owner_id,
        admin,
        ana_id,
        ana,
        ben_id,
        help,
    }
}

async fn open_channel(
    state: &AppState,
    ws: Uuid,
    owner_id: Uuid,
    name: &str,
    members: &[Uuid],
) -> Uuid {
    let ch = seed_channel(state, ws, owner_id, name, false).await;
    for member in members {
        state
            .workspace_service
            .repo
            .add_channel_member(ch, *member, &crate::workspace::models::ChannelRole::Member)
            .await
            .expect("join");
    }
    ch
}

fn wifi() -> Value {
    json!({ "triggers": ["wifi password", "WiFi Password", "  "], "answers": ["It's on the fridge"] })
}

async fn create(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/custom-responses"),
        Some(token),
        Some(body),
    )
    .await
}

async fn created(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> Value {
    let (status, response) = create(app, token, ws, body).await;
    assert_eq!(status, StatusCode::OK, "{response:?}");
    response
}

async fn list(app: &axum::Router, token: &str, ws: Uuid) -> (StatusCode, Value) {
    send(
        app,
        "GET",
        &format!("/api/workspaces/{ws}/custom-responses"),
        Some(token),
        None,
    )
    .await
}

async fn say(app: &axum::Router, token: &str, ch: Uuid, content: &str) -> Uuid {
    say_in(app, token, ch, None, content).await
}

async fn say_in(
    app: &axum::Router,
    token: &str,
    ch: Uuid,
    thread: Option<Uuid>,
    content: &str,
) -> Uuid {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(token),
        Some(json!({ "content": content, "thread_parent_id": thread })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["id"].as_str().expect("id").parse().expect("uuid")
}

fn created_event(message_id: Uuid) -> Event {
    Event {
        id: Uuid::new_v4(),
        event_type: "message.created".into(),
        payload: json!({ "id": message_id }),
        timestamp: chrono::Utc::now(),
        stream_id: None,
    }
}

async fn deliver(state: &AppState) -> usize {
    let rows = sqlx::query_as::<_, (Uuid, String, Value, chrono::DateTime<chrono::Utc>)>(
        "SELECT event_id, event_type, payload, created_at FROM event_outbox
          WHERE event_type = 'message.created' ORDER BY id",
    )
    .fetch_all(&state.pool)
    .await
    .expect("outbox");
    let mut answered = 0;
    for (id, event_type, payload, timestamp) in rows {
        let event = Event {
            id,
            event_type,
            payload,
            timestamp,
            stream_id: None,
        };
        for _ in 0..2 {
            if consumer::handle(state, &event)
                .await
                .expect("handle")
                .is_some()
            {
                answered += 1;
            }
        }
    }
    answered
}

async fn bot_says(state: &AppState, ch: Uuid) -> Vec<(String, Option<Uuid>)> {
    sqlx::query_as(
        "SELECT m.content, m.thread_parent_id
           FROM messages m JOIN users u ON u.id = m.user_id
          WHERE m.channel_id = $1 AND u.is_bot
          ORDER BY m.created_at, m.id",
    )
    .bind(ch)
    .fetch_all(&state.pool)
    .await
    .expect("bot messages")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn members_edit_responses_unless_the_rule_says_admins(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "cr-guest", false).await;
    add_ws_member(&state, t.ws, guest_id, "guest").await;
    let (_, _, outsider) = seed_and_login(&app, &state, "cr-out", false).await;

    let response = created(&app, &t.ana, t.ws, wifi()).await;
    assert_eq!(
        response["triggers"],
        json!(["wifi password"]),
        "blank and repeated phrases are dropped"
    );
    let id = response["id"].as_str().expect("id").to_string();

    let (status, page) = list(&app, &t.ana, t.ws).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["data"].as_array().expect("data").len(), 1);
    assert_eq!(page["can_edit"], true);
    assert_eq!(page["can_rename"], false);
    assert_eq!(page["bot"]["name"], "Test");
    assert!(
        page["bot"]["user_id"].is_string(),
        "the first response made the bot"
    );

    for token in [&guest, &outsider] {
        assert_eq!(list(&app, token, t.ws).await.0, StatusCode::FORBIDDEN);
        assert_eq!(
            create(&app, token, t.ws, wifi()).await.0,
            StatusCode::FORBIDDEN
        );
    }

    for body in [
        json!({ "triggers": [" "], "answers": ["x"] }),
        json!({ "triggers": ["x"], "answers": [""] }),
        json!({ "triggers": ["x".repeat(101)], "answers": ["x"] }),
        json!({ "triggers": ["x"], "answers": ["x".repeat(4001)] }),
        json!({ "triggers": (0..21).map(|i| format!("p{i}")).collect::<Vec<_>>(), "answers": ["x"] }),
    ] {
        assert_eq!(
            create(&app, &t.ana, t.ws, body).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{}", t.ws),
        Some(&t.admin),
        Some(json!({ "custom_responders": "admins" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, page) = list(&app, &t.ana, t.ws).await;
    assert_eq!(status, StatusCode::OK, "members still read them");
    assert_eq!(page["can_edit"], false);
    assert_eq!(
        create(&app, &t.ana, t.ws, wifi()).await.0,
        StatusCode::FORBIDDEN
    );
    let path = format!("/api/workspaces/{}/custom-responses/{id}", t.ws);
    let (status, _) = send(
        &app,
        "PATCH",
        &path,
        Some(&t.ana),
        Some(json!({ "answers": ["Ask IT"] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        send(&app, "DELETE", &path, Some(&t.ana), None).await.0,
        StatusCode::FORBIDDEN
    );

    let (status, updated) = send(
        &app,
        "PATCH",
        &path,
        Some(&t.admin),
        Some(json!({ "answers": ["Ask IT", "It's on the fridge"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated:?}");
    assert_eq!(updated["triggers"], json!(["wifi password"]));
    assert_eq!(updated["answers"], json!(["Ask IT", "It's on the fridge"]));

    let rename = format!("/api/workspaces/{}/custom-responses/bot", t.ws);
    assert_eq!(
        send(
            &app,
            "PUT",
            &rename,
            Some(&t.ana),
            Some(json!({ "name": "Helper" }))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, bot) = send(
        &app,
        "PUT",
        &rename,
        Some(&t.admin),
        Some(json!({ "name": "  Helper  " })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{bot:?}");
    assert_eq!(bot["name"], "Helper");
    assert_eq!(list(&app, &t.admin, t.ws).await.1["bot"]["name"], "Helper");

    assert_eq!(
        send(&app, "DELETE", &path, Some(&t.admin), None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        send(&app, "DELETE", &path, Some(&t.admin), None).await.0,
        StatusCode::NOT_FOUND
    );

    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE workspace_id = $1
            AND resource_type IN ('custom_response', 'workspace_bot')
          ORDER BY created_at",
    )
    .bind(t.ws)
    .fetch_all(&state.pool)
    .await
    .expect("audit");
    assert_eq!(
        actions,
        [
            "custom_response.created",
            "custom_response.updated",
            "workspace_bot.renamed",
            "custom_response.deleted"
        ]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_bot_answers_once_in_the_channel_or_the_thread(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    created(&app, &t.admin, t.ws, wifi()).await;
    created(
        &app,
        &t.admin,
        t.ws,
        json!({ "triggers": ["vpn guide"], "answers": ["See the wiki"] }),
    )
    .await;

    say(&app, &t.ana, t.help, "what's the WiFi password?").await;
    say(&app, &t.ana, t.help, "nothing to see here").await;
    assert_eq!(deliver(&state).await, 1);
    assert_eq!(
        deliver(&state).await,
        0,
        "a replayed entry is not answered again"
    );
    assert_eq!(
        bot_says(&state, t.help).await,
        [("It's on the fridge".to_string(), None)]
    );

    let root = say(&app, &t.ana, t.help, "IT questions").await;
    say_in(&app, &t.ana, t.help, Some(root), "where is the VPN guide?").await;
    assert_eq!(deliver(&state).await, 1);
    assert_eq!(
        bot_says(&state, t.help).await[1],
        ("See the wiki".to_string(), Some(root)),
        "a thread is answered in the thread"
    );

    let bot: (Uuid, bool, String) = sqlx::query_as(
        "SELECT u.id, u.is_bot, u.display_name FROM workspace_bots b JOIN users u ON u.id = b.user_id
          WHERE b.workspace_id = $1 AND b.kind = 'responses'",
    )
    .bind(t.ws)
    .fetch_one(&state.pool)
    .await
    .expect("bot");
    assert!(bot.1);
    assert_eq!(bot.2, "Test");
    let replies: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM custom_response_replies r JOIN messages m ON m.id = r.reply_id
          WHERE m.user_id = $1",
    )
    .bind(bot.0)
    .fetch_one(&state.pool)
    .await
    .expect("replies");
    assert_eq!(replies, 2);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_response_answers_once_a_minute_in_a_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let random = open_channel(&state, t.ws, t.owner_id, "random", &[t.ana_id]).await;
    created(&app, &t.admin, t.ws, wifi()).await;

    say(&app, &t.ana, t.help, "wifi password?").await;
    say(&app, &t.ana, t.help, "wifi password??").await;
    say(&app, &t.ana, random, "wifi password").await;
    assert_eq!(deliver(&state).await, 2);
    assert_eq!(bot_says(&state, t.help).await.len(), 1);
    assert_eq!(bot_says(&state, random).await.len(), 1);

    sqlx::query(
        "UPDATE custom_response_replies SET answered_at = answered_at - INTERVAL '61 seconds'",
    )
    .execute(&state.pool)
    .await
    .expect("age");
    let later = say(&app, &t.ana, t.help, "the wifi password again").await;
    assert!(consumer::handle(&state, &created_event(later))
        .await
        .expect("handle")
        .is_some());
    assert_eq!(bot_says(&state, t.help).await.len(), 2);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn bots_archived_channels_and_direct_messages_get_no_answer(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    created(&app, &t.admin, t.ws, wifi()).await;

    let hooked = say(&app, &t.ana, t.help, "wifi password").await;
    sqlx::query("UPDATE messages SET metadata = '{\"bot\": {\"name\": \"CI\"}}' WHERE id = $1")
        .bind(hooked)
        .execute(&state.pool)
        .await
        .expect("hook message");
    assert!(consumer::handle(&state, &created_event(hooked))
        .await
        .expect("handle")
        .is_none());

    let (robot_id, _) = seed(&state, "cr-robot", false).await;
    add_ws_member(&state, t.ws, robot_id, "member").await;
    sqlx::query("UPDATE users SET is_bot = true WHERE id = $1")
        .bind(robot_id)
        .execute(&state.pool)
        .await
        .expect("bot user");
    let robot: Uuid = sqlx::query_scalar(
        "INSERT INTO messages (channel_id, user_id, content) VALUES ($1, $2, 'wifi password') RETURNING id",
    )
    .bind(t.help)
    .bind(robot_id)
    .fetch_one(&state.pool)
    .await
    .expect("bot message");
    assert!(consumer::handle(&state, &created_event(robot))
        .await
        .expect("handle")
        .is_none());

    let (status, dm) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{}/conversations", t.ws),
        Some(&t.ana),
        Some(json!({ "participant_ids": [t.ben_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{dm:?}");
    let dm: Uuid = dm["id"].as_str().expect("id").parse().expect("uuid");
    let direct = say(&app, &t.ana, dm, "wifi password").await;
    assert!(consumer::handle(&state, &created_event(direct))
        .await
        .expect("handle")
        .is_none());

    let (status, group) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{}/conversations", t.ws),
        Some(&t.ana),
        Some(json!({ "participant_ids": [t.ben_id, t.owner_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{group:?}");
    assert_eq!(group["kind"], "group");
    let group: Uuid = group["id"].as_str().expect("id").parse().expect("uuid");
    let grouped = say(&app, &t.ana, group, "wifi password").await;
    assert!(consumer::handle(&state, &created_event(grouped))
        .await
        .expect("handle")
        .is_none());

    let archived = open_channel(&state, t.ws, t.owner_id, "old", &[t.ana_id]).await;
    let late = say(&app, &t.ana, archived, "wifi password").await;
    sqlx::query("UPDATE channels SET is_archived = true WHERE id = $1")
        .bind(archived)
        .execute(&state.pool)
        .await
        .expect("archive");
    assert!(consumer::handle(&state, &created_event(late))
        .await
        .expect("handle")
        .is_none());

    for ch in [t.help, dm, group, archived] {
        assert!(bot_says(&state, ch)
            .await
            .iter()
            .all(|(content, _)| content != "It's on the fridge"));
    }
    let claimed: i64 = sqlx::query_scalar("SELECT count(*) FROM custom_response_replies")
        .fetch_one(&state.pool)
        .await
        .expect("claims");
    assert_eq!(claimed, 0);
}
