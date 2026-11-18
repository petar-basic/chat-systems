use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::http::{HeaderMap, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::hooks::executor::{sign_body, SIGNATURE_HEADER};
use crate::state::AppState;

type Seen = Arc<Mutex<Vec<(HeaderMap, String)>>>;
type Reply = Arc<Mutex<(u16, Value)>>;

pub(super) struct FakeApp {
    pub(super) url: String,
    seen: Seen,
    reply: Reply,
}

impl FakeApp {
    pub(super) fn payloads(&self) -> Vec<Value> {
        self.seen
            .lock()
            .expect("seen")
            .iter()
            .filter_map(|(_, body)| serde_json::from_str::<Value>(body).ok())
            .filter(|body| body["type"] != "url_verification")
            .collect()
    }

    pub(super) fn last(&self) -> Value {
        self.payloads().pop().expect("the app was called")
    }

    pub(super) fn answer(&self, status: u16, body: Value) {
        *self.reply.lock().expect("reply") = (status, body);
    }
}

pub(super) async fn fake_app() -> FakeApp {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let reply: Reply = Arc::new(Mutex::new((200, Value::Null)));
    let (recorded, replying) = (seen.clone(), reply.clone());
    let app = axum::Router::new().fallback(move |headers: HeaderMap, body: String| {
        let recorded = recorded.clone();
        let replying = replying.clone();
        async move {
            recorded.lock().expect("seen").push((headers, body.clone()));
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            if parsed["type"] == "url_verification" {
                return (
                    StatusCode::OK,
                    json!({ "challenge": parsed["challenge"] }).to_string(),
                );
            }
            let (status, answer) = replying.lock().expect("reply").clone();
            let text = if answer.is_null() {
                String::new()
            } else {
                answer.to_string()
            };
            (StatusCode::from_u16(status).expect("status"), text)
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the fake app");
    let url = format!("http://{}/slack", listener.local_addr().expect("addr"));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    FakeApp { url, seen, reply }
}

pub(super) struct Setup {
    pub(super) app: axum::Router,
    pub(super) state: Arc<AppState>,
    pub(super) owner_token: String,
    pub(super) ws_id: Uuid,
    pub(super) ch_id: Uuid,
    pub(super) app_id: Uuid,
    pub(super) bot_user_id: Uuid,
    pub(super) bot_token: String,
    pub(super) member_id: Uuid,
    pub(super) member_token: String,
}

pub(super) async fn setup(pool: PgPool) -> Setup {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "act-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Interactive").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "deploys", false).await;
    let (status, made) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/bot-apps"),
        Some(&owner_token),
        Some(json!({ "name": "Deployer", "scopes": ["chat:write", "commands"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{made:?}");
    let bot_user_id: Uuid = made["bot_user_id"]
        .as_str()
        .expect("bot")
        .parse()
        .expect("uuid");
    let (member_id, _, member_token) = seed_and_login(&app, &state, "act-ana", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let s = Setup {
        app_id: made["id"].as_str().expect("id").parse().expect("uuid"),
        bot_token: made["token"].as_str().expect("token").to_string(),
        bot_user_id,
        app,
        state,
        owner_token,
        ws_id,
        ch_id,
        member_id,
        member_token,
    };
    s.add(bot_user_id).await;
    s.add(member_id).await;
    s
}

fn deploy_blocks() -> Value {
    json!([
        { "type": "section", "text": { "type": "mrkdwn", "text": "*Deploy v2?*" } },
        { "type": "actions", "block_id": "deploy", "elements": [
            { "type": "button", "action_id": "approve", "value": "v2", "style": "primary",
              "text": { "type": "plain_text", "text": "Approve" } },
            { "type": "button", "action_id": "reject", "value": "v2", "style": "danger",
              "text": { "type": "plain_text", "text": "Reject" } },
            { "type": "static_select", "action_id": "region",
              "options": [{ "text": { "type": "plain_text", "text": "Europe" }, "value": "eu" }] }
        ] }
    ])
}

fn request_view(title: &str) -> Value {
    json!({
        "type": "modal",
        "callback_id": "request",
        "external_id": "request-1",
        "title": { "type": "plain_text", "text": title },
        "submit": { "type": "plain_text", "text": "Send" },
        "close": { "type": "plain_text", "text": "Cancel" },
        "blocks": [
            { "type": "input", "block_id": "what",
              "label": { "type": "plain_text", "text": "What do you need?" },
              "element": { "type": "plain_text_input", "action_id": "text", "max_length": 40 } },
            { "type": "input", "block_id": "when", "optional": true,
              "label": { "type": "plain_text", "text": "By when" },
              "element": { "type": "datepicker", "action_id": "date" } },
            { "type": "actions", "block_id": "more", "elements": [
                { "type": "button", "action_id": "details",
                  "text": { "type": "plain_text", "text": "Add details" } }
            ] }
        ]
    })
}

impl Setup {
    pub(super) async fn add(&self, user_id: Uuid) {
        let (status, body) = send(
            &self.app,
            "POST",
            &format!("/api/channels/{}/members", self.ch_id),
            Some(&self.owner_token),
            Some(json!({ "user_id": user_id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }

    pub(super) async fn configure(&self, settings: Value) {
        let (status, body) = send(
            &self.app,
            "PATCH",
            &format!("/api/bot-apps/{}", self.app_id),
            Some(&self.owner_token),
            Some(settings),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }

    pub(super) async fn connect(&self, to: &FakeApp) {
        self.configure(json!({ "request_url": to.url, "interactivity": true }))
            .await;
    }

    pub(super) async fn post_deploy(&self) -> Uuid {
        let (status, body) = send(
            &self.app,
            "POST",
            &format!("/api/channels/{}/messages", self.ch_id),
            Some(&self.bot_token),
            Some(json!({ "blocks": deploy_blocks() })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body["id"].as_str().expect("id").parse().expect("uuid")
    }

    async fn press(
        &self,
        msg_id: Uuid,
        action_id: &str,
        value: Option<&str>,
    ) -> (StatusCode, Value) {
        send(
            &self.app,
            "POST",
            &format!("/api/messages/{msg_id}/actions"),
            Some(&self.member_token),
            Some(json!({ "block_id": "deploy", "action_id": action_id, "value": value })),
        )
        .await
    }

    pub(super) async fn as_bot(&self, path: &str, body: Value) -> (StatusCode, Value) {
        send(&self.app, "POST", path, Some(&self.bot_token), Some(body)).await
    }

    pub(super) async fn as_member(&self, path: &str, body: Value) -> (StatusCode, Value) {
        send(
            &self.app,
            "POST",
            path,
            Some(&self.member_token),
            Some(body),
        )
        .await
    }

    pub(super) async fn respond(&self, response_url: &str, body: Value) -> (StatusCode, Value) {
        let path = response_url
            .strip_prefix("http://localhost")
            .expect("a response URL on this instance");
        send(&self.app, "POST", path, None, Some(body)).await
    }

    pub(super) async fn message(&self, msg_id: Uuid) -> Option<(String, Value)> {
        sqlx::query_as::<_, (String, Value)>(
            "SELECT content, metadata FROM messages WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(msg_id)
        .fetch_optional(&self.state.pool)
        .await
        .expect("message")
    }

    async fn open_view(&self, fake: &FakeApp, title: &str) -> Value {
        let msg_id = self.post_deploy().await;
        let (status, body) = self.press(msg_id, "approve", None).await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        let trigger_id = fake.last()["trigger_id"].clone();
        let (status, view) = self
            .as_bot(
                "/api/views/open",
                json!({ "trigger_id": trigger_id, "view": request_view(title) }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{view:?}");
        view
    }

    async fn stack(&self, view_id: &Value) -> Vec<(String, Value)> {
        sqlx::query_as::<_, (String, Value)>(
            "SELECT hash, view FROM app_views
              WHERE stack_id = (SELECT stack_id FROM app_views WHERE id = $1::uuid)
              ORDER BY position",
        )
        .bind(view_id.as_str().expect("view id"))
        .fetch_all(&self.state.pool)
        .await
        .expect("stack")
    }

    async fn submit(&self, view_id: &Value, values: Value) -> (StatusCode, Value) {
        self.as_member(
            &format!("/api/views/{}/submit", view_id.as_str().expect("id")),
            json!({ "values": values }),
        )
        .await
    }

    async fn signing_secret(&self) -> String {
        let (status, body) = send(
            &self.app,
            "GET",
            &format!("/api/bot-apps/{}/signing-secret", self.app_id),
            Some(&self.owner_token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body["signing_secret"].as_str().expect("secret").to_string()
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn approving_a_deploy_reaches_the_app_which_replaces_the_message(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let msg_id = s.post_deploy().await;

    let (status, body) = s.press(msg_id, "approve", None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (headers, raw) = fake
        .seen
        .lock()
        .expect("seen")
        .iter()
        .rev()
        .find(|(_, body)| body.contains("block_actions"))
        .cloned()
        .expect("a payload");
    assert_eq!(
        headers[SIGNATURE_HEADER].to_str().expect("header"),
        sign_body(&s.signing_secret().await, raw.as_bytes()),
        "signed the way events are"
    );
    let payload: Value = serde_json::from_str(&raw).expect("json");
    assert_eq!(payload["type"], "block_actions");
    assert_eq!(payload["user"]["id"], json!(s.member_id));
    assert_eq!(payload["channel"]["id"], json!(s.ch_id));
    assert_eq!(payload["container"]["message_id"], json!(msg_id));
    assert_eq!(payload["message"]["blocks"][1]["block_id"], "deploy");
    assert_eq!(payload["actions"][0]["action_id"], "approve");
    assert_eq!(payload["actions"][0]["block_id"], "deploy");
    assert_eq!(payload["actions"][0]["value"], "v2");
    assert!(payload["trigger_id"].is_string());

    let (status, body) = s
        .respond(
            payload["response_url"].as_str().expect("response_url"),
            json!({ "replace_original": true, "text": "Approved by Ana" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (content, metadata) = s.message(msg_id).await.expect("still there");
    assert_eq!(content, "Approved by Ana");
    assert!(metadata.get("blocks").is_none(), "{metadata:?}");

    let recorded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'bot_app.interaction' AND resource_id = $1",
    )
    .bind(s.app_id)
    .fetch_one(&s.state.pool)
    .await
    .expect("audit");
    assert_eq!(recorded, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_choice_is_sent_only_when_it_is_one_of_the_options(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let msg_id = s.post_deploy().await;

    let (status, body) = s.press(msg_id, "region", Some("mars")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    let (status, body) = s.press(msg_id, "missing", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body:?}");
    assert!(fake.payloads().is_empty());

    let (status, body) = s.press(msg_id, "region", Some("eu")).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let action = fake.last()["actions"][0].clone();
    assert_eq!(action["type"], "static_select");
    assert_eq!(action["selected_option"]["value"], "eu");
    assert_eq!(action["selected_option"]["text"]["text"], "Europe");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_response_url_answers_five_times_within_thirty_minutes(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let msg_id = s.post_deploy().await;
    s.press(msg_id, "approve", None).await;
    let response_url = fake.last()["response_url"]
        .as_str()
        .expect("url")
        .to_string();

    for _ in 0..5 {
        let (status, body) = s
            .respond(&response_url, json!({ "text": "Only you see this" }))
            .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        assert_eq!(body["status"], "shown");
    }
    let (status, body) = s.respond(&response_url, json!({ "text": "Sixth" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.to_string().contains("used up"), "{body:?}");

    s.press(msg_id, "reject", None).await;
    let second = fake.last()["response_url"]
        .as_str()
        .expect("url")
        .to_string();
    let (status, body) = s
        .respond(
            &second,
            json!({ "response_type": "in_channel", "text": "Rejected; nothing ships" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let posted: (Uuid, String) = sqlx::query_as(
        "SELECT user_id, content FROM messages WHERE channel_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(s.ch_id)
    .fetch_one(&s.state.pool)
    .await
    .expect("posted");
    assert_eq!(posted, (s.bot_user_id, "Rejected; nothing ships".into()));

    sqlx::query("UPDATE app_interactions SET expires_at = NOW() - INTERVAL '1 second'")
        .execute(&s.state.pool)
        .await
        .expect("expire");
    let (status, body) = s.respond(&second, json!({ "text": "Late" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.to_string().contains("expired"), "{body:?}");

    let forged = format!("{}x", &second[..second.len() - 1]);
    let (status, body) = s.respond(&forged, json!({ "text": "Forged" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.to_string().contains("Unknown"), "{body:?}");

    s.press(msg_id, "approve", None).await;
    let third = fake.last()["response_url"]
        .as_str()
        .expect("url")
        .to_string();
    let (status, body) = s.respond(&third, json!({ "delete_original": true })).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(s.message(msg_id).await.is_none(), "deleted");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn elements_reach_nobody_while_interactivity_is_off(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.configure(json!({ "request_url": fake.url })).await;
    let msg_id = s.post_deploy().await;

    let listed = |s: &Setup| {
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
            body["data"].as_array().expect("data").clone()
        }
    };
    let apps = listed(&s).await;
    assert_eq!(apps.len(), 1, "the app is on");
    assert_eq!(apps[0]["interactive"], false);

    let (status, body) = s.press(msg_id, "approve", None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    assert!(body.to_string().contains("not set up interactivity"));
    assert!(fake.payloads().is_empty(), "nothing reached the app");

    s.configure(json!({ "interactivity": true })).await;
    let apps = listed(&s).await;
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0]["interactive"], true);
    assert_eq!(apps[0]["bot_user_id"], json!(s.bot_user_id));

    let (status, person) = send(
        &s.app,
        "POST",
        &format!("/api/channels/{}/messages", s.ch_id),
        Some(&s.member_token),
        Some(json!({ "content": "not from an app" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{person:?}");
    let person_msg: Uuid = person["id"].as_str().expect("id").parse().expect("uuid");
    let (status, _) = s.press(person_msg, "approve", None).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a person's message has no app"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_app_that_does_not_answer_changes_nothing(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let msg_id = s.post_deploy().await;
    let before = s.message(msg_id).await;

    fake.answer(500, Value::Null);
    let (status, body) = s.press(msg_id, "approve", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "The app did not respond");
    assert_eq!(s.message(msg_id).await, before);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_trigger_opens_one_view_and_then_expires(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let msg_id = s.post_deploy().await;
    s.press(msg_id, "approve", None).await;
    let trigger_id = fake.last()["trigger_id"].clone();

    let (status, body) = send(
        &s.app,
        "POST",
        "/api/views/open",
        Some(&s.member_token),
        Some(json!({ "trigger_id": trigger_id, "view": request_view("Request") })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");

    let (status, body) = s
        .as_bot(
            "/api/views/open",
            json!({ "trigger_id": trigger_id, "view": { "type": "modal", "blocks": [] } }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    assert!(
        body["error"].as_str().expect("error").contains("title"),
        "{body:?}"
    );

    let (status, view) = s
        .as_bot(
            "/api/views/open",
            json!({ "trigger_id": trigger_id, "view": request_view("Request") }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view:?}");
    assert_eq!(view["root_view_id"], view["id"]);
    assert_eq!(view["title"]["text"], "Request");
    assert_eq!(view["blocks"][0]["block_id"], "what");

    let (status, body) = s
        .as_bot(
            "/api/views/open",
            json!({ "trigger_id": trigger_id, "view": request_view("Again") }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "used: {body:?}");

    s.press(msg_id, "reject", None).await;
    let stale = fake.last()["trigger_id"].clone();
    sqlx::query("UPDATE app_triggers SET expires_at = NOW() - INTERVAL '1 second'")
        .execute(&s.state.pool)
        .await
        .expect("expire");
    let (status, body) = s
        .as_bot(
            "/api/views/open",
            json!({ "trigger_id": stale, "view": request_view("Late") }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "expired: {body:?}"
    );
    assert!(body.to_string().contains("expired"), "{body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_command_opens_a_modal_that_shows_the_apps_errors_then_closes(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let (status, body) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/hooks", s.ws_id),
        Some(&s.owner_token),
        Some(json!({
            "hook_type": "slash_command",
            "name": "request",
            "app_id": s.app_id,
            "config": {
                "command": "request",
                "url": fake.url,
                "channel_ids": [s.ch_id.to_string()],
            },
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (status, body) = s
        .as_member(
            &format!("/api/channels/{}/commands", s.ch_id),
            json!({ "command": "request" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let command = fake.last();
    assert_eq!(command["command"], "request");
    let (status, view) = s
        .as_bot(
            "/api/views/open",
            json!({ "trigger_id": command["trigger_id"], "view": request_view("Request") }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view:?}");
    let calls = fake.payloads().len();

    let (status, outcome) = s.submit(&view["id"], json!({})).await;
    assert_eq!(status, StatusCode::OK, "{outcome:?}");
    assert_eq!(outcome["outcome"], "errors");
    assert_eq!(outcome["errors"]["what"], "This field is required");
    assert!(outcome["errors"].get("when").is_none(), "optional");
    assert_eq!(
        fake.payloads().len(),
        calls,
        "refused before asking the app"
    );

    let (_, outcome) = s
        .submit(&view["id"], json!({ "what": { "text": ["x".repeat(41)] } }))
        .await;
    assert_eq!(outcome["errors"]["what"], "At most 40 characters");

    fake.answer(
        200,
        json!({ "response_action": "errors", "errors": { "what": "Say which laptop" } }),
    );
    let values = json!({ "what": { "text": ["A laptop"] }, "when": { "date": ["2026-12-01"] } });
    let (status, outcome) = s.submit(&view["id"], values.clone()).await;
    assert_eq!(status, StatusCode::OK, "{outcome:?}");
    assert_eq!(outcome["outcome"], "errors");
    assert_eq!(outcome["errors"]["what"], "Say which laptop");
    let submitted = fake.last();
    assert_eq!(submitted["type"], "view_submission");
    assert_eq!(submitted["view"]["callback_id"], "request");
    assert_eq!(
        submitted["view"]["state"]["values"]["what"]["text"]["value"],
        "A laptop"
    );
    assert_eq!(
        submitted["view"]["state"]["values"]["when"]["date"]["selected_date"],
        "2026-12-01"
    );

    fake.answer(200, Value::Null);
    let (status, outcome) = s.submit(&view["id"], values).await;
    assert_eq!(status, StatusCode::OK, "{outcome:?}");
    assert_eq!(outcome["outcome"], "closed");
    assert!(s.stack(&view["id"]).await.is_empty(), "the modal closed");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_answer_can_update_push_and_clear_the_stack(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let root = s.open_view(&fake, "Request").await;
    let values = json!({ "what": { "text": ["A laptop"] } });

    fake.answer(
        200,
        json!({ "response_action": "update", "view": request_view("Updated") }),
    );
    let (_, outcome) = s.submit(&root["id"], values.clone()).await;
    assert_eq!(outcome["outcome"], "updated");
    let stack = s.stack(&root["id"]).await;
    assert_eq!(stack.len(), 1);
    assert_eq!(stack[0].1["title"]["text"], "Updated");
    assert_ne!(json!(stack[0].0), root["hash"], "a new hash");

    fake.answer(
        200,
        json!({ "response_action": "push", "view": request_view("Second") }),
    );
    let (_, outcome) = s.submit(&root["id"], values.clone()).await;
    assert_eq!(outcome["outcome"], "pushed");
    let (status, body) = s.submit(&root["id"], values.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT, "only the top: {body:?}");

    let second: Uuid = sqlx::query_scalar(
        "SELECT id FROM app_views WHERE stack_id = (SELECT stack_id FROM app_views WHERE id = $1) AND position = 1",
    )
    .bind(root["id"].as_str().expect("id").parse::<Uuid>().expect("uuid"))
    .fetch_one(&s.state.pool)
    .await
    .expect("second");
    let second = json!(second);
    let (_, outcome) = s.submit(&second, values.clone()).await;
    assert_eq!(outcome["outcome"], "pushed");
    assert_eq!(s.stack(&root["id"]).await.len(), 3);
    let top = json!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM app_views WHERE stack_id = (SELECT stack_id FROM app_views WHERE id = $1::uuid) ORDER BY position DESC LIMIT 1",
        )
        .bind(root["id"].as_str().expect("id"))
        .fetch_one(&s.state.pool)
        .await
        .expect("top")
    );
    let (status, body) = s.submit(&top, values.clone()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a fourth: {body:?}");
    assert!(body.to_string().contains("at most 3"), "{body:?}");

    let (status, body) = s
        .as_member(
            &format!("/api/views/{}/close", top.as_str().expect("id")),
            json!({ "all": false }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(s.stack(&root["id"]).await.len(), 2, "back one");

    fake.answer(200, json!({ "response_action": "clear" }));
    let (_, outcome) = s.submit(&second, values).await;
    assert_eq!(outcome["outcome"], "cleared");
    assert!(s.stack(&root["id"]).await.is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_push_needs_a_trigger_from_the_view_and_an_update_its_hash(pool: PgPool) {
    let s = setup(pool).await;
    let fake = fake_app().await;
    s.connect(&fake).await;
    let msg_id = s.post_deploy().await;
    s.press(msg_id, "approve", None).await;
    let from_message = fake.last()["trigger_id"].clone();
    let (status, body) = s
        .as_bot(
            "/api/views/push",
            json!({ "trigger_id": from_message, "view": request_view("Pushed") }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");

    let root = s.open_view(&fake, "Request").await;
    let (status, body) = s
        .as_member(
            &format!("/api/views/{}/actions", root["id"].as_str().expect("id")),
            json!({
                "block_id": "more",
                "action_id": "details",
                "values": { "what": { "text": ["A laptop"] } },
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let pressed = fake.last();
    assert_eq!(pressed["container"]["type"], "view");
    assert_eq!(
        pressed["view"]["state"]["values"]["what"]["text"]["value"], "A laptop",
        "the app sees what is typed so far"
    );
    let (status, pushed) = s
        .as_bot(
            "/api/views/push",
            json!({ "trigger_id": pressed["trigger_id"], "view": request_view("Details") }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{pushed:?}");
    assert_eq!(pushed["previous_view_id"], root["id"]);
    assert_eq!(pushed["root_view_id"], root["id"]);

    let (status, body) = s
        .as_bot(
            "/api/views/update",
            json!({ "view_id": root["id"], "hash": "stale", "view": request_view("Changed") }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    let (status, updated) = s
        .as_bot(
            "/api/views/update",
            json!({ "view_id": root["id"], "hash": root["hash"], "view": request_view("Changed") }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated:?}");
    assert_eq!(updated["title"]["text"], "Changed");
    let (status, updated) = s
        .as_bot(
            "/api/views/update",
            json!({ "external_id": "request-1", "view": request_view("By external id") }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated:?}");
}

async fn answer_from_the_socket(state: Arc<AppState>, app_id: Uuid, answers: Vec<Value>) {
    for answer in answers {
        loop {
            let waiting: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM app_deliveries
                  WHERE app_id = $1 AND kind = 'interactive' AND delivered_at IS NULL
                  ORDER BY seq LIMIT 1",
            )
            .bind(app_id)
            .fetch_optional(&state.pool)
            .await
            .expect("deliveries");
            if let Some(id) = waiting {
                sqlx::query(
                    "UPDATE app_deliveries SET delivered_at = NOW(), response = $2 WHERE id = $1",
                )
                .bind(id)
                .bind(if answer.is_null() {
                    None
                } else {
                    Some(&answer)
                })
                .execute(&state.pool)
                .await
                .expect("ack");
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_socket_mode_app_answers_in_its_acknowledgement(pool: PgPool) {
    let s = setup(pool).await;
    s.configure(json!({ "socket_mode": true, "interactivity": true }))
        .await;
    let msg_id = s.post_deploy().await;

    let (status, body) = s.press(msg_id, "approve", None).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "no socket: {body:?}"
    );

    sqlx::query(
        "INSERT INTO app_sockets (app_id, connected_at, seen_at) VALUES ($1, NOW(), NOW())",
    )
    .bind(s.app_id)
    .execute(&s.state.pool)
    .await
    .expect("socket");
    let acks = tokio::spawn(answer_from_the_socket(
        s.state.clone(),
        s.app_id,
        vec![
            Value::Null,
            json!({ "response_action": "errors", "errors": { "what": "Which one?" } }),
        ],
    ));

    let (status, body) = s.press(msg_id, "approve", None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let trigger_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM app_triggers WHERE app_id = $1 ORDER BY expires_at DESC LIMIT 1",
    )
    .bind(s.app_id)
    .fetch_one(&s.state.pool)
    .await
    .expect("trigger");
    let (status, view) = s
        .as_bot(
            "/api/views/open",
            json!({ "trigger_id": trigger_id, "view": request_view("Request") }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view:?}");

    let (status, outcome) = s
        .submit(&view["id"], json!({ "what": { "text": ["A laptop"] } }))
        .await;
    assert_eq!(status, StatusCode::OK, "{outcome:?}");
    assert_eq!(outcome["outcome"], "errors");
    assert_eq!(outcome["errors"]["what"], "Which one?");
    acks.await.expect("acks");

    let left: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM app_deliveries WHERE kind = 'interactive'")
            .fetch_one(&s.state.pool)
            .await
            .expect("left");
    assert_eq!(left, 0, "an answered interaction is not kept");
}
