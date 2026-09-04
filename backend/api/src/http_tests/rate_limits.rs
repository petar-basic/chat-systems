use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::common::*;

/// The budget is per user and per class of action, so this runs against the
/// smallest class rather than the busiest: five workspaces an hour is five
/// requests to reach, where proving it through the message class would take a
/// hundred and twenty writes and tell us nothing more.
#[test_macros::db_test(migrations = "../migrations")]
async fn a_write_budget_is_spent_per_user_and_per_class(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "limit-owner", false).await;

    for i in 0..5 {
        let (status, body) = send(
            &app,
            "POST",
            "/api/workspaces",
            Some(&token),
            Some(json!({ "name": format!("Budget {i}") })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "workspace {i} is within budget: {body:?}"
        );
    }

    let (status, body) = send(
        &app,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({ "name": "One too many" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "the sixth workspace this hour is refused: {body:?}"
    );

    let ws_id = seed_workspace(&state, owner_id, "Still Writable").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "budgets", false).await;
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&token),
        Some(json!({ "content": "posting is a different budget" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "spending the workspace budget must not stop them writing: {body:?}"
    );

    let (_, _, other_token) = seed_and_login(&app, &state, "limit-other", false).await;
    let (status, body) = send(
        &app,
        "POST",
        "/api/workspaces",
        Some(&other_token),
        Some(json!({ "name": "Somebody else" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "one person's flood must not limit anybody else: {body:?}"
    );
}
