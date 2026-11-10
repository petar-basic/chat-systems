use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

async fn owner_id(pool: &PgPool, ws: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT owner_id FROM workspaces WHERE id = $1")
        .bind(ws)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn role(pool: &PgPool, ws: Uuid, user: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT role::text FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(ws)
    .bind(user)
    .fetch_optional(pool)
    .await
    .unwrap()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_invite_never_carries_a_role_above_the_person_who_made_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, owner_token) = seed_and_login(&app, &state, "inv-role-owner", false).await;
    let (admin, _, admin_token) = seed_and_login(&app, &state, "inv-role-admin", false).await;
    let ws = seed_workspace(&state, owner, "Invite Roles").await;
    add_ws_member(&state, ws, admin, "admin").await;
    let path = format!("/api/workspaces/{ws}/invites");

    for (token, role, expected) in [
        (&admin_token, "owner", StatusCode::FORBIDDEN),
        (&admin_token, "admin", StatusCode::OK),
        (&admin_token, "member", StatusCode::OK),
        (&admin_token, "guest", StatusCode::OK),
        (&owner_token, "owner", StatusCode::OK),
        (&owner_token, "admin", StatusCode::OK),
    ] {
        let (status, body) = send(
            &app,
            "POST",
            &path,
            Some(token),
            Some(json!({ "role": role, "max_uses": 1 })),
        )
        .await;
        let who = if token == &admin_token {
            "admin"
        } else {
            "owner"
        };
        assert_eq!(status, expected, "{who} inviting as {role}: {body:?}");
    }

    let (status, body) = send(
        &app,
        "POST",
        &path,
        Some(&admin_token),
        Some(json!({ "email": unique_email("inv-role-new"), "role": "owner" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an email invite is held to the same rule: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn opening_an_invite_keeps_the_role_you_already_have(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, owner_token) = seed_and_login(&app, &state, "keep-owner", false).await;
    let ws = seed_workspace(&state, owner, "Keep Roles").await;
    let mut people = Vec::new();
    for (prefix, role) in [
        ("keep-owner2", "owner"),
        ("keep-admin", "admin"),
        ("keep-guest", "guest"),
    ] {
        let (id, _, token) = seed_and_login(&app, &state, prefix, false).await;
        add_ws_member(&state, ws, id, role).await;
        people.push((id, token, role));
    }
    let (newcomer, _, newcomer_token) = seed_and_login(&app, &state, "keep-new", false).await;

    let (status, invite) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/invites"),
        Some(&owner_token),
        Some(json!({ "role": "member", "max_uses": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{invite:?}");
    let accept = format!("/api/invites/{}/accept", invite["token"].as_str().unwrap());

    for (id, token, kept) in &people {
        let (status, body) = send(&app, "POST", &accept, Some(token), None).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{kept} opening a member link: {body:?}"
        );
        assert_eq!(
            body["role"], *kept,
            "the answer is the membership they have"
        );
        assert_eq!(role(&state.pool, ws, *id).await.as_deref(), Some(*kept));
    }

    let (status, body) = send(&app, "POST", &accept, Some(&newcomer_token), None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "people already in the workspace did not use up the link: {body:?}"
    );
    assert_eq!(body["role"], "member");
    assert_eq!(
        role(&state.pool, ws, newcomer).await.as_deref(),
        Some("member")
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_owner_steps_down_or_leaves_only_while_another_owner_remains(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ana, _, ana_token) = seed_and_login(&app, &state, "own-ana", false).await;
    let (bo, _, bo_token) = seed_and_login(&app, &state, "own-bo", false).await;
    let ws = seed_workspace(&state, ana, "Two Owners").await;
    add_ws_member(&state, ws, bo, "member").await;
    let role_of = |user: Uuid| format!("/api/workspaces/{ws}/members/{user}/role");
    let member = |user: Uuid| format!("/api/workspaces/{ws}/members/{user}");

    let (status, body) = send(
        &app,
        "PATCH",
        &role_of(ana),
        Some(&ana_token),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the only owner stays: {body:?}"
    );
    assert_eq!(body["error"], "Make someone else an owner first");
    let (status, body) = send(&app, "DELETE", &member(ana), Some(&ana_token), None).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the only owner can't leave: {body:?}"
    );
    assert_eq!(body["error"], "Make someone else an owner first");

    let (status, body) = send(
        &app,
        "PATCH",
        &role_of(bo),
        Some(&ana_token),
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = send(
        &app,
        "PATCH",
        &role_of(bo),
        Some(&ana_token),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an owner is never demoted by another: {body:?}"
    );
    let (status, body) = send(
        &app,
        "PATCH",
        &role_of(ana),
        Some(&ana_token),
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "stepping down goes to admin: {body:?}"
    );

    let (status, body) = send(
        &app,
        "PATCH",
        &role_of(ana),
        Some(&ana_token),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["role"], "admin");
    assert_eq!(
        owner_id(&state.pool, ws).await,
        bo,
        "the workspace names an owner it still has"
    );
    let (status, body) = send(
        &app,
        "PATCH",
        &role_of(bo),
        Some(&bo_token),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");

    let (status, body) = send(
        &app,
        "PATCH",
        &role_of(ana),
        Some(&bo_token),
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = send(&app, "DELETE", &member(bo), Some(&bo_token), None).await;
    assert_eq!(status, StatusCode::OK, "one of two owners leaves: {body:?}");
    assert_eq!(role(&state.pool, ws, bo).await, None);
    assert_eq!(owner_id(&state.pool, ws).await, ana);

    let (cy, _, _) = seed_and_login(&app, &state, "own-cy", false).await;
    add_ws_member(&state, ws, cy, "owner").await;
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = $1")
        .bind(cy)
        .execute(&state.pool)
        .await
        .unwrap();
    let (status, body) = send(&app, "DELETE", &member(ana), Some(&ana_token), None).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a suspended owner is nobody to hand over to: {body:?}"
    );

    let audited: Vec<Value> = sqlx::query_scalar(
        "SELECT details FROM audit_log WHERE action = 'workspace.role_changed' AND resource_id = $1 AND user_id = $1",
    )
    .bind(ana)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(audited, vec![json!({ "from": "owner", "to": "admin" })]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_instance_admin_gives_a_workspace_whose_owner_is_gone_a_new_one(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, _, root) = seed_and_login(&app, &state, "appoint-root", true).await;
    let (owner, _, _) = seed_and_login(&app, &state, "appoint-owner", false).await;
    let (maya, _, maya_token) = seed_and_login(&app, &state, "appoint-maya", false).await;
    let (away, _, _) = seed_and_login(&app, &state, "appoint-away", false).await;
    let (outsider, _, _) = seed_and_login(&app, &state, "appoint-out", false).await;
    let ws = seed_workspace(&state, owner, "Orphaned").await;
    add_ws_member(&state, ws, maya, "member").await;
    add_ws_member(&state, ws, away, "member").await;
    for user in [owner, away] {
        let (status, body) = send(
            &app,
            "POST",
            &format!("/api/admin/users/{user}/suspend"),
            Some(&root),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }

    let (status, list) = send(&app, "GET", "/api/admin/workspaces", Some(&root), None).await;
    assert_eq!(status, StatusCode::OK, "{list:?}");
    let listed = list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["id"] == ws.to_string())
        .unwrap()
        .clone();
    assert_eq!(listed["owners"][0]["user_id"], owner.to_string());
    assert_eq!(
        listed["owners"][0]["status"], "suspended",
        "the list shows the owner is gone"
    );

    let (status, people) = send(
        &app,
        "GET",
        &format!("/api/admin/workspaces/{ws}/members"),
        Some(&root),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{people:?}");
    assert_eq!(people["data"].as_array().unwrap().len(), 3);

    let appoint = format!("/api/admin/workspaces/{ws}/owners");
    let (status, _) = send(
        &app,
        "POST",
        &appoint,
        Some(&maya_token),
        Some(json!({ "user_id": maya })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "only an instance admin");
    let (status, _) = send(
        &app,
        "POST",
        &appoint,
        Some(&root),
        Some(json!({ "user_id": outsider })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "somebody outside the workspace"
    );
    let (status, _) = send(
        &app,
        "POST",
        &appoint,
        Some(&root),
        Some(json!({ "user_id": away })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a suspended account");

    let (status, body) = send(
        &app,
        "POST",
        &appoint,
        Some(&root),
        Some(json!({ "user_id": maya })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["role"], "owner");
    assert_eq!(
        owner_id(&state.pool, ws).await,
        maya,
        "the gone owner is no longer the one the workspace names"
    );

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}/retention"),
        Some(&maya_token),
        Some(json!({ "message_days": 90 })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the new owner runs data retention: {body:?}"
    );

    let audited: Vec<Value> = sqlx::query_scalar(
        "SELECT details FROM audit_log WHERE action = 'workspace.role_changed' AND workspace_id = $1 AND resource_id = $2",
    )
    .bind(ws)
    .bind(maya)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(
        audited,
        vec![json!({ "from": "member", "to": "owner", "instance_admin": true })]
    );
}
