use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;

struct Team {
    ws: Uuid,
    admin: String,
    ana_id: Uuid,
    ana: String,
    ben_id: Uuid,
    ben: String,
    cleo_id: Uuid,
    cleo: String,
}

async fn team(app: &axum::Router, state: &AppState) -> Team {
    let (owner_id, _, _) = seed_and_login(app, state, "pf-owner", false).await;
    let ws = seed_workspace(state, owner_id, "Profile Fields").await;
    let (admin_id, _, admin) = seed_and_login(app, state, "pf-admin", false).await;
    add_ws_member(state, ws, admin_id, "admin").await;
    let (ana_id, _, ana) = seed_and_login(app, state, "pf-ana", false).await;
    add_ws_member(state, ws, ana_id, "member").await;
    let (ben_id, _, ben) = seed_and_login(app, state, "pf-ben", false).await;
    add_ws_member(state, ws, ben_id, "member").await;
    let (cleo_id, _, cleo) = seed_and_login(app, state, "pf-cleo", false).await;
    add_ws_member(state, ws, cleo_id, "member").await;
    for (id, name) in [
        (ana_id, "Ana Admir"),
        (ben_id, "Ben Brook"),
        (cleo_id, "Cleo Cole"),
    ] {
        sqlx::query("UPDATE users SET display_name = $2 WHERE id = $1")
            .bind(id)
            .bind(name)
            .execute(&state.pool)
            .await
            .expect("name");
    }
    Team {
        ws,
        admin,
        ana_id,
        ana,
        ben_id,
        ben,
        cleo_id,
        cleo,
    }
}

async fn create(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/profile-fields"),
        Some(token),
        Some(body),
    )
    .await
}

async fn created(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> Value {
    let (status, field) = create(app, token, ws, body).await;
    assert_eq!(status, StatusCode::OK, "{field:?}");
    field
}

async fn patch(
    app: &axum::Router,
    token: &str,
    ws: Uuid,
    field: &Value,
    body: Value,
) -> (StatusCode, Value) {
    send(
        app,
        "PATCH",
        &format!("/api/workspaces/{ws}/profile-fields/{}", id(field)),
        Some(token),
        Some(body),
    )
    .await
}

async fn set(
    app: &axum::Router,
    token: &str,
    ws: Uuid,
    user: Uuid,
    values: Value,
) -> (StatusCode, Value) {
    send(
        app,
        "PUT",
        &format!("/api/workspaces/{ws}/members/{user}/profile-fields"),
        Some(token),
        Some(json!({ "values": values })),
    )
    .await
}

async fn fields(app: &axum::Router, token: &str, ws: Uuid) -> (StatusCode, Value) {
    send(
        app,
        "GET",
        &format!("/api/workspaces/{ws}/profile-fields"),
        Some(token),
        None,
    )
    .await
}

async fn people(app: &axum::Router, token: &str, ws: Uuid, query: &str) -> (StatusCode, Value) {
    send(
        app,
        "GET",
        &format!("/api/workspaces/{ws}/members{query}"),
        Some(token),
        None,
    )
    .await
}

fn id(value: &Value) -> &str {
    value["id"].as_str().expect("id")
}

fn option(field: &Value, label: &str) -> String {
    field["options"]
        .as_array()
        .expect("options")
        .iter()
        .find(|o| o["label"] == label)
        .and_then(|o| o["id"].as_str())
        .expect("option")
        .to_string()
}

fn refusal(body: &Value) -> String {
    body.to_string()
}

fn names(body: &Value) -> Vec<String> {
    let mut names: Vec<String> = body["data"]
        .as_array()
        .expect("members")
        .iter()
        .map(|m| m["display_name"].as_str().unwrap_or_default().to_string())
        .collect();
    names.sort();
    names
}

fn department() -> Value {
    json!({
        "label": "Department",
        "kind": "options",
        "options": [{ "label": "Finance" }, { "label": "Support" }],
        "on_card": true,
    })
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_admins_define_fields_and_every_member_reads_them(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "pf-guest", false).await;
    add_ws_member(&state, t.ws, guest_id, "guest").await;
    let (_, _, outsider) = seed_and_login(&app, &state, "pf-out", false).await;

    let (status, _) = create(&app, &t.ana, t.ws, department()).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a member does not define fields"
    );
    let dept = created(&app, &t.admin, t.ws, department()).await;
    let manager = created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Manager", "kind": "person", "hint": "Who you report to", "on_card": true }),
    )
    .await;
    created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Office", "kind": "text" }),
    )
    .await;

    for token in [&t.ana, &guest] {
        let (status, list) = fields(&app, token, t.ws).await;
        assert_eq!(status, StatusCode::OK, "{list:?}");
        let labels: Vec<&str> = list["data"]
            .as_array()
            .expect("fields")
            .iter()
            .map(|f| f["label"].as_str().expect("label"))
            .collect();
        assert_eq!(labels, ["Department", "Manager", "Office"]);
    }
    let (status, _) = fields(&app, &outsider, t.ws).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "someone from outside reads nothing"
    );

    let (status, _) = patch(&app, &t.ana, t.ws, &dept, json!({ "label": "Team" })).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{}/profile-fields/{}", t.ws, id(&dept)),
        Some(&t.ana),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = create(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "department", "kind": "text" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "labels are unique whatever the case: {body:?}"
    );
    let (status, body) = create(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Level", "kind": "options" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    let (status, body) = create(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Desk", "kind": "text", "options": [{ "label": "A" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    let (status, body) = create(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Started", "kind": "date", "on_card": true }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "two fields at most on the card: {body:?}"
    );

    let (_, list) = fields(&app, &t.admin, t.ws).await;
    let mut ids: Vec<String> = list["data"]
        .as_array()
        .expect("fields")
        .iter()
        .map(|f| id(f).to_string())
        .collect();
    ids.reverse();
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/workspaces/{}/profile-fields/order", t.ws),
        Some(&t.admin),
        Some(json!({ "field_ids": &ids[..2] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "every field is listed: {body:?}"
    );
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/workspaces/{}/profile-fields/order", t.ws),
        Some(&t.admin),
        Some(json!({ "field_ids": ids })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["data"][0]["label"], "Office");
    assert_eq!(body["data"][2]["label"], "Department");

    let (status, body) = set(
        &app,
        &t.ben,
        t.ws,
        t.ben_id,
        json!({ id(&dept): option(&dept, "Support"), id(&manager): t.ana_id }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, renamed) = patch(
        &app,
        &t.admin,
        t.ws,
        &dept,
        json!({ "options": [
            { "id": option(&dept, "Finance"), "label": "Finance" },
            { "id": option(&dept, "Support"), "label": "Customer support" },
        ] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{renamed:?}");
    assert_eq!(
        renamed["value_count"], 1,
        "a renamed option keeps its values"
    );
    let (status, trimmed) = patch(
        &app,
        &t.admin,
        t.ws,
        &dept,
        json!({ "options": [{ "id": option(&dept, "Finance"), "label": "Finance" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{trimmed:?}");
    assert_eq!(
        trimmed["value_count"], 0,
        "a removed option takes its values with it"
    );

    let (_, list) = fields(&app, &t.admin, t.ws).await;
    let held = list["data"]
        .as_array()
        .expect("fields")
        .iter()
        .find(|f| f["label"] == "Manager")
        .expect("manager")["value_count"]
        .clone();
    assert_eq!(
        held, 1,
        "the delete confirmation can say how many values go"
    );
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{}/profile-fields/{}", t.ws, id(&manager)),
        Some(&t.admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM profile_field_values WHERE field_id = $1")
            .bind(Uuid::parse_str(id(&manager)).expect("uuid"))
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(left, 0, "deleting a field deletes its values");

    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE resource_id = $1 ORDER BY created_at",
    )
    .bind(Uuid::parse_str(id(&manager)).expect("uuid"))
    .fetch_all(&state.pool)
    .await
    .expect("audit");
    assert_eq!(actions, ["profile_field.created", "profile_field.deleted"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_workspace_has_at_most_fifty_fields(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    for n in 0..50 {
        created(
            &app,
            &t.admin,
            t.ws,
            json!({ "label": format!("Field {n}"), "kind": "text" }),
        )
        .await;
    }
    let (status, body) = create(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "One more", "kind": "text" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_value_is_checked_against_its_field(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let (stranger_id, _) = seed(&state, "pf-stranger", false).await;
    let dept = created(&app, &t.admin, t.ws, department()).await;
    let manager = created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Manager", "kind": "person" }),
    )
    .await;
    let office = created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Office", "kind": "text" }),
    )
    .await;
    let site = created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Website", "kind": "link" }),
    )
    .await;
    let start = created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Start date", "kind": "date" }),
    )
    .await;
    let number = created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Employee number", "kind": "text", "admins_only": true }),
    )
    .await;

    let refused = [
        (json!({ id(&dept): Uuid::new_v4() }), "Department"),
        (json!({ id(&dept): "Finance" }), "Department"),
        (json!({ id(&manager): stranger_id }), "Manager"),
        (json!({ id(&manager): "Ana" }), "Manager"),
        (json!({ id(&office): "x".repeat(201) }), "Office"),
        (json!({ id(&office): "two\nlines" }), "Office"),
        (json!({ id(&site): "javascript:alert(1)" }), "Website"),
        (json!({ id(&start): "next tuesday" }), "Start date"),
        (json!({ id(&start): "2026-02-30" }), "Start date"),
    ];
    for (values, label) in refused {
        let (status, body) = set(&app, &t.ben, t.ws, t.ben_id, values.clone()).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{values}: {body:?}"
        );
        assert!(
            refusal(&body).contains(label),
            "{values} names {label}: {body:?}"
        );
    }
    let (status, _) = set(
        &app,
        &t.ben,
        t.ws,
        t.ben_id,
        json!({ Uuid::new_v4().to_string(): "x" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "an unknown field is refused"
    );

    let (status, body) = set(&app, &t.ben, t.ws, t.ben_id, json!({ id(&number): "E-7" })).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");
    assert!(refusal(&body).contains("Employee number"), "{body:?}");
    let (status, _) = set(
        &app,
        &t.ana,
        t.ws,
        t.ben_id,
        json!({ id(&office): "Belgrade" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a member edits only their own fields"
    );

    let (status, body) = set(
        &app,
        &t.admin,
        t.ws,
        t.ben_id,
        json!({ id(&number): "E-7" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = set(
        &app,
        &t.ben,
        t.ws,
        t.ben_id,
        json!({
            id(&dept): option(&dept, "Finance"),
            id(&manager): t.ana_id,
            id(&office): "  Belgrade  ",
            id(&site): "https://example.com/ben",
            id(&start): "2026-02-03",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let values = &body["values"];
    assert_eq!(values[id(&office)], "Belgrade");
    assert_eq!(
        values[id(&number)],
        "E-7",
        "a member's write leaves the admin's field alone"
    );
    assert_eq!(values[id(&manager)], t.ana_id.to_string());

    let (status, body) = set(
        &app,
        &t.ben,
        t.ws,
        t.ben_id,
        json!({ id(&office): null, id(&site): "" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(
        body["values"].get(id(&office)).is_none(),
        "null clears a value"
    );
    assert!(
        body["values"].get(id(&site)).is_none(),
        "a blank clears a value"
    );

    let (_, list) = people(&app, &t.cleo, t.ws, "").await;
    let ben = list["data"]
        .as_array()
        .expect("members")
        .iter()
        .find(|m| m["user_id"] == t.ben_id.to_string())
        .expect("ben");
    assert_eq!(
        ben["profile_fields"][id(&start)],
        "2026-02-03",
        "everyone sees the values"
    );
    assert_eq!(
        ben["profile_fields"][id(&number)],
        "E-7",
        "an admins-only value is still shown"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_directory_filters_by_option_person_and_text(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let dept = created(&app, &t.admin, t.ws, department()).await;
    let manager = created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Manager", "kind": "person" }),
    )
    .await;
    let office = created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Office", "kind": "text" }),
    )
    .await;
    let finance = option(&dept, "Finance");
    let support = option(&dept, "Support");

    for (token, user, values) in [
        (
            &t.ana,
            t.ana_id,
            json!({ id(&dept): finance, id(&office): "Belgrade" }),
        ),
        (
            &t.ben,
            t.ben_id,
            json!({ id(&dept): finance, id(&manager): t.ana_id, id(&office): "Novi Sad" }),
        ),
        (
            &t.cleo,
            t.cleo_id,
            json!({ id(&dept): support, id(&manager): t.ana_id }),
        ),
    ] {
        let (status, body) = set(&app, token, t.ws, user, values).await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }

    let (status, body) = people(
        &app,
        &t.ben,
        t.ws,
        &format!("?filter={}:{finance}", id(&dept)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(names(&body), ["Ana Admir", "Ben Brook"]);

    let (_, body) = people(
        &app,
        &t.ben,
        t.ws,
        &format!("?filter={}:{}", id(&manager), t.ana_id),
    )
    .await;
    assert_eq!(
        names(&body),
        ["Ben Brook", "Cleo Cole"],
        "Manager = Ana lists her reports"
    );

    let (_, body) = people(
        &app,
        &t.ben,
        t.ws,
        &format!(
            "?filter={}:{finance},{}:{}",
            id(&dept),
            id(&manager),
            t.ana_id
        ),
    )
    .await;
    assert_eq!(names(&body), ["Ben Brook"], "filters narrow together");

    let (_, body) = people(&app, &t.ben, t.ws, "?q=belgr").await;
    assert_eq!(
        names(&body),
        ["Ana Admir"],
        "the search box matches text values"
    );
    let (_, body) = people(&app, &t.ben, t.ws, "?q=cleo").await;
    assert_eq!(names(&body), ["Cleo Cole"], "and still matches names");

    let (status, body) = people(
        &app,
        &t.ben,
        t.ws,
        &format!("?filter={}:{}", id(&office), Uuid::new_v4()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a text field is searched, not filtered: {body:?}"
    );
    let (status, _) = people(&app, &t.ben, t.ws, "?filter=nonsense").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_bot_with_users_read_gets_fields_and_values(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "pf-bot-owner", false).await;
    add_ws_member(&state, t.ws, owner_id, "owner").await;
    let office = created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Office", "kind": "text" }),
    )
    .await;
    set(
        &app,
        &t.ana,
        t.ws,
        t.ana_id,
        json!({ id(&office): "Belgrade" }),
    )
    .await;

    let mut tokens = Vec::new();
    for scopes in [json!(["users:read"]), json!(["chat:write"])] {
        let (status, bot) = send(
            &app,
            "POST",
            &format!("/api/workspaces/{}/bot-apps", t.ws),
            Some(&owner),
            Some(json!({ "name": format!("Directory {}", tokens.len()), "description": "Looks people up", "scopes": scopes })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{bot:?}");
        tokens.push(bot["token"].as_str().expect("token").to_string());
    }

    let (status, body) = fields(&app, &tokens[0], t.ws).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["data"][0]["label"], "Office");
    let (status, body) = people(&app, &tokens[0], t.ws, "").await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let ana = body["data"]
        .as_array()
        .expect("members")
        .iter()
        .find(|m| m["user_id"] == t.ana_id.to_string())
        .expect("ana");
    assert_eq!(
        ana["profile_fields"][id(&office)],
        "Belgrade",
        "values are keyed by field id"
    );

    let (status, _) = fields(&app, &tokens[1], t.ws).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "without users:read a bot sees no fields"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn export_carries_the_values_and_erase_and_leaving_remove_them(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let dept = created(&app, &t.admin, t.ws, department()).await;
    let office = created(
        &app,
        &t.admin,
        t.ws,
        json!({ "label": "Office", "kind": "text" }),
    )
    .await;
    set(
        &app,
        &t.ben,
        t.ws,
        t.ben_id,
        json!({ id(&dept): option(&dept, "Finance"), id(&office): "Belgrade" }),
    )
    .await;
    set(
        &app,
        &t.cleo,
        t.ws,
        t.cleo_id,
        json!({ id(&office): "Niš" }),
    )
    .await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/users/{}/exports", t.ben_id),
        Some(&t.ben),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let job = state
        .export_repo
        .claim_next()
        .await
        .expect("claim")
        .expect("a pending job");
    let job_id = job.id;
    crate::export::job::run_for_test(&state, job)
        .await
        .expect("export runs");
    let manifest = state
        .export_repo
        .find(job_id)
        .await
        .expect("find")
        .expect("job")
        .manifest
        .expect("manifest");
    assert_eq!(manifest["files"]["profile_fields.jsonl"]["rows"], 2);
    let owned = state
        .profile_field_repo
        .owned_by(t.ben_id)
        .await
        .expect("owned values");
    let named: Vec<_> = owned
        .iter()
        .map(|v| (v.label.as_str(), v.option_label.as_deref()))
        .collect();
    assert_eq!(
        named,
        [("Department", Some("Finance")), ("Office", None)],
        "an exported option carries its label, not only its id"
    );

    let (_, _, instance_admin) = seed_and_login(&app, &state, "pf-instance", true).await;
    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/admin/users/{}/data", t.ben_id),
        Some(&instance_admin),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["account"]["profile_fields"], 2);

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{}/members/{}", t.ws, t.cleo_id),
        Some(&t.admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let kept: i64 =
        sqlx::query_scalar("SELECT count(*) FROM profile_field_values WHERE user_id = ANY($1)")
            .bind(vec![t.ben_id, t.cleo_id])
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(
        kept, 0,
        "an erased person and a person who left keep no values"
    );
}
