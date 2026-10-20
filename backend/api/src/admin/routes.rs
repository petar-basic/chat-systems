use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::{middleware, Json};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::AppResult;

use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::dto::{DataList, StatusResponse};
use crate::middleware::{admin_middleware, AuthUser};
use crate::pagination::PageQuery;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(health))
        .routes(routes!(stats))
        .routes(routes!(list_audit_log))
        .routes(routes!(list_users))
        .routes(routes!(suspend_user))
        .routes(routes!(activate_user))
        .routes(routes!(update_instance_role))
        .routes(routes!(list_workspaces))
        .routes(routes!(delete_workspace))
        .layer(middleware::from_fn(admin_middleware))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminHealth {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InstanceStats {
    pub users: i64,
    pub workspaces: i64,
    pub messages: i64,
    pub files: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InstanceRoleResponse {
    pub is_instance_admin: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminWorkspace {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_id: Uuid,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(get, path = "/admin/health", tag = "admin", responses((status = 200, body = AdminHealth)))]
async fn health() -> Json<AdminHealth> {
    Json(AdminHealth {
        status: "ok",
        service: "chat-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[utoipa::path(get, path = "/admin/stats", tag = "admin", responses((status = 200, body = InstanceStats)))]
async fn stats(State(state): State<Arc<AppState>>) -> AppResult<Json<InstanceStats>> {
    let user_count = sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM users"#)
        .fetch_one(&state.pool)
        .await?;

    let workspace_count = sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM workspaces"#)
        .fetch_one(&state.pool)
        .await?;

    let message_count = sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM messages"#)
        .fetch_one(&state.pool)
        .await?;

    let file_count = sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM files"#)
        .fetch_one(&state.pool)
        .await?;

    Ok(Json(InstanceStats {
        users: user_count,
        workspaces: workspace_count,
        messages: message_count,
        files: file_count,
    }))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminUser {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub status: String,
    pub is_instance_admin: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(get, path = "/admin/users", tag = "admin", params(PageQuery), responses((status = 200, body = DataList<AdminUser>)))]
async fn list_users(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PageQuery>,
) -> AppResult<Json<DataList<AdminUser>>> {
    let users = sqlx::query_as!(
        AdminUser,
        r#"SELECT id, email, display_name, status::text AS "status!",
                  is_instance_admin AS "is_instance_admin!", created_at
             FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
        params.limit(),
        params.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(users.into()))
}

#[utoipa::path(
    operation_id = "admin_list_audit_log",
    get, path = "/admin/audit-log", tag = "admin", params(audit::AuditQuery), responses((status = 200, body = DataList<audit::AuditRow>)))]
async fn list_audit_log(
    State(state): State<Arc<AppState>>,
    Query(query): Query<audit::AuditQuery>,
) -> AppResult<Json<DataList<audit::AuditRow>>> {
    let entries = audit::list(&state, query.workspace_id, &query).await?;
    Ok(Json(entries.into()))
}

#[utoipa::path(post, path = "/admin/users/{user_id}/suspend", tag = "admin", responses((status = 200, body = StatusResponse)))]
async fn suspend_user(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(user_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    sqlx::query!(
        "UPDATE users SET status = 'suspended', updated_at = NOW() WHERE id = $1",
        user_id
    )
    .execute(&state.pool)
    .await?;

    crate::sessions::revoke(
        &state,
        user_id,
        crate::sessions::SessionScope::All,
        "account suspended",
    )
    .await?;

    let _ = state
        .publisher
        .publish("user.suspended", serde_json::json!({ "user_id": user_id }))
        .await;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::UserSuspended, auth.user_id)
            .resource(user_id)
            .ip(&ip),
    )
    .await;
    Ok(Json(StatusResponse::new("suspended")))
}

#[utoipa::path(post, path = "/admin/users/{user_id}/activate", tag = "admin", responses((status = 200, body = StatusResponse)))]
async fn activate_user(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(user_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    sqlx::query!(
        "UPDATE users SET status = 'active', updated_at = NOW() WHERE id = $1",
        user_id
    )
    .execute(&state.pool)
    .await?;

    crate::sessions::restore(&state, user_id).await;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::UserActivated, auth.user_id)
            .resource(user_id)
            .ip(&ip),
    )
    .await;
    Ok(Json(StatusResponse::new("activated")))
}

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct UpdateInstanceRoleRequest {
    pub is_instance_admin: bool,
}

#[utoipa::path(patch, path = "/admin/users/{user_id}/instance-role", tag = "admin", request_body = UpdateInstanceRoleRequest, responses((status = 200, body = InstanceRoleResponse)))]
async fn update_instance_role(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(user_id): Path<Uuid>,
    Json(body): Json<UpdateInstanceRoleRequest>,
) -> AppResult<Json<InstanceRoleResponse>> {
    sqlx::query!(
        "UPDATE users SET is_instance_admin = $1, updated_at = NOW() WHERE id = $2",
        body.is_instance_admin,
        user_id
    )
    .execute(&state.pool)
    .await?;
    audit::record(
        &state,
        AuditEntry::new(AuditAction::InstanceRoleChanged, auth.user_id)
            .resource(user_id)
            .ip(&ip)
            .details(serde_json::json!({ "is_instance_admin": body.is_instance_admin })),
    )
    .await;
    Ok(Json(InstanceRoleResponse {
        is_instance_admin: body.is_instance_admin,
    }))
}

#[utoipa::path(
    operation_id = "admin_list_workspaces",
    get, path = "/admin/workspaces", tag = "admin", params(PageQuery), responses((status = 200, body = DataList<AdminWorkspace>)))]
async fn list_workspaces(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PageQuery>,
) -> AppResult<Json<DataList<AdminWorkspace>>> {
    let data = sqlx::query_as!(
        AdminWorkspace,
        r#"SELECT id, name, slug, owner_id, is_active AS "is_active!", created_at
             FROM workspaces ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
        params.limit(),
        params.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(DataList { data }))
}

#[utoipa::path(
    operation_id = "admin_delete_workspace",
    delete, path = "/admin/workspaces/{ws_id}", tag = "admin", responses((status = 200, body = StatusResponse)))]
async fn delete_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    // Soft-delete (reversible via restore) instead of an irreversible cascade.
    let mut tx = state.pool.begin().await?;
    state
        .workspace_service
        .repo
        .soft_delete_workspace_in(&mut tx, ws_id)
        .await?;
    let staged = state
        .publisher
        .stage_workspace_deleted(&mut tx, ws_id, "soft")
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::WorkspaceDeleted, auth.user_id)
            .workspace(ws_id)
            .resource(ws_id)
            .ip(&ip)
            .details(serde_json::json!({ "hard": false })),
    )
    .await;

    Ok(Json(StatusResponse::new("deleted")))
}
