use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use super::workflows::*;
use crate::state::AppState;
use crate::workflows::runner;

fn branch(value: &str, test: Value, then: Vec<Value>, otherwise: Vec<Value>) -> Value {
    json!({ "type": "branch", "value": value, "test": test, "then": then, "otherwise": otherwise })
}

fn ask(fields: Value) -> Value {
    json!({ "type": "form", "to": { "kind": "trigger_user" }, "title": "Request", "fields": fields })
}

fn question(name: &str, kind: &str) -> Value {
    json!({ "name": name, "label": name, "kind": kind })
}

async fn open_form(state: &AppState, ws: Uuid, from: Uuid, to: Uuid) -> Value {
    let conversation = state
        .conversation_repo
        .find_direct(ws, from, to)
        .await
        .unwrap()
        .expect("the form arrives in a DM");
    bot_messages(state, conversation.id)
        .await
        .into_iter()
        .rev()
        .map(|(_, metadata)| metadata["form"].clone())
        .find(|form| form.is_object() && form["submitted_at"].is_null())
        .expect("an open form")
}

fn texts(messages: Vec<(String, Value)>) -> Vec<String> {
    messages.into_iter().map(|(content, _)| content).collect()
}

async fn channel_named(
    state: &AppState,
    ws: Uuid,
    name: &str,
) -> Option<(Uuid, bool, Option<String>)> {
    sqlx::query_as::<_, (Uuid, bool, Option<String>)>(
        "SELECT id, channel_type = 'private', topic FROM channels WHERE workspace_id = $1 AND name = $2",
    )
    .bind(ws)
    .bind(name)
    .fetch_optional(&state.pool)
    .await
    .unwrap()
}

async fn members_of(state: &AppState, channel: Uuid) -> Vec<Uuid> {
    let mut members: Vec<Uuid> =
        sqlx::query_scalar("SELECT user_id FROM channel_members WHERE channel_id = $1")
            .bind(channel)
            .fetch_all(&state.pool)
            .await
            .unwrap();
    members.sort();
    members
}

fn sorted(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    ids.sort();
    ids
}

async fn set_rule(state: &AppState, ws: Uuid, rule: Value) {
    sqlx::query("UPDATE workspaces SET settings = settings || $2 WHERE id = $1")
        .bind(ws)
        .bind(rule)
        .execute(&state.pool)
        .await
        .unwrap();
}

async fn channel_role(state: &AppState, channel: Uuid, user: Uuid, role: &str) {
    sqlx::query(
        "UPDATE channel_members SET role = $3::channel_role WHERE channel_id = $1 AND user_id = $2",
    )
    .bind(channel)
    .bind(user)
    .bind(role)
    .execute(&state.pool)
    .await
    .unwrap();
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_urgent_request_pages_on_call_and_any_other_is_filed(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-urgent").await;
    let (bob_id, bob_email) = seed(&state, "wf-urgent-bob", false).await;
    add_ws_member(&state, ws, bob_id, "member").await;
    let bob = login(&app, &bob_email, "password123").await;
    let oncall = seed_channel(&state, ws, owner_id, "oncall", false).await;
    let requests = seed_channel(&state, ws, owner_id, "requests", false).await;
    join(&state, oncall, &[member_id]).await;
    join(&state, requests, &[member_id, bob_id]).await;

    let workflow = built(
        &app,
        &member,
        ws,
        shortcut(
            "Help",
            requests,
            vec![
                ask(json!([
                    { "name": "urgency", "label": "Urgency", "kind": "select", "options": ["Urgent", "Can wait"], "required": true },
                    question("what", "short_text"),
                ])),
                branch(
                    "form.urgency",
                    json!({ "op": "equals", "value": "urgent" }),
                    vec![post_to(oncall, "Paging: {{form.what}}")],
                    vec![post_to(requests, "Filed: {{form.what}}")],
                ),
                post_to(requests, "Thanks {{trigger.user}}"),
            ],
        ),
    )
    .await;

    for (urgency, what) in [("Urgent", "prod is down"), ("Can wait", "new laptop")] {
        start(&app, &bob, &workflow, None).await;
        runner::run_pending(&state).await.unwrap();
        let form = open_form(&state, ws, member_id, bob_id).await;
        let (status, body) = fill(
            &app,
            &bob,
            &form,
            json!({ "urgency": urgency, "what": what }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        runner::run_pending(&state).await.unwrap();
    }

    let thanks = format!("Thanks @[Test User]({bob_id})");
    assert_eq!(
        texts(bot_messages(&state, oncall).await),
        ["Paging: prod is down"]
    );
    assert_eq!(
        texts(bot_messages(&state, requests).await),
        [thanks.clone(), "Filed: new laptop".into(), thanks]
    );

    let history = runs(&app, &member, &workflow).await;
    let branch_of = |run: &Value| {
        run["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|step| step["position"] == 1)
            .cloned()
            .unwrap()
    };
    let (filed, paged) = (branch_of(&history[0]), branch_of(&history[1]));
    assert_eq!(paged["label"], "Branch");
    assert_eq!(paged["matched"], true);
    assert_eq!(filed["matched"], false);
    let positions = |run: &Value| -> Vec<i64> {
        run["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|step| step["position"].as_i64().unwrap())
            .collect()
    };
    assert_eq!(positions(&history[1]), [0, 1, 2, 4]);
    assert_eq!(positions(&history[0]), [0, 1, 3, 4]);
    assert!(history.iter().all(|run| run["status"] == "succeeded"));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn each_condition_and_a_nested_branch_pick_their_steps(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-conditions").await;
    let alerts = seed_channel(&state, ws, owner_id, "alerts", false).await;
    join(&state, alerts, &[member_id]).await;

    let workflow = built(
        &app,
        &member,
        ws,
        webhook(
            "Alerts",
            &["service", "code", "note"],
            vec![
                branch(
                    "webhook.code",
                    json!({ "op": "greater_than", "value": "499" }),
                    vec![branch(
                        "webhook.service",
                        json!({ "op": "one_of", "values": ["API", "web"] }),
                        vec![post_to(alerts, "{{webhook.service}} is down")],
                        vec![post_to(alerts, "something else is down")],
                    )],
                    vec![post_to(alerts, "all fine")],
                ),
                branch(
                    "webhook.note",
                    json!({ "op": "contains", "value": "DEPLOY" }),
                    vec![post_to(alerts, "after a deploy")],
                    vec![],
                ),
                branch(
                    "webhook.note",
                    json!({ "op": "is_empty" }),
                    vec![post_to(alerts, "no note")],
                    vec![],
                ),
            ],
        ),
    )
    .await;

    let path = hook_path(&workflow);
    let calls = [
        (
            json!({ "service": "api", "code": "503", "note": "right after the deploy" }),
            vec!["api is down", "after a deploy"],
        ),
        (
            json!({ "service": "db", "code": 502 }),
            vec!["something else is down", "no note"],
        ),
        (
            json!({ "service": "api", "code": "200", "note": "" }),
            vec!["all fine", "no note"],
        ),
    ];
    let mut expected = Vec::new();
    for (index, (body, posted)) in calls.into_iter().enumerate() {
        let (status, _) = call(&app, &path, Some(&format!("call-{index}")), body).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        runner::run_pending(&state).await.unwrap();
        expected.extend(posted.into_iter().map(String::from));
        assert_eq!(texts(bot_messages(&state, alerts).await), expected);
    }

    let history = runs(&app, &member, &workflow).await;
    let matched = |run: &Value| -> Vec<(i64, Value)> {
        run["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|step| step["label"] == "Branch")
            .map(|step| (step["position"].as_i64().unwrap(), step["matched"].clone()))
            .collect()
    };
    assert_eq!(
        matched(&history[2]),
        [
            (0, json!(true)),
            (1, json!(true)),
            (5, json!(true)),
            (7, json!(false))
        ]
    );
    assert_eq!(
        matched(&history[1]),
        [
            (0, json!(true)),
            (1, json!(false)),
            (5, json!(false)),
            (7, json!(true))
        ]
    );
    assert_eq!(
        matched(&history[0]),
        [(0, json!(false)), (5, json!(false)), (7, json!(true))]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_branch_reads_the_trigger_users_profile_and_a_list_column(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, owner, member_id, member) =
        workspace_with_member(&app, &state, "wf-sources").await;
    let (bob_id, bob_email) = seed(&state, "wf-sources-bob", false).await;
    add_ws_member(&state, ws, bob_id, "member").await;
    let bob = login(&app, &bob_email, "password123").await;
    let requests = seed_channel(&state, ws, owner_id, "requests", false).await;
    join(&state, requests, &[member_id, bob_id]).await;

    let (status, office) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/profile-fields"),
        Some(&owner),
        Some(json!({ "label": "Office", "kind": "options", "options": [{ "label": "Berlin" }, { "label": "Belgrade" }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{office:?}");
    let belgrade = office["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| option["label"] == "Belgrade")
        .unwrap()["id"]
        .clone();
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/workspaces/{ws}/members/{bob_id}/profile-fields"),
        Some(&bob),
        Some(json!({ "values": { office["id"].as_str().unwrap(): belgrade } })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let list = requests_list(&app, &member, ws, None).await;
    let status_column = column_id(&list, "Status");
    let workflow = built(
        &app,
        &member,
        ws,
        shortcut(
            "Sources",
            requests,
            vec![
                add_to(
                    &list,
                    vec![
                        fill_cell(&list, "Request", "desk"),
                        fill_cell(&list, "Status", "Open"),
                    ],
                ),
                branch(
                    &format!("step.1.column.{status_column}"),
                    json!({ "op": "equals", "value": "open" }),
                    vec![post_to(requests, "still open")],
                    vec![post_to(requests, "closed")],
                ),
                branch(
                    &format!("profile.{}", office["id"].as_str().unwrap()),
                    json!({ "op": "equals", "value": "Belgrade" }),
                    vec![post_to(requests, "from Belgrade")],
                    vec![post_to(requests, "from elsewhere")],
                ),
            ],
        ),
    )
    .await;

    let (status, body) = create(
        &app,
        &member,
        ws,
        shortcut(
            "Bad choice",
            requests,
            vec![branch(
                &format!("profile.{}", office["id"].as_str().unwrap()),
                json!({ "op": "equals", "value": "Paris" }),
                vec![post_to(requests, "x")],
                vec![],
            )],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"],
        "Step 1: \u{201c}Paris\u{201d} isn't one of its choices"
    );

    start(&app, &bob, &workflow, None).await;
    runner::run_pending(&state).await.unwrap();
    start(&app, &owner, &workflow, None).await;
    runner::run_pending(&state).await.unwrap();
    assert_eq!(
        texts(bot_messages(&state, requests).await),
        [
            "still open",
            "from Belgrade",
            "still open",
            "from elsewhere"
        ]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_newcomer_gets_the_checklist_a_day_later_and_only_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-delay").await;
    let lobby = seed_channel(&state, ws, owner_id, "lobby", false).await;
    join(&state, lobby, &[member_id]).await;
    let workflow = built(
        &app,
        &member,
        ws,
        on_join(
            "Checklist",
            lobby,
            vec![
                json!({ "type": "delay", "wait": { "kind": "for", "amount": 1, "unit": "days" } }),
                dm_back("Here is your checklist, {{trigger.user}}"),
            ],
        ),
    )
    .await;

    let (carol_id, _, carol) = seed_and_login(&app, &state, "wf-delay-carol", false).await;
    add_ws_member(&state, ws, carol_id, "member").await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{lobby}/join"),
        Some(&carol),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    deliver(&state).await;
    runner::run_pending(&state).await.unwrap();

    let history = runs(&app, &member, &workflow).await;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["status"], "waiting");
    assert_eq!(history[0]["waiting"]["position"], 0);
    assert_eq!(history[0]["waiting"]["label"], "Wait");
    assert!(history[0]["waiting"]["until"].is_string());
    let a_day_out: bool = sqlx::query_scalar(
        "SELECT resume_at > NOW() + INTERVAL '23 hours' FROM workflow_runs WHERE status = 'waiting'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert!(a_day_out);

    assert_eq!(runner::resume_delays(&state).await.unwrap(), 0);
    runner::run_pending(&state).await.unwrap();
    assert!(dms(&state, ws, member_id, carol_id).await.is_empty());

    let (_, restarted) = app_and_state(state.pool.clone()).await;
    let (_, other) = app_and_state(state.pool.clone()).await;
    sqlx::query("UPDATE workflow_runs SET resume_at = resume_at - INTERVAL '1 day'")
        .execute(&state.pool)
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        runner::resume_delays(&restarted),
        runner::resume_delays(&other)
    );
    assert_eq!(first.unwrap() + second.unwrap(), 1);
    assert_eq!(runner::resume_delays(&restarted).await.unwrap(), 0);
    let (first, second) =
        tokio::join!(runner::run_pending(&restarted), runner::run_pending(&other));
    assert_eq!(first.unwrap() + second.unwrap(), 1);
    runner::run_pending(&restarted).await.unwrap();

    assert_eq!(
        dms(&state, ws, member_id, carol_id).await,
        [format!("Here is your checklist, @[Test User]({carol_id})")]
    );
    let history = runs(&app, &member, &workflow).await;
    assert_eq!(history[0]["status"], "succeeded");
    assert!(history[0]["waiting"].is_null());
    assert_eq!(history[0]["steps"][0]["label"], "Wait");
    assert_eq!(history[0]["steps"][0]["succeeded"], true);
    assert_eq!(history[0]["steps"][1]["label"], "Send a DM");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_wait_until_a_moment_is_checked_against_its_timezone(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-until").await;
    let lobby = seed_channel(&state, ws, owner_id, "lobby", false).await;
    join(&state, lobby, &[member_id]).await;
    let until = |date: String, timezone: &str| {
        on_join(
            "Later",
            lobby,
            vec![
                json!({ "type": "delay", "wait": { "kind": "until", "date": date, "time": "09:30", "timezone": timezone } }),
                dm_back("now"),
            ],
        )
    };
    let day = |days: i64| {
        (chrono::Utc::now() + chrono::Duration::days(days))
            .format("%Y-%m-%d")
            .to_string()
    };
    for (date, timezone, refusal) in [
        (day(-2), "Europe/Belgrade", "Step 1: that time has passed"),
        (
            day(40),
            "Europe/Belgrade",
            "Step 1: a workflow waits at most 30 days",
        ),
        (
            day(3),
            "Mars/Olympus",
            "Step 1: \"Mars/Olympus\" isn't a timezone",
        ),
    ] {
        let (status, body) = create(&app, &member, ws, until(date, timezone)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"], refusal);
    }
    built(&app, &member, ws, until(day(3), "Asia/Tokyo")).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_workflow_makes_a_project_channel_adds_the_people_picked_and_links_it_back(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-project").await;
    let (bob_id, bob_email) = seed(&state, "wf-project-bob", false).await;
    add_ws_member(&state, ws, bob_id, "member").await;
    let bob = login(&app, &bob_email, "password123").await;
    let (dave_id, _) = seed(&state, "wf-project-dave", false).await;
    add_ws_member(&state, ws, dave_id, "member").await;
    let requests = seed_channel(&state, ws, owner_id, "requests", false).await;
    join(&state, requests, &[member_id, bob_id]).await;

    let workflow = built(
        &app,
        &member,
        ws,
        shortcut(
            "New project",
            requests,
            vec![
                ask(json!([
                    question("project", "short_text"),
                    question("lead", "person"),
                    question("helper", "person"),
                ])),
                json!({ "type": "create_channel", "name": "proj-{{form.project}}", "private": false }),
                json!({ "type": "add_to_channel", "channel": { "kind": "from_step", "step": 1 },
                        "people": [{ "kind": "answer", "field": "lead" }, { "kind": "answer", "field": "helper" }, { "kind": "trigger_user" }] }),
                json!({ "type": "set_topic", "channel": { "kind": "from_step", "step": 1 }, "topic": "{{form.project}} launch" }),
                dm_back("Your channel: {{step.2.channel}} ({{step.2.channel_link}})"),
            ],
        ),
    )
    .await;

    start(&app, &bob, &workflow, None).await;
    runner::run_pending(&state).await.unwrap();
    let form = open_form(&state, ws, member_id, bob_id).await;
    let (status, body) = fill(
        &app,
        &bob,
        &form,
        json!({ "project": "Apollo Launch", "lead": owner_id, "helper": dave_id }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    runner::run_pending(&state).await.unwrap();

    let (channel, private, topic) = channel_named(&state, ws, "proj-apollo-launch")
        .await
        .expect("the project channel");
    assert!(!private);
    assert_eq!(topic.as_deref(), Some("Apollo Launch launch"));
    assert_eq!(
        members_of(&state, channel).await,
        sorted(vec![member_id, owner_id, dave_id, bob_id])
    );
    assert_eq!(
        dms(&state, ws, member_id, bob_id).await[1..],
        [format!(
            "Your channel: #proj-apollo-launch (http://localhost/app/{ws}/{channel})"
        )]
    );
    let history = runs(&app, &member, &workflow).await;
    assert_eq!(history[0]["status"], "succeeded", "{history:?}");
    assert_eq!(history[0]["steps"][1]["label"], "Create a channel");
    assert_eq!(history[0]["steps"][1]["channel_id"], json!(channel));

    start(&app, &bob, &workflow, None).await;
    runner::run_pending(&state).await.unwrap();
    let form = open_form(&state, ws, member_id, bob_id).await;
    fill(&app, &bob, &form, json!({ "project": "apollo launch" })).await;
    runner::run_pending(&state).await.unwrap();
    let history = runs(&app, &member, &workflow).await;
    assert_eq!(history[0]["status"], "failed");
    assert_eq!(
        history[0]["error"],
        "Step 2 (Create a channel): A channel named #proj-apollo-launch already exists"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_owner_who_may_no_longer_create_channels_gets_a_clear_error_and_no_channel(
    pool: PgPool,
) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-no-create").await;
    let requests = seed_channel(&state, ws, owner_id, "requests", false).await;
    join(&state, requests, &[member_id]).await;
    let make = |private: bool| {
        shortcut(
            "Make",
            requests,
            vec![json!({ "type": "create_channel", "name": "Made Here", "private": private })],
        )
    };
    let workflow = built(&app, &member, ws, make(false)).await;

    set_rule(&state, ws, json!({ "public_channel_creators": "admins" })).await;
    start(&app, &member, &workflow, None).await;
    runner::run_pending(&state).await.unwrap();
    let history = runs(&app, &member, &workflow).await;
    assert_eq!(history[0]["status"], "failed");
    assert_eq!(
        history[0]["error"],
        "Step 1 (Create a channel): Only admins can create public channels in this workspace"
    );
    assert_eq!(history[0]["steps"][0]["succeeded"], false);
    assert!(channel_named(&state, ws, "made-here").await.is_none());

    let (status, body) = create(&app, &member, ws, make(false)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"],
        "Step 1: Only admins can create public channels in this workspace"
    );
    let private = built(&app, &member, ws, make(true)).await;
    start(&app, &member, &private, None).await;
    runner::run_pending(&state).await.unwrap();
    let (channel, private, _) = channel_named(&state, ws, "made-here")
        .await
        .expect("a private channel is still allowed");
    assert!(private);
    assert_eq!(members_of(&state, channel).await, [member_id]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_reaction_gets_a_reaction_and_a_thread_reply_from_the_owner(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-thread").await;
    let (bob_id, bob_email) = seed(&state, "wf-thread-bob", false).await;
    add_ws_member(&state, ws, bob_id, "member").await;
    let bob = login(&app, &bob_email, "password123").await;
    let help = seed_channel(&state, ws, owner_id, "help", false).await;
    join(&state, help, &[member_id, bob_id]).await;
    let workflow = built(
        &app,
        &member,
        ws,
        on_reaction(
            "Triage",
            help,
            "\u{1F6A8}",
            vec![
                json!({ "type": "add_reaction", "emoji": "\u{1F440}" }),
                json!({ "type": "reply_in_thread", "text": "On it, {{trigger.user}}" }),
                dm_back("Follow along: {{step.2.message_link}}"),
            ],
        ),
    )
    .await;

    let (status, message) = send(
        &app,
        "POST",
        &format!("/api/channels/{help}/messages"),
        Some(&bob),
        Some(json!({ "content": "the printer is on fire" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{message:?}");
    let message_id: Uuid = message["id"].as_str().unwrap().parse().unwrap();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/messages/{message_id}/reactions"),
        Some(&bob),
        Some(json!({ "emoji": "\u{1F6A8}" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    deliver(&state).await;
    runner::run_pending(&state).await.unwrap();

    let reactions = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT user_id, emoji FROM reactions WHERE message_id = $1 ORDER BY created_at",
    )
    .bind(message_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(
        reactions,
        [
            (bob_id, "\u{1F6A8}".to_string()),
            (member_id, "\u{1F440}".to_string())
        ]
    );
    let replies = sqlx::query_as::<_, (Uuid, String, Value)>(
        "SELECT id, content, metadata FROM messages WHERE thread_parent_id = $1",
    )
    .bind(message_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].1, format!("On it, @[Test User]({bob_id})"));
    assert_eq!(replies[0].2["bot"]["workflow_id"], workflow["id"]);
    let root: i32 = sqlx::query_scalar("SELECT reply_count FROM messages WHERE id = $1")
        .bind(message_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(root, 1);
    assert_eq!(
        dms(&state, ws, member_id, bob_id).await,
        [format!(
            "Follow along: http://localhost/app/{ws}/{help}/{}",
            replies[0].0
        )]
    );
    assert_eq!(
        runs(&app, &member, &workflow).await[0]["status"],
        "succeeded"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn channel_steps_are_held_to_what_the_owner_may_do_when_they_run(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-rights").await;
    let (dave_id, _) = seed(&state, "wf-rights-dave", false).await;
    add_ws_member(&state, ws, dave_id, "member").await;
    let (erin_id, _) = seed(&state, "wf-rights-erin", false).await;
    add_ws_member(&state, ws, erin_id, "member").await;
    let lobby = seed_channel(&state, ws, owner_id, "lobby", false).await;
    let secret = seed_channel(&state, ws, owner_id, "secret", true).await;
    join(&state, lobby, &[member_id]).await;
    join(&state, secret, &[member_id]).await;
    let group = state
        .group_repo
        .create(ws, "design", "Design", None, owner_id)
        .await
        .unwrap();
    for user in [dave_id, erin_id] {
        state.group_repo.add_member(group.id, user).await.unwrap();
    }
    let fixed = json!({ "kind": "channel", "channel_id": secret });
    let tidy = built(
        &app,
        &member,
        ws,
        shortcut(
            "Tidy",
            lobby,
            vec![
                json!({ "type": "set_topic", "channel": fixed, "topic": "Quiet, please" }),
                json!({ "type": "add_to_channel", "channel": fixed, "people": [{ "kind": "group", "group_id": group.id }] }),
            ],
        ),
    )
    .await;
    let close = shortcut(
        "Close",
        lobby,
        vec![json!({ "type": "archive_channel", "channel": fixed })],
    );
    let (status, body) = create(&app, &member, ws, close.clone()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "Step 1: you can't archive that channel");
    channel_role(&state, secret, member_id, "admin").await;
    let close = built(&app, &member, ws, close).await;

    start(&app, &member, &tidy, None).await;
    runner::run_pending(&state).await.unwrap();
    assert_eq!(runs(&app, &member, &tidy).await[0]["status"], "succeeded");
    let (_, _, topic) = channel_named(&state, ws, "secret").await.unwrap();
    assert_eq!(topic.as_deref(), Some("Quiet, please"));
    assert_eq!(
        members_of(&state, secret).await,
        sorted(vec![owner_id, member_id, dave_id, erin_id])
    );

    channel_role(&state, secret, member_id, "member").await;
    start(&app, &member, &close, None).await;
    runner::run_pending(&state).await.unwrap();
    let history = runs(&app, &member, &close).await;
    assert_eq!(
        history[0]["error"],
        "Step 1 (Archive a channel): Its creator can no longer archive that channel"
    );
    let archived: bool = sqlx::query_scalar("SELECT is_archived FROM channels WHERE id = $1")
        .bind(secret)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(!archived);

    sqlx::query("DELETE FROM channel_members WHERE channel_id = $1 AND user_id = $2")
        .bind(secret)
        .bind(member_id)
        .execute(&state.pool)
        .await
        .unwrap();
    start(&app, &member, &tidy, None).await;
    runner::run_pending(&state).await.unwrap();
    assert_eq!(
        runs(&app, &member, &tidy).await[0]["error"],
        "Step 1 (Set a channel topic): Its creator can no longer change that channel's topic"
    );

    join(&state, secret, &[member_id]).await;
    channel_role(&state, secret, member_id, "admin").await;
    start(&app, &member, &close, None).await;
    runner::run_pending(&state).await.unwrap();
    assert_eq!(runs(&app, &member, &close).await[0]["status"], "succeeded");
    let archived: bool = sqlx::query_scalar("SELECT is_archived FROM channels WHERE id = $1")
        .bind(secret)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(archived);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_waiting_run_finishes_on_the_version_it_started_on(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws, owner_id, _, member_id, member) =
        workspace_with_member(&app, &state, "wf-version").await;
    let (bob_id, bob_email) = seed(&state, "wf-version-bob", false).await;
    add_ws_member(&state, ws, bob_id, "member").await;
    let bob = login(&app, &bob_email, "password123").await;
    let requests = seed_channel(&state, ws, owner_id, "requests", false).await;
    join(&state, requests, &[member_id, bob_id]).await;
    let version = |text: &str| {
        shortcut(
            "Versions",
            requests,
            vec![
                ask(json!([question("what", "short_text")])),
                post_to(requests, text),
            ],
        )
    };
    let workflow = built(&app, &member, ws, version("first: {{form.what}}")).await;

    start(&app, &bob, &workflow, None).await;
    runner::run_pending(&state).await.unwrap();
    let first = open_form(&state, ws, member_id, bob_id).await;

    let mut edited = version("second: {{form.what}}");
    edited["definition"]["steps"]
        .as_array_mut()
        .unwrap()
        .insert(
            1,
            json!({ "type": "delay", "wait": { "kind": "for", "amount": 5, "unit": "minutes" } }),
        );
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/workflows/{}", workflow["id"].as_str().unwrap()),
        Some(&member),
        Some(edited),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["version"], 2);

    fill(&app, &bob, &first, json!({ "what": "a desk" })).await;
    runner::run_pending(&state).await.unwrap();
    assert_eq!(
        texts(bot_messages(&state, requests).await),
        ["first: a desk"]
    );

    start(&app, &bob, &workflow, None).await;
    runner::run_pending(&state).await.unwrap();
    let second = open_form(&state, ws, member_id, bob_id).await;
    fill(&app, &bob, &second, json!({ "what": "a chair" })).await;
    runner::run_pending(&state).await.unwrap();
    let history = runs(&app, &member, &workflow).await;
    assert_eq!(history[0]["status"], "waiting");
    assert_eq!(history[0]["workflow_version"], 2);
    assert_eq!(history[0]["waiting"]["label"], "Wait");
    assert_eq!(history[1]["status"], "succeeded");
    assert_eq!(history[1]["workflow_version"], 1);
    assert_eq!(history[1]["steps"][1]["label"], "Send a message");

    sqlx::query(
        "UPDATE workflow_runs SET resume_at = NOW() - INTERVAL '1 second' WHERE status = 'waiting'",
    )
    .execute(&state.pool)
    .await
    .unwrap();
    assert_eq!(runner::resume_delays(&state).await.unwrap(), 1);
    runner::run_pending(&state).await.unwrap();
    assert_eq!(
        texts(bot_messages(&state, requests).await),
        ["first: a desk", "second: a chair"]
    );
}
