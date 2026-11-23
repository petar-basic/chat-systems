use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use super::interactions::{fake_app, FakeApp};
use super::workflows::*;
use crate::app_events::sender;
use crate::authz::Stopped;
use crate::state::AppState;
use crate::workflows::runner;

struct Steps {
    app: axum::Router,
    state: Arc<AppState>,
    ws: Uuid,
    owner_id: Uuid,
    owner: String,
    member_id: Uuid,
    member: String,
    channel: Uuid,
    app_id: Uuid,
    bot: String,
    to: FakeApp,
}

fn ticket_steps() -> Value {
    json!({ "steps": [{
        "callback_id": "open_ticket",
        "title": "Open ticket",
        "description": "Files a ticket with the help desk",
        "inputs": [
            { "name": "title", "title": "Title", "kind": "text", "required": true },
            { "name": "priority", "title": "Priority", "kind": "number" },
            { "name": "reporter", "title": "Reporter", "kind": "person" },
        ],
        "outputs": [
            { "name": "ticket_id", "title": "Ticket id", "kind": "text", "required": true },
            { "name": "urgent", "title": "Urgent", "kind": "boolean" },
        ],
    }] })
}

fn open_ticket(app_id: Uuid, values: Value) -> Value {
    json!({ "type": "app_step", "app_id": app_id, "callback_id": "open_ticket", "values": values })
}

fn printer(app_id: Uuid) -> Value {
    open_ticket(
        app_id,
        json!({ "title": "Printer on 3 is jammed", "priority": "2", "reporter": "{{trigger.user}}" }),
    )
}

fn branch(value: &str, test: Value, then: Vec<Value>) -> Value {
    json!({ "type": "branch", "value": value, "test": test, "then": then, "otherwise": [] })
}

async fn steps_setup(pool: PgPool, prefix: &str) -> Steps {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, owner, member_id, member) =
        workspace_with_member(&app, &state, prefix).await;
    let channel = seed_channel(&state, ws, owner_id, "helpdesk", false).await;
    join(&state, channel, &[member_id]).await;
    let (status, made) = send(
        &app,
        "POST",
        &format!("/api/v1/workspaces/{ws}/bot-apps"),
        Some(&owner),
        Some(json!({ "name": "Ticketing", "scopes": ["chat:write", "workflow.steps:execute"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{made:?}");
    let to = fake_app().await;
    let s = Steps {
        app_id: made["id"].as_str().expect("id").parse().expect("uuid"),
        bot: made["token"].as_str().expect("token").to_string(),
        app,
        state,
        ws,
        owner_id,
        owner,
        member_id,
        member,
        channel,
        to,
    };
    s.configure(json!({ "request_url": s.to.url, "events": ["function_executed"] }))
        .await;
    let (status, body) = s.define(&s.owner, ticket_steps()).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    s
}

impl Steps {
    async fn configure(&self, settings: Value) {
        let (status, body) = send(
            &self.app,
            "PATCH",
            &format!("/api/v1/bot-apps/{}", self.app_id),
            Some(&self.owner),
            Some(settings),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }

    async fn define(&self, token: &str, body: Value) -> (StatusCode, Value) {
        send(
            &self.app,
            "PUT",
            &format!("/api/v1/bot-apps/{}/workflow-steps", self.app_id),
            Some(token),
            Some(body),
        )
        .await
    }

    async fn providers(&self, token: &str) -> (StatusCode, Value) {
        send(
            &self.app,
            "GET",
            &format!("/api/v1/workspaces/{}/workflow-step-apps", self.ws),
            Some(token),
            None,
        )
        .await
    }

    async fn settle(
        &self,
        token: &str,
        execution: &str,
        how: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        send(
            &self.app,
            "POST",
            &format!("/api/v1/workflow-executions/{execution}/{how}"),
            Some(token),
            Some(body),
        )
        .await
    }

    async fn send_due(&self) {
        sender::send_due(&self.state, &sender::client().expect("client")).await;
    }

    async fn ticket_workflow(&self, steps: Vec<Value>) -> Value {
        built(
            &self.app,
            &self.member,
            self.ws,
            shortcut("Report a problem", self.channel, steps),
        )
        .await
    }

    async fn run_until_the_app_is_asked(&self, workflow: &Value) -> String {
        start(&self.app, &self.member, workflow, None).await;
        runner::run_pending(&self.state).await.expect("run");
        self.send_due().await;
        let event = self.to.last();
        assert_eq!(event["type"], "function_executed", "{event:?}");
        event["event"]["execution_id"]
            .as_str()
            .expect("an execution id")
            .to_string()
    }

    async fn last_run(&self, workflow: &Value) -> Value {
        runs(&self.app, &self.member, workflow).await.remove(0)
    }

    async fn notices(&self, user: Uuid) -> Vec<Value> {
        sqlx::query_scalar::<_, Value>(
            "SELECT data FROM notifications WHERE user_id = $1 AND data->>'notice' = 'workflow_app_gone'",
        )
        .bind(user)
        .fetch_all(&self.state.pool)
        .await
        .expect("notifications")
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn admins_and_the_app_define_up_to_twenty_typed_steps(pool: PgPool) {
    let s = steps_setup(pool, "wfa-define").await;

    let (status, _) = s.define(&s.member, ticket_steps()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a member can't");

    let many: Vec<Value> = (0..21)
        .map(|i| json!({ "callback_id": format!("step_{i}"), "title": format!("Step {i}") }))
        .collect();
    let (status, body) = s.define(&s.owner, json!({ "steps": many })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    assert!(
        body["error"].as_str().unwrap().contains("at most 20"),
        "{body:?}"
    );

    for broken in [
        json!({ "steps": [
            { "callback_id": "same", "title": "One" },
            { "callback_id": "same", "title": "Two" },
        ] }),
        json!({ "steps": [{ "callback_id": "has space", "title": "One" }] }),
        json!({ "steps": [{ "callback_id": "one", "title": "One",
            "inputs": [{ "name": "Bad Name", "title": "Bad", "kind": "text" }] }] }),
        json!({ "steps": [{ "callback_id": "one", "title": "One",
            "outputs": [
                { "name": "id", "title": "Id", "kind": "text" },
                { "name": "id", "title": "Id again", "kind": "number" },
            ] }] }),
        json!({ "steps": [{ "callback_id": "one", "title": "One",
            "inputs": [{ "name": "when", "title": "When", "kind": "time" }] }] }),
    ] {
        let (status, body) = s.define(&s.owner, broken).await;
        assert!(
            status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::BAD_REQUEST,
            "{status} {body:?}"
        );
    }

    let (status, listed) = send(
        &s.app,
        "GET",
        &format!("/api/v1/bot-apps/{}/workflow-steps", s.app_id),
        Some(&s.owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed:?}");
    let step = &listed["steps"][0];
    assert_eq!(step["callback_id"], "open_ticket");
    assert_eq!(step["inputs"][1]["kind"], "number");
    assert_eq!(step["outputs"][0]["required"], true);
    let id = step["id"].clone();

    let mut renamed = ticket_steps();
    renamed["steps"][0]["title"] = json!("File a ticket");
    let (status, body) = send(
        &s.app,
        "PUT",
        "/api/v1/bot-apps/me/workflow-steps",
        Some(&s.bot),
        Some(renamed),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the app sends its own: {body:?}");
    assert_eq!(body["steps"][0]["title"], "File a ticket");
    assert_eq!(body["steps"][0]["id"], id, "a step keeps its id");

    let (status, providers) = s.providers(&s.member).await;
    assert_eq!(status, StatusCode::OK, "{providers:?}");
    let ticketing = &providers["data"][0];
    assert_eq!(ticketing["name"], "Ticketing");
    assert!(ticketing["problem"].is_null(), "{ticketing:?}");
    assert_eq!(ticketing["steps"][0]["title"], "File a ticket");

    s.configure(json!({ "scopes": ["chat:write"] })).await;
    let (status, _) = send(
        &s.app,
        "PUT",
        "/api/v1/bot-apps/me/workflow-steps",
        Some(&s.bot),
        Some(ticket_steps()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "it needs workflow.steps:execute"
    );
    let (_, providers) = s.providers(&s.member).await;
    assert_eq!(
        providers["data"][0]["problem"],
        "Ticketing doesn't have the workflow.steps:execute scope"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_workflow_checks_the_types_of_what_it_hands_a_step(pool: PgPool) {
    let s = steps_setup(pool, "wfa-types").await;
    let date_form = json!({ "type": "form", "to": { "kind": "trigger_user" }, "title": "When",
        "fields": [{ "name": "due", "label": "Due", "kind": "date" }] });

    for (steps, error) in [
        (
            vec![open_ticket(s.app_id, json!({ "priority": "2" }))],
            "Step 1: fill in \u{201c}Title\u{201d}",
        ),
        (
            vec![open_ticket(
                s.app_id,
                json!({ "title": "Jam", "priority": "two" }),
            )],
            "Step 1: \u{201c}Priority\u{201d} needs a number",
        ),
        (
            vec![open_ticket(
                s.app_id,
                json!({ "title": "Jam", "priority": "{{trigger.user}}" }),
            )],
            "Step 1: \u{201c}Priority\u{201d} needs a number, and {{trigger.user}} isn't one",
        ),
        (
            vec![
                date_form.clone(),
                open_ticket(
                    s.app_id,
                    json!({ "title": "Jam", "reporter": "{{form.due}}" }),
                ),
            ],
            "Step 2: \u{201c}Reporter\u{201d} needs a person, and {{form.due}} isn't one",
        ),
        (
            vec![open_ticket(
                s.app_id,
                json!({ "title": "Jam", "eta": "soon" }),
            )],
            "Step 1: \"eta\" isn't an input of this step",
        ),
        (
            vec![
                open_ticket(s.app_id, json!({ "title": "Jam" })),
                post_to(s.channel, "Filed {{step.1.ticket}}"),
            ],
            "Step 2: {{step.1.ticket}} isn't available for this trigger",
        ),
        (
            vec![json!({ "type": "app_step", "app_id": s.app_id, "callback_id": "close_ticket" })],
            "Step 1: Ticketing no longer offers this step",
        ),
        (
            vec![
                json!({ "type": "app_step", "app_id": Uuid::new_v4(), "callback_id": "open_ticket" }),
            ],
            "Step 1: the app that runs this step was removed",
        ),
    ] {
        let (status, body) = create(
            &s.app,
            &s.member,
            s.ws,
            shortcut("Report a problem", s.channel, steps),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
        assert_eq!(body["error"], error);
    }

    let workflow = s
        .ticket_workflow(vec![
            date_form,
            open_ticket(
                s.app_id,
                json!({ "title": "Due {{form.due}}", "reporter": "{{trigger.user}}" }),
            ),
            post_to(s.channel, "Filed {{step.2.ticket_id}}"),
        ])
        .await;
    let step = &workflow["definition"]["steps"][1];
    assert_eq!(step["title"], "Open ticket", "the server fills in the step");
    assert_eq!(step["outputs"][0]["name"], "ticket_id");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_workflow_opens_a_ticket_through_the_app_and_posts_its_id(pool: PgPool) {
    let s = steps_setup(pool, "wfa-ticket").await;
    let workflow = s
        .ticket_workflow(vec![
            printer(s.app_id),
            post_to(s.channel, "Ticket {{step.1.ticket_id}} opened"),
            branch(
                "step.1.urgent",
                json!({ "op": "equals", "value": "true" }),
                vec![post_to(
                    s.channel,
                    "Paging on call about {{step.1.ticket_id}}",
                )],
            ),
        ])
        .await;

    start(&s.app, &s.member, &workflow, None).await;
    runner::run_pending(&s.state).await.expect("run");
    let run = s.last_run(&workflow).await;
    assert_eq!(run["status"], "waiting", "{run:?}");
    assert_eq!(run["waiting"]["label"], "Open ticket");
    assert!(
        run["waiting"]["until"].is_string(),
        "it waits an hour at most"
    );

    s.send_due().await;
    let event = s.to.last();
    assert_eq!(event["type"], "function_executed");
    assert_eq!(event["workspace_id"], json!(s.ws));
    let body = &event["event"];
    assert_eq!(body["run_id"], run["id"]);
    assert_eq!(body["step"]["callback_id"], "open_ticket");
    assert_eq!(body["step"]["position"], 1);
    assert_eq!(body["workflow"]["name"], "Report a problem");
    assert_eq!(
        body["inputs"],
        json!({ "title": "Printer on 3 is jammed", "priority": 2, "reporter": s.member_id })
    );
    let execution = body["execution_id"].as_str().expect("id").to_string();

    for (outputs, error) in [
        (json!({ "ticket_id": 42 }), "outputs.ticket_id needs text"),
        (json!({ "urgent": true }), "outputs.ticket_id is required"),
        (
            json!({ "ticket_id": "T-42", "urgent": "yes" }),
            "outputs.urgent needs true or false",
        ),
        (
            json!({ "ticket_id": "T-42", "eta": "soon" }),
            "outputs.eta: the step has no such output",
        ),
    ] {
        let (status, body) = s
            .settle(
                &s.bot,
                &execution,
                "complete",
                json!({ "outputs": outputs }),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
        assert_eq!(body["error"], error);
    }
    runner::run_pending(&s.state).await.expect("run");
    assert_eq!(
        s.last_run(&workflow).await["status"],
        "waiting",
        "a refused answer leaves it waiting"
    );
    assert!(bot_messages(&s.state, s.channel).await.is_empty());

    let (status, body) = s
        .settle(
            &s.bot,
            &execution,
            "complete",
            json!({ "outputs": { "ticket_id": "T-42", "urgent": true } }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, _) = s
        .settle(
            &s.bot,
            &execution,
            "complete",
            json!({ "outputs": { "ticket_id": "T-43" } }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "a step finishes once");

    runner::run_pending(&s.state).await.expect("run");
    let posted: Vec<String> = bot_messages(&s.state, s.channel)
        .await
        .into_iter()
        .map(|(content, _)| content)
        .collect();
    assert_eq!(
        posted,
        vec!["Ticket T-42 opened", "Paging on call about T-42"]
    );
    let run = s.last_run(&workflow).await;
    assert_eq!(run["status"], "succeeded", "{run:?}");
    let step = &run["steps"][0];
    assert_eq!(step["label"], "Open ticket");
    assert_eq!(step["inputs"]["title"], "Printer on 3 is jammed");
    assert_eq!(
        step["outputs"],
        json!({ "ticket_id": "T-42", "urgent": true })
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_app_that_fails_the_step_stops_the_run_with_its_message(pool: PgPool) {
    let s = steps_setup(pool, "wfa-fail").await;
    let workflow = s
        .ticket_workflow(vec![
            printer(s.app_id),
            post_to(s.channel, "Ticket {{step.1.ticket_id}} opened"),
        ])
        .await;
    let execution = s.run_until_the_app_is_asked(&workflow).await;

    let (status, _) = s
        .settle(&s.member, &execution, "fail", json!({ "error": "nope" }))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "only an app's bot token");
    let (status, _) = s
        .settle(&s.bot, &execution, "fail", json!({ "error": "  " }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = s
        .settle(
            &s.bot,
            &Uuid::new_v4().to_string(),
            "fail",
            json!({ "error": "x" }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = s
        .settle(
            &s.bot,
            &execution,
            "fail",
            json!({ "error": "The help desk is down for maintenance" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    runner::run_pending(&s.state).await.expect("run");
    assert!(
        bot_messages(&s.state, s.channel).await.is_empty(),
        "the steps after it don't run"
    );
    let run = s.last_run(&workflow).await;
    assert_eq!(run["status"], "failed");
    assert_eq!(
        run["error"],
        "Step 1 (Open ticket): The help desk is down for maintenance"
    );
    assert_eq!(
        run["steps"][0]["error"],
        "The help desk is down for maintenance"
    );
    assert_eq!(run["steps"][0]["inputs"]["priority"], 2);
    let (status, _) = s
        .settle(
            &s.bot,
            &execution,
            "complete",
            json!({ "outputs": { "ticket_id": "T-1" } }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_the_app_with_the_scope_finishes_its_step(pool: PgPool) {
    let s = steps_setup(pool, "wfa-scope").await;
    let workflow = s.ticket_workflow(vec![printer(s.app_id)]).await;
    let execution = s.run_until_the_app_is_asked(&workflow).await;

    let (status, other) = send(
        &s.app,
        "POST",
        &format!("/api/v1/workspaces/{}/bot-apps", s.ws),
        Some(&s.owner),
        Some(json!({ "name": "Impostor", "scopes": ["workflow.steps:execute"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{other:?}");
    let impostor = other["token"].as_str().expect("token");
    let (status, _) = s
        .settle(
            impostor,
            &execution,
            "complete",
            json!({ "outputs": { "ticket_id": "T-9" } }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another app's step is not there"
    );

    s.configure(json!({ "scopes": ["chat:write"] })).await;
    let (status, _) = s
        .settle(
            &s.bot,
            &execution,
            "complete",
            json!({ "outputs": { "ticket_id": "T-9" } }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "it needs workflow.steps:execute"
    );
    assert_eq!(s.last_run(&workflow).await["status"], "waiting");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_step_the_app_never_finishes_fails_after_an_hour(pool: PgPool) {
    let s = steps_setup(pool, "wfa-timeout").await;
    let workflow = s
        .ticket_workflow(vec![
            printer(s.app_id),
            post_to(s.channel, "Ticket {{step.1.ticket_id}} opened"),
        ])
        .await;
    let execution = s.run_until_the_app_is_asked(&workflow).await;

    assert_eq!(runner::expire_app_calls(&s.state).await.expect("expire"), 0);
    let left: f64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM expires_at - NOW())::float8 FROM workflow_app_calls WHERE id = $1",
    )
    .bind(execution.parse::<Uuid>().expect("uuid"))
    .fetch_one(&s.state.pool)
    .await
    .expect("deadline");
    assert!((3500.0..=3600.0).contains(&left), "an hour, got {left}");

    sqlx::query("UPDATE workflow_app_calls SET expires_at = NOW() - INTERVAL '1 second'")
        .execute(&s.state.pool)
        .await
        .expect("age");
    assert_eq!(runner::expire_app_calls(&s.state).await.expect("expire"), 1);
    assert_eq!(runner::expire_app_calls(&s.state).await.expect("expire"), 0);

    runner::run_pending(&s.state).await.expect("run");
    assert!(bot_messages(&s.state, s.channel).await.is_empty());
    let run = s.last_run(&workflow).await;
    assert_eq!(run["status"], "failed");
    assert_eq!(run["error"], "Step 1 (Open ticket): The app did not finish");
    let (status, body) = s
        .settle(
            &s.bot,
            &execution,
            "complete",
            json!({ "outputs": { "ticket_id": "T-1" } }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "too late");
    assert_eq!(body["error"], "This step ran out of time");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_socket_mode_app_gets_the_event_on_its_socket(pool: PgPool) {
    let s = steps_setup(pool, "wfa-socket").await;
    s.configure(json!({ "request_url": "", "socket_mode": true }))
        .await;
    let workflow = s.ticket_workflow(vec![printer(s.app_id)]).await;
    start(&s.app, &s.member, &workflow, None).await;
    runner::run_pending(&s.state).await.expect("run");
    s.send_due().await;
    assert!(s.to.payloads().is_empty(), "nothing goes over HTTP");

    let queued: Vec<(String, Value)> = sqlx::query_as(
        "SELECT kind, body FROM app_deliveries WHERE app_id = $1 AND delivered_at IS NULL",
    )
    .bind(s.app_id)
    .fetch_all(&s.state.pool)
    .await
    .expect("deliveries");
    assert_eq!(queued.len(), 1, "{queued:?}");
    let (kind, envelope) = &queued[0];
    assert_eq!(kind, "event", "it rides the socket like any event");
    assert_eq!(envelope["type"], "function_executed");
    assert_eq!(
        envelope["event"]["inputs"]["title"],
        "Printer on 3 is jammed"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_turned_off_app_fails_its_steps_and_tells_the_owner_once(pool: PgPool) {
    let s = steps_setup(pool, "wfa-off").await;
    let workflow = s
        .ticket_workflow(vec![
            printer(s.app_id),
            post_to(s.channel, "Ticket {{step.1.ticket_id}} opened"),
        ])
        .await;

    crate::workspace::membership::stand_down(&s.state, Some(s.ws), s.owner_id, Stopped::Inactive)
        .await;
    let notices = s.notices(s.member_id).await;
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(notices[0]["workflow_name"], "Report a problem");
    assert_eq!(notices[0]["app_name"], "Ticketing");
    assert!(s.notices(s.owner_id).await.is_empty());

    let (_, providers) = s.providers(&s.member).await;
    assert_eq!(providers["data"][0]["problem"], "Ticketing was turned off");

    start(&s.app, &s.member, &workflow, None).await;
    runner::run_pending(&s.state).await.expect("run");
    let run = s.last_run(&workflow).await;
    assert_eq!(run["status"], "failed");
    assert_eq!(
        run["error"],
        "Step 1 (Open ticket): Ticketing was turned off"
    );
    assert!(bot_messages(&s.state, s.channel).await.is_empty());

    let (status, body) = send(
        &s.app,
        "PUT",
        &format!("/api/v1/workflows/{}", workflow["id"].as_str().expect("id")),
        Some(&s.member),
        Some(json!({ "name": "Report a problem", "definition": workflow["definition"] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    assert_eq!(body["error"], "Step 1: Ticketing was turned off");

    let (status, _) = send(
        &s.app,
        "DELETE",
        &format!("/api/v1/bot-apps/{}", s.app_id),
        Some(&s.owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s.notices(s.member_id).await.len(), 1, "once");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn removing_the_app_fails_its_steps_and_tells_the_owner(pool: PgPool) {
    let s = steps_setup(pool, "wfa-gone").await;
    let workflow = s.ticket_workflow(vec![printer(s.app_id)]).await;

    let (status, body) = send(
        &s.app,
        "DELETE",
        &format!("/api/v1/bot-apps/{}", s.app_id),
        Some(&s.owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(s.notices(s.member_id).await.len(), 1);

    start(&s.app, &s.member, &workflow, None).await;
    runner::run_pending(&s.state).await.expect("run");
    let run = s.last_run(&workflow).await;
    assert_eq!(run["status"], "failed");
    assert_eq!(
        run["error"],
        "Step 1 (Open ticket): The app that runs this step was removed"
    );
}
