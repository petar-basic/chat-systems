use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use super::interactions::{fake_app, setup, Setup};
use crate::messaging::unfurl::consumer::unfurl_event;
use shared_events::Event;

const ISSUE: &str = "https://tracker.example.com/issues/7";
const OTHER_ISSUE: &str = "https://tracker.example.com/issues/8";

fn event(kind: &str, message_id: Uuid) -> Event {
    Event::new(kind, json!({ "id": message_id }))
}

fn issue_blocks(title: &str) -> Value {
    json!([
        { "type": "header", "text": { "type": "plain_text", "text": title } },
        { "type": "section", "fields": [
            { "type": "mrkdwn", "text": "*Status*\nOpen" },
            { "type": "mrkdwn", "text": "*Assignee*\nAna" }
        ] }
    ])
}

impl Setup {
    async fn grant_links(&self) {
        self.configure(
            json!({ "scopes": ["chat:write", "commands", "links:read", "links:write"] }),
        )
        .await;
    }

    async fn claim(&self, app_id: Uuid, domains: Value) -> (StatusCode, Value) {
        send(
            &self.app,
            "PUT",
            &format!("/api/v1/bot-apps/{app_id}/unfurl-domains"),
            Some(&self.owner_token),
            Some(json!({ "domains": domains })),
        )
        .await
    }

    async fn listening_tracker(&self) {
        let fake = fake_app().await;
        self.grant_links().await;
        self.configure(json!({ "request_url": fake.url, "events": ["link_shared"] }))
            .await;
        let (status, body) = self
            .claim(self.app_id, json!(["tracker.example.com"]))
            .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }

    async fn post_in(&self, ch_id: Uuid, content: &str) -> Uuid {
        let (status, body) = send(
            &self.app,
            "POST",
            &format!("/api/v1/channels/{ch_id}/messages"),
            Some(&self.member_token),
            Some(json!({ "content": content })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        let id = body["id"].as_str().expect("id").parse().expect("uuid");
        unfurl_event(&self.state, &event("message.created", id)).await;
        id
    }

    async fn edit(&self, msg_id: Uuid, content: &str) {
        let (status, body) = send(
            &self.app,
            "PATCH",
            &format!("/api/v1/messages/{msg_id}"),
            Some(&self.member_token),
            Some(json!({ "content": content })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        unfurl_event(&self.state, &event("message.updated", msg_id)).await;
    }

    async fn shared_links(&self) -> Vec<Value> {
        sqlx::query_scalar::<_, Value>(
            "SELECT body FROM app_deliveries
              WHERE app_id = $1 AND body->>'type' = 'link_shared' ORDER BY created_at",
        )
        .bind(self.app_id)
        .fetch_all(&self.state.pool)
        .await
        .expect("deliveries")
    }

    async fn answer(&self, msg_id: Uuid, url: &str, blocks: Value) -> (StatusCode, Value) {
        self.as_bot(
            &format!("/api/v1/messages/{msg_id}/unfurls"),
            json!({ "unfurls": { url: { "blocks": blocks } } }),
        )
        .await
    }

    async fn previews(&self, msg_id: Uuid) -> Vec<Value> {
        let (_, metadata) = self.message(msg_id).await.expect("message");
        metadata["unfurls"].as_array().cloned().unwrap_or_default()
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn admins_claim_up_to_five_domains_nobody_else_holds(pool: PgPool) {
    let s = setup(pool).await;
    let (status, body) = s.claim(s.app_id, json!(["tracker.example.com"])).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the links scopes come first: {body:?}"
    );

    s.grant_links().await;
    let six: Vec<String> = (0..6).map(|i| format!("d{i}.example.com")).collect();
    let (status, _) = s.claim(s.app_id, json!(six)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    for bad in [
        "https://tracker.example.com/x",
        "localhost",
        "*.example.com",
        "10.0.0.1",
    ] {
        let (status, body) = s.claim(s.app_id, json!([bad])).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}: {body:?}");
    }
    let (status, _) = s
        .claim(
            s.app_id,
            json!(["tracker.example.com", "Tracker.Example.com."]),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the same domain twice"
    );

    let (status, body) = s
        .claim(s.app_id, json!(["Tracker.Example.com", "wiki.example.com"]))
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body["domains"],
        json!(["tracker.example.com", "wiki.example.com"])
    );

    let (status, other) = send(
        &s.app,
        "POST",
        &format!("/api/v1/workspaces/{}/bot-apps", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "name": "Other", "scopes": ["links:read", "links:write"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{other:?}");
    let other_id: Uuid = other["id"].as_str().expect("id").parse().expect("uuid");
    let (status, body) = s.claim(other_id, json!(["tracker.example.com"])).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    let (status, _) = s.claim(other_id, json!(["github.com"])).await;
    assert_eq!(status, StatusCode::OK, "GitHub previews are off");
    let (status, _) = s.claim(other_id, json!([])).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &s.app,
        "PUT",
        &format!("/api/v1/workspaces/{}/apps/github", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "client_id": "gh-client", "client_secret": "shh", "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    for taken in ["github.com", "www.github.com"] {
        let (status, body) = s.claim(other_id, json!([taken])).await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "{taken} while GitHub is on: {body:?}"
        );
    }

    let (status, body) = send(
        &s.app,
        "GET",
        &format!("/api/v1/bot-apps/{}/unfurl-domains", s.app_id),
        Some(&s.member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "admins only: {body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_claimed_link_reaches_the_app_only_where_its_bot_is(pool: PgPool) {
    let s = setup(pool).await;
    s.listening_tracker().await;

    let msg_id = s.post_in(s.ch_id, &format!("look at {ISSUE}")).await;
    let shared = s.shared_links().await;
    assert_eq!(shared.len(), 1, "{shared:?}");
    assert_eq!(shared[0]["event"]["message_id"], json!(msg_id));
    assert_eq!(shared[0]["event"]["channel_id"], json!(s.ch_id));
    assert_eq!(shared[0]["event"]["user_id"], json!(s.member_id));
    assert_eq!(
        shared[0]["event"]["links"],
        json!([{ "domain": "tracker.example.com", "url": ISSUE }])
    );
    assert!(
        s.previews(msg_id).await.is_empty(),
        "no page preview of its own"
    );

    let quiet = seed_channel(&s.state, s.ws_id, s.member_id, "quiet", false).await;
    let elsewhere = s.post_in(quiet, &format!("same {ISSUE}")).await;
    assert_eq!(
        s.shared_links().await.len(),
        1,
        "no event where the bot is not"
    );
    assert!(
        s.previews(elsewhere).await.is_empty(),
        "and no preview at all"
    );
    let (status, _) = s.answer(elsewhere, ISSUE, issue_blocks("TRK-7")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_app_answers_with_blocks_for_that_message_within_30_minutes(pool: PgPool) {
    let s = setup(pool).await;
    s.listening_tracker().await;
    let msg_id = s.post_in(s.ch_id, &format!("look at {ISSUE}")).await;

    let (status, body) = s
        .as_member(
            &format!("/api/v1/messages/{msg_id}/unfurls"),
            json!({ "unfurls": { ISSUE: { "blocks": issue_blocks("TRK-7") } } }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a person does not answer: {body:?}"
    );
    let (status, _) = s.answer(msg_id, OTHER_ISSUE, issue_blocks("TRK-8")).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a link not in the event"
    );
    let buttons = json!([{ "type": "actions", "elements": [
        { "type": "button", "action_id": "go", "text": { "type": "plain_text", "text": "Go" } }
    ] }]);
    let (status, body) = s.answer(msg_id, ISSUE, buttons).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");

    let (status, body) = s
        .answer(msg_id, ISSUE, issue_blocks("TRK-7 Login fails"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let previews = s.previews(msg_id).await;
    assert_eq!(previews.len(), 1, "{previews:?}");
    assert_eq!(previews[0]["url"], json!(ISSUE));
    assert_eq!(previews[0]["kind"], json!("blocks"));
    assert_eq!(previews[0]["app"]["id"], json!(s.app_id));
    assert_eq!(previews[0]["app"]["name"], json!("Deployer"));
    assert_eq!(
        previews[0]["blocks"][0]["text"]["text"],
        json!("TRK-7 Login fails")
    );

    let (status, _) = s.answer(msg_id, ISSUE, issue_blocks("TRK-7 Fixed")).await;
    assert_eq!(status, StatusCode::OK, "a second answer replaces the first");
    let previews = s.previews(msg_id).await;
    assert_eq!(previews.len(), 1);
    assert_eq!(
        previews[0]["blocks"][0]["text"]["text"],
        json!("TRK-7 Fixed")
    );

    let plain = s.post_in(s.ch_id, "no links here").await;
    let (status, _) = s.answer(plain, ISSUE, issue_blocks("TRK-7")).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a message that did not link to it"
    );

    sqlx::query("UPDATE app_link_shares SET expires_at = NOW() - INTERVAL '1 second'")
        .execute(&s.state.pool)
        .await
        .expect("age the share");
    let (status, _) = s.answer(msg_id, ISSUE, issue_blocks("TRK-7 Late")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "after 30 minutes");

    let fresh = s.post_in(s.ch_id, &format!("again {OTHER_ISSUE}")).await;
    s.configure(json!({ "scopes": ["chat:write", "links:read"] }))
        .await;
    let (status, body) = s.answer(fresh, OTHER_ISSUE, issue_blocks("TRK-8")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "needs links:write: {body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_edit_keeps_the_preview_and_asks_only_about_new_links(pool: PgPool) {
    let s = setup(pool).await;
    s.listening_tracker().await;
    let msg_id = s.post_in(s.ch_id, &format!("look at {ISSUE}")).await;
    let (status, _) = s.answer(msg_id, ISSUE, issue_blocks("TRK-7")).await;
    assert_eq!(status, StatusCode::OK);

    s.edit(msg_id, &format!("look at {ISSUE} and {OTHER_ISSUE}"))
        .await;
    let shared = s.shared_links().await;
    assert_eq!(shared.len(), 2, "{shared:?}");
    assert_eq!(
        shared[1]["event"]["links"],
        json!([{ "domain": "tracker.example.com", "url": OTHER_ISSUE }])
    );
    let previews = s.previews(msg_id).await;
    assert_eq!(previews.len(), 1, "the first preview stays: {previews:?}");
    assert_eq!(previews[0]["url"], json!(ISSUE));
    let (status, _) = s.answer(msg_id, OTHER_ISSUE, issue_blocks("TRK-8")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s.previews(msg_id).await.len(), 2);
    let (status, _) = s.answer(msg_id, ISSUE, issue_blocks("TRK-7 Fixed")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the first link still takes an answer"
    );
    let order: Vec<Value> = s
        .previews(msg_id)
        .await
        .iter()
        .map(|preview| preview["url"].clone())
        .collect();
    assert_eq!(
        order,
        vec![json!(ISSUE), json!(OTHER_ISSUE)],
        "previews follow the links"
    );

    s.edit(msg_id, &format!("only {OTHER_ISSUE} now")).await;
    let previews = s.previews(msg_id).await;
    assert_eq!(
        previews.len(),
        1,
        "a link taken out takes its preview: {previews:?}"
    );
    assert_eq!(previews[0]["url"], json!(OTHER_ISSUE));

    let both = s
        .post_in(s.ch_id, &format!("two {ISSUE} {OTHER_ISSUE}"))
        .await;
    let (status, body) = send(
        &s.app,
        "PATCH",
        &format!("/api/v1/messages/{both}"),
        Some(&s.member_token),
        Some(json!({ "content": format!("one {OTHER_ISSUE}") })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, _) = s.answer(both, ISSUE, issue_blocks("TRK-7")).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a link edited out before the answer"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_removed_preview_stays_removed_and_the_app_is_not_asked_again(pool: PgPool) {
    let s = setup(pool).await;
    s.listening_tracker().await;
    let msg_id = s.post_in(s.ch_id, &format!("look at {ISSUE}")).await;
    let (status, _) = s.answer(msg_id, ISSUE, issue_blocks("TRK-7")).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &s.app,
        "DELETE",
        &format!("/api/v1/messages/{msg_id}/unfurls"),
        Some(&s.member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    s.edit(msg_id, &format!("look at {ISSUE} and {OTHER_ISSUE}"))
        .await;
    assert_eq!(s.shared_links().await.len(), 1, "not asked again");
    assert!(s.previews(msg_id).await.is_empty());
    let (status, _) = s.answer(msg_id, ISSUE, issue_blocks("TRK-7")).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn workspace_link_previews_off_turns_app_previews_off(pool: PgPool) {
    let s = setup(pool).await;
    s.listening_tracker().await;
    let asked = s.post_in(s.ch_id, &format!("look at {ISSUE}")).await;

    let (status, body) = send(
        &s.app,
        "PATCH",
        &format!("/api/v1/workspaces/{}", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "link_previews": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    s.post_in(s.ch_id, &format!("again {OTHER_ISSUE}")).await;
    assert_eq!(
        s.shared_links().await.len(),
        1,
        "no event while previews are off"
    );
    let (status, _) = s.answer(asked, ISSUE, issue_blocks("TRK-7")).await;
    assert_eq!(status, StatusCode::CONFLICT);
}
