use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use super::interactions::{fake_app, setup, Setup};

fn helpdesk_shortcuts() -> Value {
    json!({ "shortcuts": [
        { "kind": "global", "name": "New request", "description": "Ask the help desk for something",
          "callback_id": "new_request" },
        { "kind": "message", "name": "Create ticket from message",
          "description": "Turn this message into a ticket", "callback_id": "ticket_from_message" },
    ] })
}

impl Setup {
    async fn save_shortcuts(&self, token: &str, body: Value) -> (StatusCode, Value) {
        send(
            &self.app,
            "PUT",
            &format!("/api/v1/bot-apps/{}/shortcuts", self.app_id),
            Some(token),
            Some(body),
        )
        .await
    }

    async fn active_apps(&self, token: &str) -> Vec<Value> {
        let (status, body) = send(
            &self.app,
            "GET",
            &format!("/api/v1/workspaces/{}/active-apps", self.ws_id),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body["data"].as_array().expect("data").clone()
    }

    async fn message_shortcuts(&self, token: &str, channel_id: Uuid) -> Vec<Value> {
        let (status, body) = send(
            &self.app,
            "GET",
            &format!("/api/v1/channels/{channel_id}/context"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body["message_shortcuts"]
            .as_array()
            .expect("message shortcuts")
            .clone()
    }

    async fn post_as(&self, token: &str, channel_id: Uuid, text: &str) -> Uuid {
        let (status, body) = send(
            &self.app,
            "POST",
            &format!("/api/v1/channels/{channel_id}/messages"),
            Some(token),
            Some(json!({ "content": text })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body["id"].as_str().expect("id").parse().expect("uuid")
    }

    async fn run(&self, token: &str, shortcut_id: &Value, body: Value) -> (StatusCode, Value) {
        send(
            &self.app,
            "POST",
            &format!(
                "/api/v1/app-shortcuts/{}/run",
                shortcut_id.as_str().expect("id")
            ),
            Some(token),
            Some(body),
        )
        .await
    }

    async fn saved_helpdesk(&self) -> (Value, Value) {
        let (status, body) = self
            .save_shortcuts(&self.owner_token, helpdesk_shortcuts())
            .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        let of = |kind: &str| {
            body["data"]
                .as_array()
                .expect("data")
                .iter()
                .find(|s| s["kind"] == kind)
                .expect("shortcut")["id"]
                .clone()
        };
        (of("global"), of("message"))
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn admins_save_at_most_five_shortcuts_of_each_kind(pool: PgPool) {
    let s = setup(pool).await;

    let (status, _) = s
        .save_shortcuts(&s.member_token, helpdesk_shortcuts())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a member cannot");

    let six: Vec<Value> = (0..6)
        .map(|i| {
            json!({ "kind": "global", "name": format!("Shortcut {i}"), "description": "Does a thing",
                    "callback_id": format!("s{i}") })
        })
        .collect();
    let (status, body) = s
        .save_shortcuts(&s.owner_token, json!({ "shortcuts": six }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");

    let twice = json!({ "shortcuts": [
        { "kind": "global", "name": "One", "description": "First", "callback_id": "same" },
        { "kind": "message", "name": "Two", "description": "Second", "callback_id": "same" },
    ] });
    let (status, body) = s.save_shortcuts(&s.owner_token, twice).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");

    let spaced = json!({ "shortcuts": [
        { "kind": "global", "name": "One", "description": "First", "callback_id": "new request" },
    ] });
    let (status, body) = s.save_shortcuts(&s.owner_token, spaced).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");

    let (global, _) = s.saved_helpdesk().await;
    let renamed = json!({ "shortcuts": [
        { "kind": "global", "name": "Ask for help", "description": "Ask the help desk",
          "callback_id": "new_request" },
    ] });
    let (status, body) = s.save_shortcuts(&s.owner_token, renamed).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let data = body["data"].as_array().expect("data");
    assert_eq!(data.len(), 1, "the message shortcut is gone");
    assert_eq!(data[0]["id"], global, "the same callback ID keeps its id");
    assert_eq!(data[0]["name"], "Ask for help");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_global_shortcut_reaches_the_app_and_opens_its_modal(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let (global, _) = s.saved_helpdesk().await;

    let apps = s.active_apps(&s.member_token).await;
    let shortcuts = apps[0]["shortcuts"].as_array().expect("shortcuts");
    assert_eq!(shortcuts.len(), 1, "only the global one: {shortcuts:?}");
    assert_eq!(shortcuts[0]["callback_id"], "new_request");

    let (status, body) = s
        .run(&s.member_token, &global, json!({ "channel_id": s.ch_id }))
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let payload = fake.last();
    assert_eq!(payload["type"], "shortcut");
    assert_eq!(payload["callback_id"], "new_request");
    assert_eq!(payload["user"]["id"], json!(s.member_id));
    assert_eq!(payload["channel"]["id"], json!(s.ch_id));
    assert!(payload.get("response_url").is_none());

    let (status, view) = s
        .as_bot(
            "/api/v1/views/open",
            json!({ "trigger_id": payload["trigger_id"], "view": {
                "type": "modal", "callback_id": "request",
                "title": { "type": "plain_text", "text": "New request" },
                "blocks": [{ "type": "section", "text": { "type": "mrkdwn", "text": "What do you need?" } }],
            } }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view:?}");

    let (status, _) = s.run(&s.member_token, &global, json!({})).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a global shortcut needs no conversation"
    );
    assert_eq!(fake.last()["channel"], Value::Null);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_message_shortcut_carries_the_message_only_where_the_app_is(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let (_, message_shortcut) = s.saved_helpdesk().await;

    let listed = s.message_shortcuts(&s.member_token, s.ch_id).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["callback_id"], "ticket_from_message");

    let msg_id = s
        .post_as(&s.member_token, s.ch_id, "The printer on floor 3 is jammed")
        .await;
    let (status, body) = s.run(&s.member_token, &message_shortcut, json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");

    let (status, body) = s
        .run(
            &s.member_token,
            &message_shortcut,
            json!({ "message_id": msg_id }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let payload = fake.last();
    assert_eq!(payload["type"], "message_action");
    assert_eq!(payload["callback_id"], "ticket_from_message");
    assert_eq!(payload["channel"]["id"], json!(s.ch_id));
    assert_eq!(payload["message"]["id"], json!(msg_id));
    assert_eq!(
        payload["message"]["text"],
        "The printer on floor 3 is jammed"
    );
    assert!(payload["trigger_id"].is_string());

    let response_url = payload["response_url"].as_str().expect("response url");
    let (status, body) = s
        .respond(
            response_url,
            json!({ "replace_original": true, "text": "Taken over" }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");
    let (status, body) = s
        .respond(response_url, json!({ "delete_original": true }))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");
    assert!(
        s.message(msg_id).await.is_some(),
        "the message is untouched"
    );
    let (status, body) = s
        .respond(
            response_url,
            json!({ "response_type": "in_channel", "text": "Ticket #12 created" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["status"], "posted");

    let owner_id: Uuid = sqlx::query_scalar("SELECT created_by FROM channels WHERE id = $1")
        .bind(s.ch_id)
        .fetch_one(&s.state.pool)
        .await
        .expect("owner");
    let elsewhere = seed_channel(&s.state, s.ws_id, owner_id, "lunch", false).await;
    let (status, body) = send(
        &s.app,
        "POST",
        &format!("/api/v1/channels/{elsewhere}/members"),
        Some(&s.owner_token),
        Some(json!({ "user_id": s.member_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(s
        .message_shortcuts(&s.member_token, elsewhere)
        .await
        .is_empty());
    let there = s.post_as(&s.member_token, elsewhere, "Pizza?").await;
    let before = fake.payloads().len();
    let (status, body) = s
        .run(
            &s.member_token,
            &message_shortcut,
            json!({ "message_id": there }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");
    assert_eq!(fake.payloads().len(), before, "nothing reached the app");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn guests_and_a_turned_off_app_have_no_shortcuts(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let (global, message_shortcut) = s.saved_helpdesk().await;

    let (guest_id, _, guest_token) = seed_and_login(&s.app, &s.state, "act-guest", false).await;
    add_ws_member(&s.state, s.ws_id, guest_id, "guest").await;
    s.add(guest_id).await;
    let apps = s.active_apps(&guest_token).await;
    assert_eq!(apps.len(), 1, "a guest still sees the app");
    assert!(apps[0]["shortcuts"]
        .as_array()
        .expect("shortcuts")
        .is_empty());
    assert!(s.message_shortcuts(&guest_token, s.ch_id).await.is_empty());
    for (token, expected) in [(&guest_token, 0), (&s.member_token, 1)] {
        let (status, body) = send(
            &s.app,
            "GET",
            &format!("/api/v1/channels/{}/message-shortcuts", s.ch_id),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        assert_eq!(body["data"].as_array().expect("data").len(), expected);
    }
    let (status, _) = s
        .run(&guest_token, &global, json!({ "channel_id": s.ch_id }))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(fake.payloads().is_empty());

    let msg_id = s.post_as(&s.member_token, s.ch_id, "Help").await;
    sqlx::query("UPDATE bot_apps SET disabled_at = NOW() WHERE id = $1")
        .bind(s.app_id)
        .execute(&s.state.pool)
        .await
        .expect("turn the app off");
    assert!(s.active_apps(&s.member_token).await.is_empty());
    assert!(s
        .message_shortcuts(&s.member_token, s.ch_id)
        .await
        .is_empty());
    let (status, _) = s
        .run(&s.member_token, &global, json!({ "channel_id": s.ch_id }))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = s
        .run(
            &s.member_token,
            &message_shortcut,
            json!({ "message_id": msg_id }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(fake.payloads().is_empty(), "nothing reached the app");
}
