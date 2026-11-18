use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use super::interactions::{fake_app, setup, Setup};

fn requests_home(lines: &[&str]) -> Value {
    let mut blocks = vec![
        json!({ "type": "header", "text": { "type": "plain_text", "text": "Your requests" } }),
        json!({ "type": "actions", "block_id": "home", "elements": [
            { "type": "button", "action_id": "new_request",
              "text": { "type": "plain_text", "text": "New request" } }
        ] }),
    ];
    blocks.extend(
        lines
            .iter()
            .map(|line| json!({ "type": "section", "text": { "type": "mrkdwn", "text": line } })),
    );
    json!({ "type": "home", "callback_id": "requests", "blocks": blocks })
}

fn texts(home: &Value) -> Vec<String> {
    home["blocks"]
        .as_array()
        .expect("blocks")
        .iter()
        .filter(|block| block["type"] == "section")
        .filter_map(|block| block["text"]["text"].as_str().map(str::to_string))
        .collect()
}

impl Setup {
    async fn publish_home(
        &self,
        user_id: Uuid,
        view: Value,
        hash: Option<&str>,
    ) -> (StatusCode, Value) {
        self.as_bot(
            "/api/views/publish",
            json!({ "user_id": user_id, "view": view, "hash": hash }),
        )
        .await
    }

    async fn home_of(&self, token: &str) -> (StatusCode, Value) {
        send(
            &self.app,
            "GET",
            &format!("/api/apps/{}/home", self.app_id),
            Some(token),
            None,
        )
        .await
    }

    async fn conversation_with_app(&self, token: &str) -> Uuid {
        let (status, dm) = send(
            &self.app,
            "POST",
            &format!("/api/workspaces/{}/conversations", self.ws_id),
            Some(token),
            Some(json!({ "participant_ids": [self.bot_user_id] })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{dm:?}");
        dm["id"].as_str().expect("id").parse().expect("uuid")
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn each_person_reads_only_the_home_published_for_them(pool: PgPool) {
    let s = setup(pool).await;
    s.configure(json!({ "home_enabled": true })).await;
    let (bob_id, _, bob_token) = seed_and_login(&s.app, &s.state, "home-bob", false).await;
    add_ws_member(&s.state, s.ws_id, bob_id, "member").await;

    let (status, ana_home) = s
        .publish_home(s.member_id, requests_home(&["Laptop: waiting"]), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{ana_home:?}");
    let (status, body) = s
        .publish_home(
            bob_id,
            requests_home(&["Monitor: shipped", "Mouse: waiting"]),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (status, ana) = s.home_of(&s.member_token).await;
    assert_eq!(status, StatusCode::OK, "{ana:?}");
    assert_eq!(texts(&ana["home"]), ["Laptop: waiting"]);
    let (_, bob) = s.home_of(&bob_token).await;
    assert_eq!(texts(&bob["home"]), ["Monitor: shipped", "Mouse: waiting"]);
    let (status, owner) = s.home_of(&s.owner_token).await;
    assert_eq!(status, StatusCode::OK, "{owner:?}");
    assert_eq!(
        owner["home"],
        Value::Null,
        "nothing published for the owner"
    );

    let (status, again) = s
        .publish_home(
            s.member_id,
            requests_home(&["Laptop: shipped"]),
            ana_home["hash"].as_str(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{again:?}");
    assert_eq!(again["id"], ana_home["id"], "a republish replaces the Home");
    assert_ne!(again["hash"], ana_home["hash"]);
    let (_, ana) = s.home_of(&s.member_token).await;
    assert_eq!(texts(&ana["home"]), ["Laptop: shipped"]);
    let read_back = json!({ "type": "home", "blocks": ana["home"]["blocks"] });
    let (status, body) = s.publish_home(s.member_id, read_back, None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the Home as read is published back: {body:?}"
    );
    assert_eq!(texts(&body), ["Laptop: shipped"]);

    let (status, body) = s
        .publish_home(
            s.member_id,
            requests_home(&["Stale"]),
            ana_home["hash"].as_str(),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");

    let (stranger_id, _, _) = seed_and_login(&s.app, &s.state, "home-stranger", false).await;
    let (status, body) = s
        .publish_home(stranger_id, requests_home(&["Nope"]), None)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");

    let (status, _) = send(
        &s.app,
        "POST",
        "/api/views/publish",
        Some(&s.member_token),
        Some(json!({ "user_id": s.member_id, "view": requests_home(&[]) })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "only the app's bot publishes"
    );

    let input = json!({ "type": "home", "blocks": [
        { "type": "input", "block_id": "what", "label": { "type": "plain_text", "text": "What" },
          "element": { "type": "plain_text_input", "action_id": "text" } }
    ] });
    let (status, body) = s.publish_home(s.member_id, input, None).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a Home has no inputs: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_home_tab_is_there_only_while_it_is_on(pool: PgPool) {
    let s = setup(pool).await;
    let home_flag = |s: &Setup| {
        let (app, token, ws_id) = (s.app.clone(), s.member_token.clone(), s.ws_id);
        async move {
            let (status, body) = send(
                &app,
                "GET",
                &format!("/api/workspaces/{ws_id}/active-apps"),
                Some(&token),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body:?}");
            body["data"]
                .as_array()
                .expect("data")
                .first()
                .map(|app| app["home"].clone())
        }
    };

    assert_eq!(home_flag(&s).await, Some(json!(false)));
    let (status, _) = s
        .publish_home(s.member_id, requests_home(&["Laptop"]), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "the Home tab is off");
    let (status, _) = s.home_of(&s.member_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    s.configure(json!({ "home_enabled": true })).await;
    assert_eq!(home_flag(&s).await, Some(json!(true)));
    let (status, _) = s
        .publish_home(s.member_id, requests_home(&["Laptop"]), None)
        .await;
    assert_eq!(status, StatusCode::OK);

    sqlx::query("UPDATE bot_apps SET disabled_at = NOW() WHERE id = $1")
        .bind(s.app_id)
        .execute(&s.state.pool)
        .await
        .expect("turn the app off");
    assert_eq!(home_flag(&s).await, None, "a turned-off app is not listed");
    let (status, _) = s.home_of(&s.member_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = s
        .publish_home(s.member_id, requests_home(&["Laptop"]), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn opening_the_home_tells_an_app_that_listens(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.configure(json!({ "request_url": fake.url, "home_enabled": true }))
        .await;
    let dm = s.conversation_with_app(&s.member_token).await;
    let opened = |channel_id: Uuid| {
        let (app, token, app_id) = (s.app.clone(), s.member_token.clone(), s.app_id);
        async move {
            send(
                &app,
                "POST",
                &format!("/api/apps/{app_id}/home/opened"),
                Some(&token),
                Some(json!({ "channel_id": channel_id })),
            )
            .await
        }
    };
    let queued = |s: &Setup| {
        let (pool, app_id) = (s.state.pool.clone(), s.app_id);
        async move {
            sqlx::query_as::<_, (Value,)>(
                "SELECT body FROM app_deliveries
                  WHERE app_id = $1 AND kind = 'event' AND body->>'type' = 'app_home_opened'",
            )
            .bind(app_id)
            .fetch_all(&pool)
            .await
            .expect("deliveries")
        }
    };

    let (status, body) = opened(dm).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(queued(&s).await.is_empty(), "the app does not listen yet");

    s.configure(json!({ "events": ["app_home_opened"] })).await;
    let (status, _) = s
        .publish_home(s.member_id, requests_home(&["Laptop: waiting"]), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = opened(dm).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["status"], "queued");
    let events = queued(&s).await;
    assert_eq!(events.len(), 1);
    let event = &events[0].0["event"];
    assert_eq!(event["user_id"], json!(s.member_id));
    assert_eq!(event["channel_id"], json!(dm));
    assert_eq!(event["tab"], "home");
    assert_eq!(texts(&event["view"]), ["Laptop: waiting"]);

    let (status, body) = opened(s.ch_id).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a channel is not the conversation with the app: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_button_in_the_home_reaches_the_app_and_opens_a_modal(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    s.configure(json!({ "home_enabled": true })).await;
    let (status, home) = s
        .publish_home(s.member_id, requests_home(&["Laptop: waiting"]), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{home:?}");

    let press = |token: String| {
        let (app, app_id) = (s.app.clone(), s.app_id);
        async move {
            send(
                &app,
                "POST",
                &format!("/api/apps/{app_id}/home/actions"),
                Some(&token),
                Some(json!({ "block_id": "home", "action_id": "new_request" })),
            )
            .await
        }
    };
    let (status, body) = press(s.owner_token.clone()).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the owner has no Home yet: {body:?}"
    );
    assert!(fake.payloads().is_empty());

    let (status, body) = press(s.member_token.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let payload = fake.last();
    assert_eq!(payload["type"], "block_actions");
    assert_eq!(payload["container"]["type"], "view");
    assert_eq!(payload["container"]["view_id"], home["id"]);
    assert_eq!(payload["view"]["type"], "home");
    assert_eq!(payload["actions"][0]["action_id"], "new_request");

    let (status, view) = s
        .as_bot(
            "/api/views/open",
            json!({ "trigger_id": payload["trigger_id"], "view": {
                "type": "modal",
                "title": { "type": "plain_text", "text": "New request" },
                "blocks": [{ "type": "section", "text": { "type": "mrkdwn", "text": "What?" } }],
            } }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view:?}");
}
