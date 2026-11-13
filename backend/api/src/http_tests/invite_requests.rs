use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;

struct Team {
    ws: Uuid,
    owner_id: Uuid,
    owner: String,
    owner_email: String,
    admin_id: Uuid,
    admin: String,
    admin_email: String,
    member_id: Uuid,
    member: String,
}

async fn team(app: &axum::Router, state: &AppState, name: &str) -> Team {
    let (owner_id, owner_email, owner) = seed_and_login(app, state, "req-owner", false).await;
    let ws = seed_workspace(state, owner_id, name).await;
    let (admin_id, admin_email, admin) = seed_and_login(app, state, "req-admin", false).await;
    add_ws_member(state, ws, admin_id, "admin").await;
    let (member_id, _, member) = seed_and_login(app, state, "req-member", false).await;
    add_ws_member(state, ws, member_id, "member").await;
    sqlx::query("UPDATE users SET display_name = 'Rita Requester' WHERE id = $1")
        .bind(member_id)
        .execute(&state.pool)
        .await
        .expect("name the requester");
    Team {
        ws,
        owner_id,
        owner,
        owner_email,
        admin_id,
        admin,
        admin_email,
        member_id,
        member,
    }
}

async fn ask(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/invite-requests"),
        Some(token),
        Some(body),
    )
    .await
}

async fn waiting(app: &axum::Router, token: &str, ws: Uuid) -> (StatusCode, Value) {
    send(
        app,
        "GET",
        &format!("/api/workspaces/{ws}/invite-requests"),
        Some(token),
        None,
    )
    .await
}

async fn answer(
    app: &axum::Router,
    token: &str,
    ws: Uuid,
    request: &Value,
    verb: &str,
    body: Value,
) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!(
            "/api/workspaces/{ws}/invite-requests/{}/{verb}",
            request["id"].as_str().expect("request id")
        ),
        Some(token),
        Some(body),
    )
    .await
}

async fn invite(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> StatusCode {
    send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/invites"),
        Some(token),
        Some(body),
    )
    .await
    .0
}

async fn set_inviters(app: &axum::Router, token: &str, ws: Uuid, who: &str) {
    let (status, body) = send(
        app,
        "PATCH",
        &format!("/api/workspaces/{ws}"),
        Some(token),
        Some(json!({ "inviters": who })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

async fn notices(state: &AppState, user: Uuid, notice: &str) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT data FROM notifications WHERE user_id = $1 AND data->>'notice' = $2
          ORDER BY created_at",
    )
    .bind(user)
    .bind(notice)
    .fetch_all(&state.pool)
    .await
    .expect("notices")
}

async fn mail_to(state: &AppState, email: &str) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT subject, text_body FROM outbound_emails WHERE to_address = $1 ORDER BY created_at",
    )
    .bind(email)
    .fetch_all(&state.pool)
    .await
    .expect("outbound emails")
}

async fn trail(state: &AppState, resource: Uuid) -> Vec<(String, Option<Uuid>)> {
    sqlx::query_as(
        "SELECT action, user_id FROM audit_log WHERE resource_id = $1 ORDER BY created_at, action",
    )
    .bind(resource)
    .fetch_all(&state.pool)
    .await
    .expect("audit")
}

fn id_of(request: &Value) -> Uuid {
    request["id"]
        .as_str()
        .and_then(|id| id.parse().ok())
        .expect("request id")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_inviters_rule_decides_who_invites_and_who_asks(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state, "Who Invites").await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "req-guest", false).await;
    add_ws_member(&state, t.ws, guest_id, "guest").await;
    let newcomer = || json!({ "email": unique_email("req-new"), "role": "member" });

    assert_eq!(
        invite(&app, &t.member, t.ws, newcomer()).await,
        StatusCode::FORBIDDEN,
        "by default only admins invite"
    );
    assert_eq!(
        invite(&app, &t.admin, t.ws, newcomer()).await,
        StatusCode::OK
    );
    let (status, body) = ask(
        &app,
        &t.member,
        t.ws,
        json!({ "emails": [unique_email("req-new")], "role": "member" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a member asks instead: {body:?}");
    let (status, _) = ask(
        &app,
        &t.admin,
        t.ws,
        json!({ "emails": [unique_email("req-new")], "role": "member" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an admin has no one to ask"
    );

    set_inviters(&app, &t.owner, t.ws, "members").await;
    assert_eq!(
        invite(&app, &t.member, t.ws, newcomer()).await,
        StatusCode::OK,
        "members invite directly"
    );
    assert_eq!(
        invite(
            &app,
            &t.member,
            t.ws,
            json!({ "email": unique_email("req-new"), "role": "admin" })
        )
        .await,
        StatusCode::FORBIDDEN,
        "up to their own role"
    );
    let (status, _) = ask(
        &app,
        &t.member,
        t.ws,
        json!({ "emails": [unique_email("req-new")], "role": "member" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "and make no request when they may invite"
    );
    assert_eq!(
        invite(&app, &guest, t.ws, newcomer()).await,
        StatusCode::FORBIDDEN,
        "a guest never invites"
    );
    let (status, _) = ask(
        &app,
        &guest,
        t.ws,
        json!({ "emails": [unique_email("req-new")], "role": "member" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "nor asks");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_approved_request_invites_as_the_admin_and_names_the_requester(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state, "Asking Around").await;
    let vendor = seed_channel(&state, t.ws, t.owner_id, "vendor", false).await;
    let (other_id, _, other) = seed_and_login(&app, &state, "req-other", false).await;
    add_ws_member(&state, t.ws, other_id, "member").await;
    let first = unique_email("req-first");
    let second = unique_email("req-second");
    let third = unique_email("req-third");

    let (status, request) = ask(
        &app,
        &t.member,
        t.ws,
        json!({
            "emails": [first.to_uppercase(), second],
            "role": "member",
            "reason": "  They start on Monday  ",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{request:?}");
    assert_eq!(request["emails"], json!([first, second]));
    assert_eq!(request["reason"], "They start on Monday");
    assert_eq!(request["requester_name"], "Rita Requester");
    assert_eq!(request["status"], "pending");

    for admin in [t.owner_id, t.admin_id] {
        let heard = notices(&state, admin, "invite_requested").await;
        assert_eq!(heard.len(), 1, "every admin hears of it in Activity");
        assert_eq!(heard[0]["invite_request_id"], request["id"]);
        assert_eq!(heard[0]["sender_name"], "Rita Requester");
    }
    for address in [&t.owner_email, &t.admin_email] {
        let mail = mail_to(&state, address).await;
        assert_eq!(mail.len(), 1, "and by email");
        assert!(mail[0].0.contains("Rita Requester"), "{:?}", mail[0]);
        assert!(mail[0].1.contains(&first), "{:?}", mail[0]);
        assert!(mail[0].1.contains("They start on Monday"), "{:?}", mail[0]);
        assert!(mail[0].1.contains("?settings=members"), "{:?}", mail[0]);
    }

    let (_, listed) = waiting(&app, &t.admin, t.ws).await;
    assert_eq!(listed["data"].as_array().map(Vec::len), Some(1));
    let (_, mine) = waiting(&app, &t.member, t.ws).await;
    assert_eq!(
        mine["data"][0]["id"], request["id"],
        "the requester sees it"
    );
    let (_, theirs) = waiting(&app, &other, t.ws).await;
    assert_eq!(
        theirs["data"].as_array().map(Vec::len),
        Some(0),
        "another member does not"
    );

    let (status, approved) = answer(
        &app,
        &t.admin,
        t.ws,
        &request,
        "approve",
        json!({ "terms": {
            "emails": [first, third],
            "role": "guest",
            "guest_kind": "single",
            "channel_ids": [vendor],
        } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved:?}");
    assert_eq!(approved["request"]["status"], "approved");
    assert_eq!(approved["request"]["emails"], json!([first, third]));
    assert_eq!(approved["request"]["role"], "guest");
    let invites = approved["invites"].as_array().expect("invites");
    assert_eq!(invites.len(), 2);
    for created in invites {
        assert_eq!(
            created["created_by"],
            json!(t.admin_id),
            "sent as the admin"
        );
        assert_eq!(created["role"], "guest");
        assert_eq!(created["guest_kind"], "single");
        assert_eq!(created["channel_ids"], json!([vendor]));
    }

    let mail = mail_to(&state, &third).await;
    assert_eq!(mail.len(), 1, "the invite goes out");
    assert!(
        mail[0].1.contains("at Rita Requester's request"),
        "the email names the requester: {:?}",
        mail[0]
    );
    assert!(
        mail_to(&state, &second).await.is_empty(),
        "an address the admin took out gets nothing"
    );

    let told = notices(&state, t.member_id, "invite_request_approved").await;
    assert_eq!(told.len(), 1, "the requester is told");
    assert_eq!(told[0]["invite_emails"], json!([first, third]));

    let (status, _) = answer(&app, &t.owner, t.ws, &request, "approve", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "a request is answered once");
    let (_, listed) = waiting(&app, &t.admin, t.ws).await;
    assert_eq!(listed["data"].as_array().map(Vec::len), Some(0));

    let actions: Vec<String> = trail(&state, id_of(&request))
        .await
        .into_iter()
        .map(|(action, _)| action)
        .collect();
    assert_eq!(
        actions,
        vec!["invite_request.created", "invite_request.approved"]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_declined_request_carries_the_note_back(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state, "Declines").await;
    let address = unique_email("req-declined");
    let (_, request) = ask(
        &app,
        &t.member,
        t.ws,
        json!({ "emails": [address], "role": "member" }),
    )
    .await;

    let (status, declined) = answer(
        &app,
        &t.admin,
        t.ws,
        &request,
        "decline",
        json!({ "note": "We hire through HR" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{declined:?}");
    assert_eq!(declined["status"], "declined");
    assert_eq!(declined["note"], "We hire through HR");
    assert_eq!(declined["decided_by"], json!(t.admin_id));

    let told = notices(&state, t.member_id, "invite_request_declined").await;
    assert_eq!(told.len(), 1);
    assert_eq!(told[0]["note"], "We hire through HR");
    assert!(
        mail_to(&state, &address).await.is_empty(),
        "nobody is invited"
    );
    let (status, _) = answer(&app, &t.owner, t.ws, &request, "approve", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, mine) = waiting(&app, &t.member, t.ws).await;
    assert_eq!(mine["data"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        trail(&state, id_of(&request)).await.last(),
        Some(&("invite_request.declined".to_string(), Some(t.admin_id)))
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_request_nobody_answers_expires_after_thirty_days(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state, "Expiring Requests").await;
    let (_, old) = ask(
        &app,
        &t.member,
        t.ws,
        json!({ "emails": [unique_email("req-old")], "role": "member" }),
    )
    .await;
    let (status, _) = ask(
        &app,
        &t.member,
        t.ws,
        json!({ "emails": [unique_email("req-fresh")], "role": "member" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    sqlx::query("UPDATE invite_requests SET created_at = NOW() - interval '31 days' WHERE id = $1")
        .bind(id_of(&old))
        .execute(&state.pool)
        .await
        .expect("age the request");

    crate::invite_requests::expiry::run(&state).await;

    let statuses: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, status::text FROM invite_requests WHERE workspace_id = $1")
            .bind(t.ws)
            .fetch_all(&state.pool)
            .await
            .expect("statuses");
    for (id, status) in statuses {
        let expected = if id == id_of(&old) {
            "expired"
        } else {
            "pending"
        };
        assert_eq!(status, expected);
    }
    assert_eq!(
        trail(&state, id_of(&old)).await.last(),
        Some(&("invite_request.expired".to_string(), None)),
        "the loop expires it, with no actor"
    );
    assert_eq!(
        notices(&state, t.member_id, "invite_request_expired")
            .await
            .len(),
        1
    );
    let (status, _) = answer(&app, &t.admin, t.ws, &old, "approve", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);

    crate::invite_requests::expiry::run(&state).await;
    assert_eq!(
        notices(&state, t.member_id, "invite_request_expired")
            .await
            .len(),
        1,
        "once"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_same_address_twice_or_a_member_is_refused(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state, "Duplicates").await;
    let (other_id, _, other) = seed_and_login(&app, &state, "req-dup-other", false).await;
    add_ws_member(&state, t.ws, other_id, "member").await;
    let address = unique_email("req-dup");

    let (status, body) = ask(
        &app,
        &t.member,
        t.ws,
        json!({ "emails": [address, address.to_uppercase()], "role": "member" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    let (status, body) = ask(
        &app,
        &t.member,
        t.ws,
        json!({ "emails": [t.admin_email], "role": "member" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    assert!(body["error"]
        .as_str()
        .is_some_and(|e| e.contains("already in this workspace")));

    let (status, _) = ask(
        &app,
        &t.member,
        t.ws,
        json!({ "emails": [address], "role": "member" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = ask(
        &app,
        &other,
        t.ws,
        json!({ "emails": [unique_email("req-dup-new"), address], "role": "member" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    assert!(body["error"].as_str().is_some_and(|e| e.contains(&address)));

    let (status, _) = ask(
        &app,
        &t.member,
        t.ws,
        json!({ "emails": [], "role": "member" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "at least one address"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_members_ask_and_only_admins_answer(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state, "Who Answers").await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "req-auth-guest", false).await;
    add_ws_member(&state, t.ws, guest_id, "guest").await;
    let (_, _, outsider) = seed_and_login(&app, &state, "req-auth-out", false).await;
    let (elsewhere_owner, _, elsewhere) =
        seed_and_login(&app, &state, "req-auth-else", false).await;
    let elsewhere_ws = seed_workspace(&state, elsewhere_owner, "Elsewhere").await;
    let one = || json!({ "emails": [unique_email("req-auth")], "role": "member" });

    for (who, token) in [("a guest", &guest), ("an outsider", &outsider)] {
        assert_eq!(
            ask(&app, token, t.ws, one()).await.0,
            StatusCode::FORBIDDEN,
            "{who} asks"
        );
        assert_eq!(
            waiting(&app, token, t.ws).await.0,
            StatusCode::FORBIDDEN,
            "{who} lists"
        );
    }
    let (status, _) = ask(
        &app,
        &t.member,
        t.ws,
        json!({ "emails": [unique_email("req-auth")], "role": "admin" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a request is for members or guests"
    );

    let (_, request) = ask(&app, &t.member, t.ws, one()).await;
    for (who, token) in [("the requester", &t.member), ("a guest", &guest)] {
        for verb in ["approve", "decline"] {
            assert_eq!(
                answer(&app, token, t.ws, &request, verb, json!({})).await.0,
                StatusCode::FORBIDDEN,
                "{who} cannot {verb}"
            );
        }
    }
    assert_eq!(
        answer(
            &app,
            &elsewhere,
            elsewhere_ws,
            &request,
            "approve",
            json!({})
        )
        .await
        .0,
        StatusCode::NOT_FOUND,
        "a request belongs to its workspace"
    );
    let (status, _) = answer(&app, &t.admin, t.ws, &request, "approve", json!({})).await;
    assert_eq!(status, StatusCode::OK);
}
