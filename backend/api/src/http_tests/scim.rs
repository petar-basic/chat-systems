use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;

async fn scim_token(app: &axum::Router, admin_token: &str) -> String {
    let (status, body) = send(
        app,
        "POST",
        "/api/admin/scim/tokens",
        Some(admin_token),
        Some(json!({ "description": "Okta" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "token creation: {body:?}");
    body["token"]
        .as_str()
        .expect("the token is revealed once")
        .to_string()
}

async fn membership_counts(state: &AppState, user_id: Uuid) -> (i64, i64) {
    let workspaces: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workspace_members WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .expect("count workspace members");
    let channels: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM channel_members WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .expect("count channel members");
    (workspaces, channels)
}

/// The whole point of the ticket: one call from the identity provider has to
/// leave nothing behind — no session, no workspace, no private channel.
#[test_macros::db_test(migrations = "../migrations")]
async fn deactivating_ends_the_sessions_and_the_memberships(pool: sqlx::PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _, admin) = seed_and_login(&app, &state, "scim-admin", true).await;
    let (owner_id, _, _owner) = seed_and_login(&app, &state, "scim-owner", false).await;
    let (leaver_id, _, leaver) = seed_and_login(&app, &state, "scim-leaver", false).await;

    let ws_id = seed_workspace(&state, owner_id, "Acme").await;
    add_ws_member(&state, ws_id, leaver_id, "member").await;
    let channel_id = seed_channel(&state, ws_id, owner_id, "private-plans", true).await;
    state
        .workspace_service
        .repo
        .add_channel_member(
            channel_id,
            leaver_id,
            &crate::workspace::models::ChannelRole::Member,
        )
        .await
        .expect("add to private channel");

    let (before_ws, before_ch) = membership_counts(&state, leaver_id).await;
    assert_eq!((before_ws, before_ch), (1, 1));

    let token = scim_token(&app, &admin).await;
    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/scim/v2/Users/{leaver_id}"),
        Some(&token),
        Some(json!({
            "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
            "Operations": [{ "op": "replace", "path": "active", "value": false }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["active"], json!(false));

    let (after_ws, after_ch) = membership_counts(&state, leaver_id).await;
    assert_eq!(
        (after_ws, after_ch),
        (0, 0),
        "deprovisioning leaves no way back in"
    );

    let (status, _) = send(&app, "GET", "/api/users/me", Some(&leaver), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the access token they already held stops working"
    );

    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'user.suspended' AND user_id IS NULL \
         AND details->>'via' = 'scim'",
    )
    .fetch_one(&state.pool)
    .await
    .expect("count audit");
    assert_eq!(audited, 1, "the machine caller is named, not borrowed");
}

/// An identity provider retrying a delete must not be able to take a workspace's
/// history with it. Erasure is CS-031's job and stays an administrator's choice.
#[test_macros::db_test(migrations = "../migrations")]
async fn delete_deactivates_and_destroys_nothing(pool: sqlx::PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _, admin) = seed_and_login(&app, &state, "scim-admin", true).await;
    let (leaver_id, leaver_email, _) = seed_and_login(&app, &state, "scim-gone", false).await;
    let ws_id = seed_workspace(&state, leaver_id, "Solo").await;
    let channel_id = seed_channel(&state, ws_id, leaver_id, "main", false).await;
    let message = state
        .message_repo
        .create_message(crate::messaging::models::NewMessage {
            channel_id,
            user_id: leaver_id,
            content: "still here",
            thread_parent_id: None,
            client_message_id: None,
            mentioned: &[],
        })
        .await
        .expect("post a message");

    let token = scim_token(&app, &admin).await;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/scim/v2/Users/{leaver_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let user = state
        .auth_service
        .repo()
        .find_by_id(leaver_id)
        .await
        .expect("query")
        .expect("the account is still there");
    assert_eq!(user.email, leaver_email);
    assert_eq!(user.status, crate::auth::models::UserStatus::Suspended);

    let survives: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE id = $1")
        .bind(message.id)
        .fetch_one(&state.pool)
        .await
        .expect("count messages");
    assert_eq!(survives, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn coming_back_needs_a_fresh_invite(pool: sqlx::PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _, admin) = seed_and_login(&app, &state, "scim-admin", true).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "scim-owner", false).await;
    let (returner_id, returner_email, _) = seed_and_login(&app, &state, "scim-back", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Acme").await;
    add_ws_member(&state, ws_id, returner_id, "member").await;

    let token = scim_token(&app, &admin).await;
    let patch = |active: bool| {
        json!({
            "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
            "Operations": [{ "op": "replace", "value": { "active": active } }]
        })
    };

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/scim/v2/Users/{returner_id}"),
        Some(&token),
        Some(patch(false)),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "Entra's pathless shape is understood"
    );

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/scim/v2/Users/{returner_id}"),
        Some(&token),
        Some(patch(true)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["active"], json!(true));

    let (workspaces, channels) = membership_counts(&state, returner_id).await;
    assert_eq!(
        (workspaces, channels),
        (0, 0),
        "the account is back, the access is not"
    );

    let refreshed = login(&app, &returner_email, PASSWORD).await;
    assert!(!refreshed.is_empty(), "they can sign in again");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_wrong_or_revoked_token_gets_nowhere(pool: sqlx::PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _, admin) = seed_and_login(&app, &state, "scim-admin", true).await;
    let (target_id, _, _) = seed_and_login(&app, &state, "scim-target", false).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/scim/v2/Users/{target_id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no token at all");

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/scim/v2/Users/{target_id}"),
        Some("not-a-real-token"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body["schemas"][0],
        json!("urn:ietf:params:scim:api:messages:2.0:Error"),
        "even the failure is SCIM-shaped, or the provider shows an opaque error"
    );

    let token = scim_token(&app, &admin).await;
    let tokens = state.scim_repo.list().await.expect("list");
    let id = tokens.first().expect("one token").id;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/admin/scim/tokens/{id}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/scim/v2/Users/{target_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "revocation takes effect");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn rotation_replaces_the_credential_without_a_gap(pool: sqlx::PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _, admin) = seed_and_login(&app, &state, "scim-admin", true).await;
    let (target_id, _, _) = seed_and_login(&app, &state, "scim-target", false).await;

    let old = scim_token(&app, &admin).await;
    let id = state
        .scim_repo
        .list()
        .await
        .expect("list")
        .first()
        .expect("one token")
        .id;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/admin/scim/tokens/{id}/rotate"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let new = body["token"].as_str().expect("new token").to_string();
    assert_ne!(new, old);

    let path = format!("/api/scim/v2/Users/{target_id}");
    let (status, _) = send(&app, "GET", &path, Some(&new), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&app, "GET", &path, Some(&old), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn provisioning_creates_an_account_with_no_password(pool: sqlx::PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _, admin) = seed_and_login(&app, &state, "scim-admin", true).await;
    let token = scim_token(&app, &admin).await;

    let (status, body) = send(
        &app,
        "POST",
        "/api/scim/v2/Users",
        Some(&token),
        Some(json!({
            "schemas": ["urn:ietf:params:scim:schemas:core:2.0:User"],
            "userName": "Newcomer@Acme.test",
            "displayName": "New Comer",
            "active": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    assert_eq!(body["userName"], json!("newcomer@acme.test"));
    assert_eq!(body["displayName"], json!("New Comer"));
    assert_eq!(body["meta"]["resourceType"], json!("User"));
    assert_eq!(body["active"], json!(true));

    let user = state
        .auth_service
        .repo()
        .find_by_email("newcomer@acme.test")
        .await
        .expect("query")
        .expect("provisioned");
    assert!(
        user.password_hash.is_none(),
        "the identity provider is the credential"
    );

    let (status, body) = send(
        &app,
        "POST",
        "/api/scim/v2/Users",
        Some(&token),
        Some(json!({ "userName": "newcomer@acme.test" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body["scimType"],
        json!("uniqueness"),
        "providers branch on scimType, not on prose"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_lookup_a_provider_makes_before_every_sync(pool: sqlx::PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _, admin) = seed_and_login(&app, &state, "scim-admin", true).await;
    let (found_id, found_email, _) = seed_and_login(&app, &state, "scim-found", false).await;
    let token = scim_token(&app, &admin).await;

    let filter = urlencoding(&format!("userName eq \"{found_email}\""));
    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/scim/v2/Users?filter={filter}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body["schemas"][0],
        json!("urn:ietf:params:scim:api:messages:2.0:ListResponse")
    );
    assert_eq!(body["totalResults"], json!(1));
    assert_eq!(body["Resources"][0]["id"], json!(found_id));

    let missing = urlencoding("userName eq \"nobody@nowhere.test\"");
    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/scim/v2/Users?filter={missing}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "an empty list, not a 404");
    assert_eq!(body["totalResults"], json!(0));

    let unsupported = urlencoding("displayName co \"admin\"");
    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/scim/v2/Users?filter={unsupported}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["scimType"], json!("invalidFilter"));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_an_instance_admin_may_mint_a_token(pool: sqlx::PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _, ordinary) = seed_and_login(&app, &state, "scim-nobody", false).await;

    let (status, _) = send(
        &app,
        "POST",
        "/api/admin/scim/tokens",
        Some(&ordinary),
        Some(json!({ "description": "mine" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

fn urlencoding(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
