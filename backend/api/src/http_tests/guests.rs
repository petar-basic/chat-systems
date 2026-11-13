use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;

async fn invite(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/invites"),
        Some(token),
        Some(body),
    )
    .await
}

async fn register(app: &axum::Router, created: &Value, name: &str) -> (Uuid, String) {
    let url = created["invite_url"].as_str().expect("invite url");
    let link = url.rsplit('/').next().expect("token");
    let (status, body) = send(
        app,
        "POST",
        "/api/auth/complete-registration",
        None,
        Some(json!({ "token": link, "password": "longenough123", "display_name": name })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    (
        body["user"]["id"]
            .as_str()
            .expect("id")
            .parse()
            .expect("uuid"),
        body["access_token"].as_str().expect("token").to_string(),
    )
}

async fn channel_names(app: &axum::Router, token: &str, ws: Uuid) -> Vec<String> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/workspaces/{ws}/channels"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let mut names: Vec<String> = body["data"]
        .as_array()
        .expect("channels")
        .iter()
        .filter(|c| c["channel_type"] == "public" || c["channel_type"] == "private")
        .map(|c| c["name"].as_str().expect("name").to_string())
        .collect();
    names.sort();
    names
}

async fn add(app: &axum::Router, token: &str, ch: Uuid, user: Uuid) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/channels/{ch}/members"),
        Some(token),
        Some(json!({ "user_id": user })),
    )
    .await
}

async fn seat(
    app: &axum::Router,
    token: &str,
    ws: Uuid,
    user: Uuid,
    body: Value,
) -> (StatusCode, Value) {
    send(
        app,
        "PATCH",
        &format!("/api/workspaces/{ws}/members/{user}/guest"),
        Some(token),
        Some(body),
    )
    .await
}

async fn convert(app: &axum::Router, token: &str, group: Uuid) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/conversations/{group}/convert"),
        Some(token),
        Some(json!({ "name": "studio-crew" })),
    )
    .await
}

async fn member_row(app: &axum::Router, token: &str, ws: Uuid, user: Uuid) -> Option<Value> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/workspaces/{ws}/members"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"]
        .as_array()
        .expect("members")
        .iter()
        .find(|m| m["user_id"] == json!(user))
        .cloned()
}

async fn make_guest(
    state: &AppState,
    ws: Uuid,
    user: Uuid,
    kind: &str,
    until: Option<chrono::DateTime<Utc>>,
) {
    add_ws_member(state, ws, user, "guest").await;
    sqlx::query(
        "UPDATE workspace_members SET guest_kind = $3::guest_kind, guest_until = $4
          WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(ws)
    .bind(user)
    .bind(kind)
    .bind(until)
    .execute(&state.pool)
    .await
    .expect("guest terms");
}

async fn join(state: &AppState, ch: Uuid, user: Uuid) {
    state
        .workspace_service
        .repo
        .add_channel_member(ch, user, &crate::workspace::models::ChannelRole::Member)
        .await
        .expect("join");
}

async fn ending_notices(state: &AppState, ws: Uuid) -> Vec<Uuid> {
    let mut to: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM notifications
          WHERE workspace_id = $1 AND data->>'notice' = 'guest_ending'",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await
    .expect("notices");
    to.sort();
    to
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_single_channel_guest_lands_in_that_channel_and_cannot_be_added_elsewhere(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "guest-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Vendors").await;
    let vendor = seed_channel(&state, ws, owner_id, "vendor", false).await;
    let ops = seed_channel(&state, ws, owner_id, "ops", false).await;
    let until = Utc::now() + Duration::days(30);

    let (status, created) = invite(
        &app,
        &owner,
        ws,
        json!({
            "email": unique_email("vera"),
            "role": "guest",
            "guest_kind": "single",
            "channel_ids": [vendor],
            "guest_until": until,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    assert_eq!(created["guest_kind"], "single");
    assert_eq!(created["channel_ids"], json!([vendor]));

    let (vera_id, vera) = register(&app, &created, "Vera Vendor").await;
    assert_eq!(
        channel_names(&app, &vera, ws).await,
        vec!["vendor".to_string()],
        "in #vendor and in nothing else, not even #general"
    );
    let row = member_row(&app, &owner, ws, vera_id).await.expect("listed");
    assert_eq!(row["role"], "guest");
    assert_eq!(row["guest_kind"], "single");
    assert!(row["guest_until"].is_string(), "{row:?}");
    let invited_by: Option<Uuid> = sqlx::query_scalar(
        "SELECT guest_invited_by FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(ws)
    .bind(vera_id)
    .fetch_one(&state.pool)
    .await
    .expect("seat");
    assert_eq!(invited_by, Some(owner_id));

    let (status, body) = add(&app, &owner, ops, vera_id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    let said = body.to_string();
    assert!(
        said.contains("Vera Vendor") && said.contains("multi-channel guest"),
        "the answer names her and says what to do: {said}"
    );
    assert_eq!(
        channel_names(&app, &vera, ws).await,
        vec!["vendor".to_string()]
    );

    let (status, body) = seat(
        &app,
        &owner,
        ws,
        vera_id,
        json!({ "guest_kind": "multi", "guest_until": until }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = add(&app, &owner, ops, vera_id).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a multi-channel guest goes anywhere asked: {body:?}"
    );

    let (status, body) = seat(
        &app,
        &owner,
        ws,
        vera_id,
        json!({ "guest_kind": "single", "guest_until": until }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "two channels are not one: {body:?}"
    );
    let (status, _) = seat(
        &app,
        &owner,
        ws,
        vera_id,
        json!({ "guest_kind": "multi", "guest_until": Utc::now() - Duration::hours(1) }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "an end date already gone"
    );
    let (status, body) = seat(
        &app,
        &owner,
        ws,
        vera_id,
        json!({ "guest_kind": "multi", "guest_until": null }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["guest_until"], Value::Null, "the date clears");

    let (status, _) = seat(
        &app,
        &owner,
        ws,
        owner_id,
        json!({ "guest_kind": "multi", "guest_until": null }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "an owner is not a guest");
    let (member_id, _, member) = seed_and_login(&app, &state, "guest-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let (status, _) = seat(
        &app,
        &member,
        ws,
        vera_id,
        json!({ "guest_kind": "single", "guest_until": null }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "only an admin changes a guest"
    );

    let changes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'guest.updated' AND resource_id = $1",
    )
    .bind(vera_id)
    .fetch_one(&state.pool)
    .await
    .expect("audit");
    assert_eq!(changes, 2, "the two changes that happened");

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}/members/{vera_id}/role"),
        Some(&owner),
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let row = member_row(&app, &owner, ws, vera_id).await.expect("listed");
    assert_eq!(
        row["guest_kind"],
        Value::Null,
        "a member is no kind of guest"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_guest_invite_says_where_the_guest_goes_and_goes_to_one_address(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "guest-inv-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Agency").await;
    let vendor = seed_channel(&state, ws, owner_id, "vendor", false).await;
    let ops = seed_channel(&state, ws, owner_id, "ops", false).await;
    let legal = seed_channel(&state, ws, owner_id, "legal", true).await;
    let (admin_id, _, admin) = seed_and_login(&app, &state, "guest-inv-admin", false).await;
    add_ws_member(&state, ws, admin_id, "admin").await;

    let (status, created) = invite(
        &app,
        &owner,
        ws,
        json!({
            "email": unique_email("milo"),
            "role": "guest",
            "guest_kind": "multi",
            "channel_ids": [vendor, ops, legal, vendor],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    let (_, milo) = register(&app, &created, "Milo Multi").await;
    assert_eq!(
        channel_names(&app, &milo, ws).await,
        vec!["legal".to_string(), "ops".to_string(), "vendor".to_string()],
        "all three, the private one too"
    );

    let other_owner = seed(&state, "guest-inv-other", false).await.0;
    let elsewhere = seed_workspace(&state, other_owner, "Elsewhere").await;
    let foreign = seed_channel(&state, elsewhere, other_owner, "theirs", false).await;
    add_ws_member(&state, elsewhere, owner_id, "member").await;
    join(&state, foreign, owner_id).await;
    let archived = seed_channel(&state, ws, owner_id, "old", false).await;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{archived}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let guest = |body: Value| {
        let mut base =
            json!({ "email": unique_email("g"), "role": "guest", "channel_ids": [vendor] });
        base.as_object_mut()
            .expect("object")
            .extend(body.as_object().expect("object").clone());
        base
    };
    let cases = [
        (
            owner.as_str(),
            json!({ "role": "guest", "channel_ids": [vendor], "max_uses": 5 }),
            StatusCode::UNPROCESSABLE_ENTITY,
            "a link for guests",
        ),
        (
            owner.as_str(),
            guest(json!({ "channel_ids": [] })),
            StatusCode::UNPROCESSABLE_ENTITY,
            "no channels",
        ),
        (
            owner.as_str(),
            guest(json!({ "guest_kind": "single", "channel_ids": [vendor, ops] })),
            StatusCode::UNPROCESSABLE_ENTITY,
            "a single-channel guest in two",
        ),
        (
            owner.as_str(),
            guest(json!({ "guest_until": Utc::now() - Duration::days(1) })),
            StatusCode::UNPROCESSABLE_ENTITY,
            "an end date already gone",
        ),
        (
            owner.as_str(),
            json!({ "email": unique_email("m"), "role": "member", "channel_ids": [vendor] }),
            StatusCode::UNPROCESSABLE_ENTITY,
            "channels on a member invite",
        ),
        (
            owner.as_str(),
            json!({ "email": unique_email("m"), "guest_until": Utc::now() + Duration::days(3) }),
            StatusCode::UNPROCESSABLE_ENTITY,
            "an end date on a member invite",
        ),
        (
            owner.as_str(),
            guest(json!({ "channel_ids": [foreign] })),
            StatusCode::UNPROCESSABLE_ENTITY,
            "a channel of another workspace",
        ),
        (
            owner.as_str(),
            guest(json!({ "channel_ids": [archived] })),
            StatusCode::FORBIDDEN,
            "an archived channel",
        ),
        (
            owner.as_str(),
            guest(json!({ "channel_ids": [Uuid::new_v4()] })),
            StatusCode::NOT_FOUND,
            "a channel that does not exist",
        ),
        (
            admin.as_str(),
            guest(json!({ "channel_ids": [legal] })),
            StatusCode::FORBIDDEN,
            "a private channel the admin is not in",
        ),
    ];
    for (token, body, expected, why) in cases {
        let (status, answer) = invite(&app, token, ws, body).await;
        assert_eq!(status, expected, "{why}: {answer:?}");
    }
    let guest_invites: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_invites WHERE workspace_id = $1 AND role = 'guest'",
    )
    .bind(ws)
    .fetch_one(&state.pool)
    .await
    .expect("invites");
    assert_eq!(guest_invites, 1, "none of the refused ones was kept");

    let (status, created) = invite(&app, &admin, ws, guest(json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    assert_eq!(
        created["guest_kind"], "multi",
        "a guest is multi-channel unless the invite says otherwise"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_group_dm_does_not_become_a_channel_while_a_single_channel_guest_in_it_is_elsewhere(
    pool: PgPool,
) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "guest-conv-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Studio").await;
    let vendor = seed_channel(&state, ws, owner_id, "vendor", false).await;
    let (ana_id, _, _) = seed_and_login(&app, &state, "guest-conv-ana", false).await;
    add_ws_member(&state, ws, ana_id, "member").await;
    let (sid_id, _) = seed(&state, "guest-conv-sid", false).await;
    make_guest(&state, ws, sid_id, "single", None).await;
    join(&state, vendor, sid_id).await;
    sqlx::query("UPDATE users SET display_name = 'Sid Single' WHERE id = $1")
        .bind(sid_id)
        .execute(&state.pool)
        .await
        .expect("name");

    let (status, group) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/conversations"),
        Some(&owner),
        Some(json!({ "participant_ids": [ana_id, sid_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{group:?}");
    let group: Uuid = group["id"].as_str().expect("id").parse().expect("uuid");

    let (status, body) = convert(&app, &owner, group).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    assert!(
        body.to_string().contains("Sid Single"),
        "the answer names who: {body:?}"
    );
    let kind: String = sqlx::query_scalar("SELECT channel_type::text FROM channels WHERE id = $1")
        .bind(group)
        .fetch_one(&state.pool)
        .await
        .expect("group");
    assert_eq!(kind, "group_dm", "nothing changed");
    let notices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE channel_id = $1")
        .bind(group)
        .fetch_one(&state.pool)
        .await
        .expect("messages");
    assert_eq!(notices, 0);

    let (status, body) = seat(
        &app,
        &owner,
        ws,
        sid_id,
        json!({ "guest_kind": "multi", "guest_until": null }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = convert(&app, &owner, group).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_guest_whose_end_date_passes_leaves_the_workspace_on_the_next_run(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "guest-exp-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Contract").await;
    let vendor = seed_channel(&state, ws, owner_id, "vendor", false).await;
    let (gus_id, _, gus) = seed_and_login(&app, &state, "guest-exp-gus", false).await;
    make_guest(
        &state,
        ws,
        gus_id,
        "multi",
        Some(Utc::now() + Duration::days(10)),
    )
    .await;
    join(&state, vendor, gus_id).await;
    let (kim_id, _, kim) = seed_and_login(&app, &state, "guest-exp-kim", false).await;
    make_guest(
        &state,
        ws,
        kim_id,
        "multi",
        Some(Utc::now() + Duration::days(10)),
    )
    .await;
    join(&state, vendor, kim_id).await;
    let home = seed_workspace(&state, gus_id, "Gus Home").await;

    crate::guests::expiry::run(&state).await;
    assert_eq!(
        channel_names(&app, &gus, ws).await,
        vec!["vendor".to_string()],
        "not yet"
    );

    sqlx::query(
        "UPDATE workspace_members SET guest_until = NOW() - interval '1 minute'
          WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(ws)
    .bind(gus_id)
    .execute(&state.pool)
    .await
    .expect("time passes");
    crate::guests::expiry::run(&state).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/channels"),
        Some(&gus),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "out of the workspace");
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{vendor}/messages"),
        Some(&gus),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(member_row(&app, &owner, ws, gus_id).await.is_none());
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{home}/channels"),
        Some(&gus),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a workspace of their own is untouched"
    );
    assert_eq!(
        channel_names(&app, &kim, ws).await,
        vec!["vendor".to_string()],
        "the guest whose date is ahead stays"
    );

    let told: Value = sqlx::query_scalar(
        "SELECT payload FROM event_outbox
          WHERE event_type = 'workspace.member_removed' AND payload->>'user_id' = $1",
    )
    .bind(gus_id.to_string())
    .fetch_one(&state.pool)
    .await
    .expect("the open tab hears it");
    assert_eq!(told["reason"], "guest_expired");
    let audited: Vec<Option<Uuid>> = sqlx::query_scalar(
        "SELECT user_id FROM audit_log WHERE action = 'guest.expired' AND resource_id = $1",
    )
    .bind(gus_id)
    .fetch_all(&state.pool)
    .await
    .expect("audit");
    assert_eq!(audited, vec![None], "the machine did it, once");

    crate::guests::expiry::run(&state).await;
    let removals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM event_outbox
          WHERE event_type = 'workspace.member_removed' AND payload->>'user_id' = $1",
    )
    .bind(gus_id.to_string())
    .fetch_one(&state.pool)
    .await
    .expect("outbox");
    assert_eq!(removals, 1, "a second run finds nobody");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_admins_and_the_inviter_hear_three_days_ahead_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "guest-warn-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Warned").await;
    let vendor = seed_channel(&state, ws, owner_id, "vendor", false).await;
    let (ivy_id, _, ivy) = seed_and_login(&app, &state, "guest-warn-ivy", false).await;
    add_ws_member(&state, ws, ivy_id, "admin").await;
    join(&state, vendor, ivy_id).await;
    let (max_id, _) = seed(&state, "guest-warn-max", false).await;
    add_ws_member(&state, ws, max_id, "member").await;

    let (status, created) = invite(
        &app,
        &ivy,
        ws,
        json!({ "email": unique_email("nell"), "role": "guest", "channel_ids": [vendor] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    let (nell_id, _) = register(&app, &created, "Nell Needed").await;
    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}/members/{ivy_id}/role"),
        Some(&owner),
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (status, body) = seat(
        &app,
        &owner,
        ws,
        nell_id,
        json!({ "guest_kind": "multi", "guest_until": Utc::now() + Duration::days(10) }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    crate::guests::expiry::run(&state).await;
    assert!(
        ending_notices(&state, ws).await.is_empty(),
        "ten days out is too early"
    );

    let (status, _) = seat(
        &app,
        &owner,
        ws,
        nell_id,
        json!({ "guest_kind": "multi", "guest_until": Utc::now() + Duration::days(2) }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    crate::guests::expiry::run(&state).await;
    let mut expected = vec![owner_id, ivy_id];
    expected.sort();
    assert_eq!(
        ending_notices(&state, ws).await,
        expected,
        "the owner, and Ivy who invited her though no longer an admin; not Max"
    );
    let data: Value = sqlx::query_scalar(
        "SELECT data FROM notifications WHERE user_id = $1 AND data->>'notice' = 'guest_ending'",
    )
    .bind(owner_id)
    .fetch_one(&state.pool)
    .await
    .expect("notice");
    assert_eq!(data["guest_name"], "Nell Needed");
    assert_eq!(data["guest_user_id"], json!(nell_id));
    assert!(data["guest_until"].is_string());

    crate::guests::expiry::run(&state).await;
    assert_eq!(ending_notices(&state, ws).await.len(), 2, "once only");

    let (status, _) = seat(
        &app,
        &owner,
        ws,
        nell_id,
        json!({ "guest_kind": "multi", "guest_until": Utc::now() + Duration::days(1) }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    crate::guests::expiry::run(&state).await;
    assert_eq!(
        ending_notices(&state, ws).await.len(),
        4,
        "a moved date is a new date to hear about"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_guest_always_has_a_kind_and_only_a_guest_has_one(pool: PgPool) {
    let (_app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "guest-kind-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Kinds").await;
    let (pat_id, _) = seed(&state, "guest-kind-pat", false).await;

    let kindless = sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'guest')",
    )
    .bind(ws)
    .bind(pat_id)
    .execute(&state.pool)
    .await;
    assert!(kindless.is_err(), "a guest without a kind cannot exist");
    let member_with_kind = sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role, guest_kind)
         VALUES ($1, $2, 'member', 'single')",
    )
    .bind(ws)
    .bind(pat_id)
    .execute(&state.pool)
    .await;
    assert!(member_with_kind.is_err(), "nor a member with one");

    state
        .workspace_service
        .join_workspace(ws, pat_id, &crate::workspace::models::WorkspaceRole::Guest)
        .await
        .expect("join as a guest");
    let kind: Option<String> = sqlx::query_scalar(
        "SELECT guest_kind::text FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(ws)
    .bind(pat_id)
    .fetch_one(&state.pool)
    .await
    .expect("seat");
    assert_eq!(
        kind.as_deref(),
        Some("multi"),
        "a guest nobody described is multi-channel"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_opens_only_for_those_who_can_read_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "peek-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Peeking").await;
    let vendor = seed_channel(&state, ws, owner_id, "vendor", false).await;
    let general = seed_channel(&state, ws, owner_id, "general", false).await;
    let secret = seed_channel(&state, ws, owner_id, "secret", true).await;
    let (member_id, _, member) = seed_and_login(&app, &state, "peek-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "peek-guest", false).await;
    make_guest(&state, ws, guest_id, "multi", None).await;
    join(&state, vendor, guest_id).await;

    let open = |token: &str, ch: Uuid| {
        let app = app.clone();
        let token = token.to_owned();
        async move {
            send(
                &app,
                "GET",
                &format!("/api/channels/{ch}"),
                Some(&token),
                None,
            )
            .await
            .0
        }
    };

    assert_eq!(
        open(&guest, vendor).await,
        StatusCode::OK,
        "the guest's own channel"
    );
    assert_eq!(
        open(&guest, general).await,
        StatusCode::FORBIDDEN,
        "a public channel the guest is not in stays shut"
    );
    assert_eq!(
        open(&member, general).await,
        StatusCode::OK,
        "a member previews a public channel before joining"
    );
    assert_eq!(
        open(&member, secret).await,
        StatusCode::FORBIDDEN,
        "a private channel says nothing to those outside it"
    );
    assert_eq!(open(&owner, secret).await, StatusCode::OK);
}
