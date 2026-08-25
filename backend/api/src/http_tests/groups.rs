use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

async fn create_group(
    app: &axum::Router,
    token: &str,
    ws_id: Uuid,
    handle: &str,
) -> (StatusCode, serde_json::Value) {
    send(
        app,
        "POST",
        &format!("/api/workspaces/{ws_id}/groups"),
        Some(token),
        Some(json!({ "handle": handle, "name": handle })),
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_group_is_created_by_an_admin_and_seen_by_everyone(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "grp-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Groups WS").await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "grp-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let (status, created) = create_group(&app, &owner_token, ws_id, "@Backend").await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    assert_eq!(created["handle"], "backend", "a handle is case-insensitive");

    let (status, again) = create_group(&app, &owner_token, ws_id, "backend").await;
    assert_eq!(status, StatusCode::CONFLICT, "{again:?}");

    let (status, listed) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/groups"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a handle nobody can discover is a trap"
    );
    assert_eq!(listed["data"][0]["handle"], "backend");
    assert_eq!(listed["data"][0]["member_count"], 0);

    let (status, _) = create_group(&app, &member_token, ws_id, "frontend").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "membership is not administration"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn membership_is_workspace_membership_first(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "grp-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Groups WS").await;
    let (member_id, _, _) = seed_and_login(&app, &state, "grp-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let (outsider_id, _, _) = seed_and_login(&app, &state, "grp-outsider", false).await;

    let (_, group) = create_group(&app, &owner_token, ws_id, "backend").await;
    let group_id = group["id"].as_str().expect("group id");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/groups/{group_id}/members"),
        Some(&owner_token),
        Some(json!({ "user_id": outsider_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a group cannot reach somebody who is not in the workspace"
    );

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/groups/{group_id}/members"),
        Some(&owner_token),
        Some(json!({ "user_id": member_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, members) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/groups/{group_id}/members"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(members["data"].as_array().expect("members").len(), 1);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}/groups/{group_id}/members/{member_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action IN ('group.member_added', 'group.member_removed')",
    )
    .fetch_one(&state.pool)
    .await
    .expect("count audit");
    assert_eq!(audited, 2, "membership changes are audited");
}

/// The decision that makes group mentions safe: the group is a shorthand for
/// people, and it reaches them through the channel like `@channel` does.
#[test_macros::db_test(migrations = "../migrations")]
async fn mentioning_a_group_notifies_only_the_members_who_are_in_the_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "grp-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Groups WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "private-plans", true).await;

    let (inside_id, _, _) = seed_and_login(&app, &state, "grp-inside", false).await;
    add_ws_member(&state, ws_id, inside_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(
            ch_id,
            inside_id,
            &crate::workspace::models::ChannelRole::Member,
        )
        .await
        .expect("add to channel");

    let (outside_id, _, _) = seed_and_login(&app, &state, "grp-outside", false).await;
    add_ws_member(&state, ws_id, outside_id, "member").await;

    let (_, group) = create_group(&app, &owner_token, ws_id, "backend").await;
    let group_id = group["id"].as_str().expect("group id").to_string();
    for member in [inside_id, outside_id] {
        let (status, _) = send(
            &app,
            "POST",
            &format!("/api/workspaces/{ws_id}/groups/{group_id}/members"),
            Some(&owner_token),
            Some(json!({ "user_id": member })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": format!("ship it @[backend](group:{group_id})") })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let unread_for = |user_id: Uuid| {
        let pool = state.pool.clone();
        async move {
            sqlx::query_scalar::<_, i32>(
                "SELECT mention_count FROM channel_members WHERE channel_id = $1 AND user_id = $2",
            )
            .bind(ch_id)
            .bind(user_id)
            .fetch_optional(&pool)
            .await
            .expect("read unread")
        }
    };

    assert_eq!(
        unread_for(inside_id).await,
        Some(1),
        "the member who can read the channel is mentioned"
    );
    assert_eq!(
        unread_for(outside_id).await,
        None,
        "the member who is not in the channel is not reachable through it"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_group_from_another_workspace_never_resolves(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "grp-owner", false).await;
    let ours = seed_workspace(&state, owner_id, "Ours").await;
    let theirs = seed_workspace(&state, owner_id, "Theirs").await;
    let ch_id = seed_channel(&state, ours, owner_id, "general", false).await;

    let (member_id, _, _) = seed_and_login(&app, &state, "grp-member", false).await;
    add_ws_member(&state, ours, member_id, "member").await;
    add_ws_member(&state, theirs, member_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(
            ch_id,
            member_id,
            &crate::workspace::models::ChannelRole::Member,
        )
        .await
        .expect("add to channel");

    let (_, group) = create_group(&app, &owner_token, theirs, "backend").await;
    let group_id = group["id"].as_str().expect("group id").to_string();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{theirs}/groups/{group_id}/members"),
        Some(&owner_token),
        Some(json!({ "user_id": member_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": format!("@[backend](group:{group_id}) hello") })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mentions: Option<i32> = sqlx::query_scalar(
        "SELECT mention_count FROM channel_members WHERE channel_id = $1 AND user_id = $2",
    )
    .bind(ch_id)
    .bind(member_id)
    .fetch_optional(&state.pool)
    .await
    .expect("read unread");
    assert_eq!(
        mentions,
        Some(0),
        "a group id from another workspace resolves to nobody"
    );
}
