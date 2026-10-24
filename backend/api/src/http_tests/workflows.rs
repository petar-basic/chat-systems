use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;
use crate::workflows::runner;

fn post_to(channel: Uuid, text: &str) -> Value {
    json!({ "type": "send_message", "channel_id": channel, "text": text })
}

fn dm_to(user: Uuid, text: &str) -> Value {
    json!({ "type": "send_dm", "to": { "kind": "person", "user_id": user }, "text": text })
}

fn scheduled(name: &str, steps: Vec<Value>) -> Value {
    json!({
        "name": name,
        "definition": {
            "trigger": { "type": "schedule", "days": ["mon", "tue", "wed", "thu", "fri"], "time": "09:50", "timezone": "Europe/Belgrade" },
            "steps": steps,
        },
    })
}

fn webhook(name: &str, variables: &[&str], steps: Vec<Value>) -> Value {
    json!({
        "name": name,
        "definition": { "trigger": { "type": "webhook", "variables": variables }, "steps": steps },
    })
}

async fn create(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/workflows"),
        Some(token),
        Some(body),
    )
    .await
}

async fn built(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> Value {
    let (status, workflow) = create(app, token, ws, body).await;
    assert_eq!(status, StatusCode::OK, "{workflow:?}");
    workflow
}

async fn act(
    app: &axum::Router,
    token: &str,
    workflow: &Value,
    action: &str,
) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!(
            "/api/workflows/{}/{action}",
            workflow["id"].as_str().unwrap()
        ),
        Some(token),
        None,
    )
    .await
}

async fn published(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> Value {
    let workflow = built(app, token, ws, body).await;
    let (status, workflow) = act(app, token, &workflow, "publish").await;
    assert_eq!(status, StatusCode::OK, "{workflow:?}");
    workflow
}

async fn list(app: &axum::Router, token: &str, ws: Uuid) -> Vec<String> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/workspaces/{ws}/workflows"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap().to_string())
        .collect()
}

fn hook_path(workflow: &Value) -> String {
    let url = workflow["webhook_url"].as_str().expect("a webhook url");
    url[url.find("/api/").unwrap()..].to_string()
}

async fn call(
    app: &axum::Router,
    path: &str,
    key: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(key) = key {
        builder = builder.header("idempotency-key", key);
    }
    let request = builder
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn runs(app: &axum::Router, token: &str, workflow: &Value) -> Vec<Value> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/workflows/{}/runs", workflow["id"].as_str().unwrap()),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"].as_array().unwrap().clone()
}

async fn bot_messages(state: &AppState, channel: Uuid) -> Vec<(String, Value)> {
    sqlx::query_as::<_, (String, Value)>(
        "SELECT content, metadata FROM messages WHERE channel_id = $1 AND metadata ? 'bot' ORDER BY created_at",
    )
    .bind(channel)
    .fetch_all(&state.pool)
    .await
    .unwrap()
}

async fn workspace_with_member(
    app: &axum::Router,
    state: &AppState,
    prefix: &str,
) -> (Uuid, Uuid, String, Uuid, String) {
    let (owner_id, _, owner) = seed_and_login(app, state, &format!("{prefix}-owner"), false).await;
    let ws = seed_workspace(state, owner_id, "Workflow WS").await;
    let (member_id, _, member) =
        seed_and_login(app, state, &format!("{prefix}-member"), false).await;
    add_ws_member(state, ws, member_id, "member").await;
    (ws, owner_id, owner, member_id, member)
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_member_builds_publishes_edits_disables_and_deletes_a_workflow(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, owner, member_id, member) =
        workspace_with_member(&app, &state, "wf-life").await;
    let general = seed_channel(&state, ws, owner_id, "standup", false).await;

    let workflow = built(
        &app,
        &member,
        ws,
        scheduled("Standup", vec![post_to(general, "standup in 10")]),
    )
    .await;
    assert_eq!(workflow["enabled"], false, "a new workflow is a draft");
    assert_eq!(workflow["created_by"], json!(member_id));
    assert_eq!(workflow["version"], 1);
    assert!(workflow["next_run_at"].is_null());
    assert!(workflow["webhook_url"].is_null());
    assert_eq!(
        workflow["definition"]["trigger"]["days"]
            .as_array()
            .unwrap()
            .len(),
        5
    );

    let (status, workflow) = act(&app, &member, &workflow, "publish").await;
    assert_eq!(status, StatusCode::OK, "{workflow:?}");
    assert_eq!(workflow["enabled"], true);
    let next: chrono::DateTime<chrono::Utc> =
        serde_json::from_value(workflow["next_run_at"].clone()).expect("a next run");
    assert!(next > chrono::Utc::now());

    let (status, edited) = send(
        &app,
        "PUT",
        &format!("/api/workflows/{}", workflow["id"].as_str().unwrap()),
        Some(&member),
        Some(scheduled(
            " Standup reminder ",
            vec![post_to(general, "standup in 5")],
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited:?}");
    assert_eq!(edited["name"], "Standup reminder");
    assert_eq!(edited["version"], 2);
    assert_eq!(edited["enabled"], true, "editing keeps it running");
    assert!(!edited["next_run_at"].is_null());

    let (third_id, _, third) = seed_and_login(&app, &state, "wf-life-third", false).await;
    add_ws_member(&state, ws, third_id, "member").await;
    assert_eq!(list(&app, &member, ws).await, vec!["Standup reminder"]);
    assert!(
        list(&app, &third, ws).await.is_empty(),
        "a member sees only their own"
    );
    assert_eq!(
        list(&app, &owner, ws).await,
        vec!["Standup reminder"],
        "an admin sees every one"
    );
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workflows/{}", workflow["id"].as_str().unwrap()),
        Some(&third),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, disabled) = act(&app, &member, &workflow, "disable").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(disabled["enabled"], false);
    assert!(disabled["next_run_at"].is_null());

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workflows/{}", workflow["id"].as_str().unwrap()),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(list(&app, &member, ws).await.is_empty());

    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE workspace_id = $1 AND resource_type = 'workflow' ORDER BY created_at, id",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(
        actions,
        vec![
            "workflow.created",
            "workflow.enabled",
            "workflow.updated",
            "workflow.disabled",
            "workflow.deleted"
        ]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_definition_the_creator_could_not_carry_out_is_refused_at_save(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-refuse").await;
    let open = seed_channel(&state, ws, owner_id, "open", false).await;
    let secret = seed_channel(&state, ws, owner_id, "secret", true).await;
    let (outsider_id, _) = seed(&state, "wf-refuse-outsider", false).await;

    let mut bad_zone = scheduled("Bad zone", vec![post_to(open, "hi")]);
    bad_zone["definition"]["trigger"]["timezone"] = json!("Mars/Olympus");
    let cases = [
        (bad_zone, "\"Mars/Olympus\" isn't a timezone"),
        (
            scheduled("Secret", vec![post_to(secret, "hi")]),
            "Step 1: you can't post in that channel",
        ),
        (
            scheduled("Self", vec![post_to(open, "ok"), dm_to(member_id, "me")]),
            "Step 2: a workflow can't send you a DM; pick someone else",
        ),
        (
            scheduled("Outsider", vec![dm_to(outsider_id, "hi")]),
            "Step 1: you can't message that person",
        ),
        (
            scheduled("Vars", vec![post_to(open, "{{webhook.x}}")]),
            "Step 1: {{webhook.x}} isn't available for this trigger",
        ),
        (
            scheduled("  ", vec![post_to(open, "hi")]),
            "Give the workflow a name",
        ),
    ];
    for (body, expected) in cases {
        let (status, error) = create(&app, &member, ws, body).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{expected}: {error:?}"
        );
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains(expected),
            "{expected}: {error:?}"
        );
    }
    assert!(list(&app, &member, ws).await.is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn admins_can_keep_building_to_themselves(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, owner, _, member) = workspace_with_member(&app, &state, "wf-builders").await;
    let open = seed_channel(&state, ws, owner_id, "open", false).await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "wf-builders-guest", false).await;
    add_ws_member(&state, ws, guest_id, "guest").await;

    let (status, _) = create(
        &app,
        &guest,
        ws,
        scheduled("Guest", vec![post_to(open, "hi")]),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "guests never build");

    let mine = built(
        &app,
        &member,
        ws,
        scheduled("Mine", vec![post_to(open, "hi")]),
    )
    .await;

    let (status, settings) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}"),
        Some(&owner),
        Some(json!({ "workflow_builders": "admins" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(settings["settings"]["workflow_builders"], "admins");

    let (status, error) = create(
        &app,
        &member,
        ws,
        scheduled("Later", vec![post_to(open, "hi")]),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(error
        .to_string()
        .contains("Only admins can build workflows"));
    let (status, _) = act(&app, &member, &mine, "publish").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "nor publish what they built before"
    );

    built(
        &app,
        &owner,
        ws,
        scheduled("Admin's", vec![post_to(open, "hi")]),
    )
    .await;

    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/workflows/{}", mine["id"].as_str().unwrap()),
        Some(&owner),
        Some(scheduled("Taken over", vec![post_to(open, "hi")])),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin doesn't rewrite someone else's"
    );
    let (status, _) = act(&app, &owner, &mine, "disable").await;
    assert_eq!(status, StatusCode::OK, "but can turn it off");
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workflows/{}", mine["id"].as_str().unwrap()),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "and delete it");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_webhook_call_runs_once_per_request_and_posts_as_the_workflow(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) = workspace_with_member(&app, &state, "wf-hook").await;
    let incidents = seed_channel(&state, ws, owner_id, "incidents", false).await;
    let (bob_id, _) = seed(&state, "wf-hook-bob", false).await;
    add_ws_member(&state, ws, bob_id, "member").await;
    let (carol_id, _) = seed(&state, "wf-hook-carol", false).await;
    add_ws_member(&state, ws, carol_id, "member").await;
    for user in [member_id, bob_id, carol_id] {
        state
            .workspace_service
            .repo
            .add_channel_member(
                incidents,
                user,
                &crate::workspace::models::ChannelRole::Member,
            )
            .await
            .unwrap();
    }

    let draft = built(
        &app,
        &member,
        ws,
        webhook(
            "Page",
            &["service", "code"],
            vec![post_to(
                incidents,
                &format!(
                    "{{{{webhook.service}}}} returned {{{{ webhook.code }}}}, @[Bob]({bob_id})"
                ),
            )],
        ),
    )
    .await;
    let path = hook_path(&draft);

    let (status, _) = call(&app, "/api/workflows/webhook/not-a-token", None, json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(&app, &path, None, json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "a draft doesn't take calls");

    let (status, _) = act(&app, &member, &draft, "publish").await;
    assert_eq!(status, StatusCode::OK);
    let body =
        json!({ "service": format!("api @[Carol]({carol_id})"), "code": 503, "extra": "ignored" });
    let (status, first) = call(&app, &path, Some("req-1"), body.clone()).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{first:?}");
    let (_, again) = call(&app, &path, Some("req-1"), body.clone()).await;
    assert_eq!(
        again["run_id"], first["run_id"],
        "a redelivered request is the same run"
    );

    assert_eq!(runner::run_pending(&state).await.unwrap(), 1);
    assert_eq!(runner::run_pending(&state).await.unwrap(), 0);

    let posted = bot_messages(&state, incidents).await;
    assert_eq!(posted.len(), 1, "one run, one message");
    let (content, metadata) = &posted[0];
    assert_eq!(
        content,
        &format!("api @[Carol]({carol_id}) returned 503, @[Bob]({bob_id})")
    );
    assert_eq!(metadata["bot"]["workflow_id"], draft["id"]);
    assert_eq!(metadata["bot"]["name"], "Page");

    let mentions: Vec<(Uuid, i32)> = sqlx::query_as(
        "SELECT user_id, mention_count FROM channel_members WHERE channel_id = $1 AND user_id = ANY($2) ORDER BY user_id = $3 DESC",
    )
    .bind(incidents)
    .bind(vec![bob_id, carol_id])
    .bind(bob_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(
        mentions,
        vec![(bob_id, 1), (carol_id, 0)],
        "the builder's mention counts; one smuggled in through a variable doesn't"
    );

    let history = runs(&app, &member, &draft).await;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["status"], "succeeded");
    assert_eq!(history[0]["steps"][0]["succeeded"], true);
    assert!(history[0]["steps"][0]["message_id"].is_string());

    let (status, second) = call(&app, &path, None, body).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_ne!(
        second["run_id"], first["run_id"],
        "without a key every call is its own run"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_busy_webhook_is_rate_limited(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, owner, _, _) = workspace_with_member(&app, &state, "wf-busy").await;
    let open = seed_channel(&state, ws, owner_id, "open", false).await;
    let workflow = published(
        &app,
        &owner,
        ws,
        webhook("Busy", &[], vec![post_to(open, "ping")]),
    )
    .await;
    let path = hook_path(&workflow);

    for _ in 0..crate::workflows::routes::WEBHOOK_CALLS_PER_MINUTE {
        let (status, _) = call(&app, &path, None, json!({})).await;
        assert_eq!(status, StatusCode::ACCEPTED);
    }
    let (status, _) = call(&app, &path, None, json!({})).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_schedule_queues_one_run_per_instant_and_skips_one_it_missed(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, owner, _, _) = workspace_with_member(&app, &state, "wf-sched").await;
    let open = seed_channel(&state, ws, owner_id, "open", false).await;
    let workflow = published(
        &app,
        &owner,
        ws,
        scheduled("Standup", vec![post_to(open, "standup in 10")]),
    )
    .await;
    let id = Uuid::parse_str(workflow["id"].as_str().unwrap()).unwrap();

    let set_next = |minutes_ago: i64| {
        sqlx::query("UPDATE workflows SET next_run_at = NOW() - make_interval(mins => $2::int) WHERE id = $1")
            .bind(id)
            .bind(minutes_ago as i32)
            .execute(&state.pool)
    };
    let next_run = || async {
        sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>(
            "SELECT next_run_at FROM workflows WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap()
    };

    set_next(1).await.unwrap();
    let due: chrono::DateTime<chrono::Utc> = next_run().await;
    assert_eq!(runner::queue_due_schedules(&state).await.unwrap(), 1);
    assert!(
        next_run().await > chrono::Utc::now(),
        "it moves on to the next instant"
    );
    assert_eq!(runner::queue_due_schedules(&state).await.unwrap(), 0);

    sqlx::query("UPDATE workflows SET next_run_at = $2 WHERE id = $1")
        .bind(id)
        .bind(due)
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(runner::queue_due_schedules(&state).await.unwrap(), 1);
    let queued: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflow_runs WHERE workflow_id = $1")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(queued, 1, "the same instant delivered twice is one run");

    runner::run_pending(&state).await.unwrap();
    assert_eq!(bot_messages(&state, open).await.len(), 1);

    set_next(120).await.unwrap();
    assert_eq!(
        runner::queue_due_schedules(&state).await.unwrap(),
        0,
        "a standup reminder two hours late is worse than none"
    );
    assert!(next_run().await > chrono::Utc::now());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn each_step_runs_with_what_its_creator_can_do_at_that_moment(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-authority").await;
    let open = seed_channel(&state, ws, owner_id, "open", false).await;
    let secret = seed_channel(&state, ws, owner_id, "secret", true).await;
    state
        .workspace_service
        .repo
        .add_channel_member(
            secret,
            member_id,
            &crate::workspace::models::ChannelRole::Member,
        )
        .await
        .unwrap();

    let workflow = published(
        &app,
        &member,
        ws,
        webhook(
            "Two places",
            &[],
            vec![post_to(open, "first"), post_to(secret, "second")],
        ),
    )
    .await;
    state
        .workspace_service
        .repo
        .remove_channel_member(secret, member_id)
        .await
        .unwrap();

    call(&app, &hook_path(&workflow), None, json!({})).await;
    runner::run_pending(&state).await.unwrap();

    assert_eq!(
        bot_messages(&state, open).await.len(),
        1,
        "the step it could still do ran"
    );
    assert!(bot_messages(&state, secret).await.is_empty());
    let history = runs(&app, &member, &workflow).await;
    assert_eq!(history[0]["status"], "failed");
    assert_eq!(
        history[0]["error"],
        "Step 2 (Send a message): Its creator can no longer post in that channel"
    );
    let steps = history[0]["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["succeeded"], true);
    assert_eq!(steps[1]["succeeded"], false);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_dm_step_messages_the_person_in_one_conversation(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, _, _, member_id, member) = workspace_with_member(&app, &state, "wf-dm").await;
    let (bob_id, _) = seed(&state, "wf-dm-bob", false).await;
    add_ws_member(&state, ws, bob_id, "member").await;

    let workflow = published(
        &app,
        &member,
        ws,
        webhook(
            "Welcome",
            &["topic"],
            vec![dm_to(bob_id, "Read up on {{webhook.topic}}")],
        ),
    )
    .await;
    let path = hook_path(&workflow);
    call(&app, &path, None, json!({ "topic": "onboarding" })).await;
    call(&app, &path, None, json!({ "topic": "the runbook" })).await;
    runner::run_pending(&state).await.unwrap();

    let conversation = state
        .conversation_repo
        .find_direct(ws, member_id, bob_id)
        .await
        .unwrap()
        .expect("a DM between the builder and the recipient");
    let contents: Vec<String> = bot_messages(&state, conversation.id)
        .await
        .into_iter()
        .map(|(content, _)| content)
        .collect();
    assert_eq!(
        contents,
        vec!["Read up on onboarding", "Read up on the runbook"]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_creator_leaving_turns_their_workflows_off_and_tells_the_admins(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, owner, member_id, member) =
        workspace_with_member(&app, &state, "wf-leave").await;
    let open = seed_channel(&state, ws, owner_id, "open", false).await;
    let workflow = published(
        &app,
        &member,
        ws,
        webhook("Pager", &[], vec![post_to(open, "ping")]),
    )
    .await;
    let id = Uuid::parse_str(workflow["id"].as_str().unwrap()).unwrap();

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws}/members/{member_id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let enabled: bool = sqlx::query_scalar("SELECT enabled FROM workflows WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(!enabled);
    let told: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT title, body FROM notifications WHERE user_id = $1 AND data ->> 'workflow_id' = $2",
    )
    .bind(owner_id)
    .bind(id.to_string())
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(
        told,
        vec![(
            "A workflow was turned off".to_string(),
            Some(
                "\u{201c}Pager\u{201d} stopped because the person who built it left the workspace."
                    .to_string()
            )
        )]
    );
    let (status, _) = call(&app, &hook_path(&workflow), None, json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);

    add_ws_member(&state, ws, member_id, "member").await;
    let (status, _) = act(&app, &member, &workflow, "publish").await;
    assert_eq!(status, StatusCode::OK);
    call(&app, &hook_path(&workflow), None, json!({})).await;
    sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
        .bind(ws)
        .bind(member_id)
        .execute(&state.pool)
        .await
        .unwrap();
    runner::run_pending(&state).await.unwrap();
    assert!(
        bot_messages(&state, open).await.is_empty(),
        "nothing posts on a departed member's behalf"
    );
    let (status, history) = send(
        &app,
        "GET",
        &format!("/api/workflows/{id}/runs"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        history["data"][0]["error"],
        "The person who built it is no longer in this workspace"
    );
    let enabled: bool = sqlx::query_scalar("SELECT enabled FROM workflows WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(!enabled, "the run that noticed turns it off too");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_unused_hook_types_are_gone(pool: PgPool) {
    let (_, state) = app_and_state(pool).await;
    let types: Vec<String> = sqlx::query_scalar("SELECT unnest(enum_range(NULL::hook_type))::text")
        .fetch_all(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        types,
        vec!["incoming_webhook", "outgoing_webhook", "slash_command"]
    );
}
