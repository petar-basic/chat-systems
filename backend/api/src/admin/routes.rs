use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{middleware, Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::middleware::{admin_middleware, auth_middleware, AuthUser};
use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/admin/health", get(health))
        .route("/admin/stats", get(stats))
        .route("/admin/users", get(list_users))
        .route("/admin/users/:user_id/suspend", post(suspend_user))
        .route("/admin/users/:user_id/activate", post(activate_user))
        .route(
            "/admin/users/:user_id/instance-role",
            patch(update_instance_role),
        )
        .route("/admin/workspaces", get(list_workspaces))
        .route("/admin/workspaces/:ws_id", delete(delete_workspace))
        .layer(middleware::from_fn(admin_middleware))
        .layer(middleware::from_fn(auth_middleware));

    Router::new().merge(routes).with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "chat-api",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(serde::Deserialize)]
struct PaginationQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn stats(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let workspace_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workspaces")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let message_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM messages")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let file_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "users": user_count.0,
        "workspaces": workspace_count.0,
        "messages": message_count.0,
        "files": file_count.0,
    })))
}

#[derive(sqlx::FromRow)]
struct AdminUserRow {
    id: Uuid,
    email: String,
    display_name: Option<String>,
    status: String,
    is_instance_admin: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    let rows: Vec<AdminUserRow> = sqlx::query_as(
        "SELECT id, email, display_name, status::text AS status, is_instance_admin, created_at \
         FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let users: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|u| {
            serde_json::json!({
                "id": u.id,
                "email": u.email,
                "display_name": u.display_name,
                "status": u.status,
                "is_instance_admin": u.is_instance_admin,
                "created_at": u.created_at,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "data": users })))
}

async fn suspend_user(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(user_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("UPDATE users SET status = 'suspended', updated_at = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    audit(
        &state,
        auth.user_id,
        "user.suspend",
        "user",
        user_id,
        serde_json::json!({}),
    )
    .await;
    Ok(Json(serde_json::json!({ "status": "suspended" })))
}

async fn activate_user(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(user_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("UPDATE users SET status = 'active', updated_at = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    audit(
        &state,
        auth.user_id,
        "user.activate",
        "user",
        user_id,
        serde_json::json!({}),
    )
    .await;
    Ok(Json(serde_json::json!({ "status": "activated" })))
}

#[derive(serde::Deserialize)]
struct UpdateInstanceRoleRequest {
    is_instance_admin: bool,
}

async fn update_instance_role(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(user_id): Path<Uuid>,
    Json(body): Json<UpdateInstanceRoleRequest>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("UPDATE users SET is_instance_admin = $1, updated_at = NOW() WHERE id = $2")
        .bind(body.is_instance_admin)
        .bind(user_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    audit(
        &state,
        auth.user_id,
        "user.update_role",
        "user",
        user_id,
        serde_json::json!({ "is_instance_admin": body.is_instance_admin }),
    )
    .await;
    Ok(Json(
        serde_json::json!({ "is_instance_admin": body.is_instance_admin }),
    ))
}

async fn list_workspaces(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    let rows: Vec<(Uuid, String, String, Uuid, bool, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, name, slug, owner_id, is_active, created_at FROM workspaces ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let workspaces: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, name, slug, owner_id, is_active, created_at)| {
            serde_json::json!({
                "id": id,
                "name": name,
                "slug": slug,
                "owner_id": owner_id,
                "is_active": is_active,
                "created_at": created_at,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "data": workspaces })))
}

async fn delete_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(ws_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    audit(
        &state,
        auth.user_id,
        "workspace.delete",
        "workspace",
        ws_id,
        serde_json::json!({}),
    )
    .await;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

async fn audit(
    state: &AppState,
    actor_id: Uuid,
    action: &str,
    resource_type: &str,
    resource_id: Uuid,
    details: serde_json::Value,
) {
    let result = sqlx::query(
        "INSERT INTO audit_log (user_id, action, resource_type, resource_id, details) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(actor_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(details)
    .execute(&state.pool)
    .await;

    if let Err(e) = result {
        tracing::warn!("Failed to write audit log: {}", e);
    }
}
