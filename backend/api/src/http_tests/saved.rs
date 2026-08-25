use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::common::*;

#[sqlx::test(migrations = "../migrations")]
async fn saving_a_channel_message_puts_it_in_your_own_list(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "saved-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Saved WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

    let (_, message) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "the runbook link" })),
    )
    .await;
    let message_id = message["id"].as_str().expect("id").to_string();

    let (status, saved) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/saved"),
        Some(&owner_token),
        Some(json!({ "message_id": message_id, "note": "for the upgrade" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved:?}");
    let saved_id = saved["id"].as_str().expect("id").to_string();

    let (status, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/saved"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = listing["data"].as_array().expect("array");
    assert_eq!(items.len(), 1, "{listing:?}");
    assert_eq!(items[0]["content"], "the runbook link");
    assert_eq!(items[0]["channel_id"], ch_id.to_string());
    assert_eq!(items[0]["note"], "for the upgrade");

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/saved/{saved_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/saved"),
        Some(&owner_token),
        None,
    )
    .await;
    assert!(listing["data"].as_array().expect("array").is_empty());
}

#[sqlx::test(migrations = "../migrations")]
async fn saving_the_same_message_twice_is_the_same_row(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "saved-twice", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Saved WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

    let (_, message) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "twice" })),
    )
    .await;
    let message_id = message["id"].as_str().expect("id").to_string();

    let (_, first) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/saved"),
        Some(&owner_token),
        Some(json!({ "message_id": message_id })),
    )
    .await;
    let (status, second) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/saved"),
        Some(&owner_token),
        Some(json!({ "message_id": message_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second:?}");
    assert_eq!(first["id"], second["id"], "saving is idempotent");

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/saved"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(listing["data"].as_array().expect("array").len(), 1);
}

#[sqlx::test(migrations = "../migrations")]
async fn you_cannot_save_a_message_you_cannot_read(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "saved-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Saved WS").await;
    let private_id = seed_channel(&state, ws_id, owner_id, "secrets", true).await;

    let (outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "saved-outsider", false).await;
    add_ws_member(&state, ws_id, outsider_id, "member").await;

    let (_, message) = send(
        &app,
        "POST",
        &format!("/api/channels/{private_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "not for you" })),
    )
    .await;
    let message_id = message["id"].as_str().expect("id").to_string();

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/saved"),
        Some(&outsider_token),
        Some(json!({ "message_id": message_id })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn a_saved_dm_is_visible_only_to_the_person_who_saved_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "saved-dm-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Saved WS").await;

    let (partner_id, _, partner_token) =
        seed_and_login(&app, &state, "saved-dm-partner", false).await;
    add_ws_member(&state, ws_id, partner_id, "member").await;

    let (_, conversation) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [partner_id] })),
    )
    .await;
    let conv_id = conversation["id"].as_str().expect("id").to_string();

    let (_, message) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "keep this one" })),
    )
    .await;
    let message_id = message["id"].as_str().expect("id").to_string();

    let (status, saved) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/saved"),
        Some(&owner_token),
        Some(json!({ "conversation_message_id": message_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved:?}");
    let saved_id = saved["id"].as_str().expect("id").to_string();

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/saved"),
        Some(&partner_token),
        None,
    )
    .await;
    assert!(
        listing["data"].as_array().expect("array").is_empty(),
        "a saved list is one person's"
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/saved/{saved_id}"),
        Some(&partner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn saving_needs_exactly_one_target(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "saved-target", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Saved WS").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/saved"),
        Some(&owner_token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
