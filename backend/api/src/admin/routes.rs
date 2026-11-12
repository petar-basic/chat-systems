use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::{middleware, Json};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::auth::models::InstanceRole;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::{admin_middleware, AuthUser};
use crate::net::AcceptLanguage;
use crate::pagination::PageQuery;
use crate::state::AppState;
use crate::workspace::models::{AdminWorkspace, WorkspaceMember, WorkspacePerson, WorkspaceRole};
use crate::workspace::ownership;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(health))
        .routes(routes!(stats))
        .routes(routes!(send_test_email))
        .routes(routes!(get_instance_settings, update_instance_settings))
        .routes(routes!(list_audit_log))
        .routes(routes!(list_users))
        .routes(routes!(suspend_user))
        .routes(routes!(activate_user))
        .routes(routes!(update_instance_role))
        .routes(routes!(list_workspaces))
        .routes(routes!(list_workspace_people))
        .routes(routes!(appoint_workspace_owner))
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
pub struct EmailTest {
    pub sent_to: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InstanceSettings {
    pub email_code_sign_in: bool,
    pub email_configured: bool,
    pub workspace_creation_restricted: bool,
}

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct UpdateInstanceSettings {
    pub email_code_sign_in: Option<bool>,
    pub workspace_creation_restricted: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InstanceRoleResponse {
    pub role: InstanceRole,
}

#[utoipa::path(get, path = "/admin/health", tag = "admin", responses((status = 200, body = AdminHealth)))]
async fn health() -> Json<AdminHealth> {
    Json(AdminHealth {
        status: "ok",
        service: "chat-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[utoipa::path(post, path = "/admin/email/test", tag = "admin", responses((status = 200, body = EmailTest)))]
async fn send_test_email(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    accept_language: AcceptLanguage,
) -> AppResult<Json<EmailTest>> {
    let user = state
        .auth_service
        .repo()
        .find_by_id(auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    let language = state
        .auth_service
        .language_for(user.id, accept_language.header())
        .await?;
    let error = state
        .auth_service
        .send_test_email(&user.email, language)
        .await?;
    Ok(Json(EmailTest {
        sent_to: user.email,
        error,
    }))
}

#[utoipa::path(get, path = "/admin/settings", tag = "admin", responses((status = 200, body = InstanceSettings)))]
async fn get_instance_settings(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<InstanceSettings>> {
    instance_settings(&state).await.map(Json)
}

#[utoipa::path(patch, path = "/admin/settings", tag = "admin", request_body = UpdateInstanceSettings, responses((status = 200, body = InstanceSettings)))]
async fn update_instance_settings(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Json(body): Json<UpdateInstanceSettings>,
) -> AppResult<Json<InstanceSettings>> {
    if body.email_code_sign_in.is_none() && body.workspace_creation_restricted.is_none() {
        return Err(AppError::Validation("Name a setting to change".into()));
    }
    state
        .instance_settings_repo
        .update(body.email_code_sign_in, body.workspace_creation_restricted)
        .await?;
    let mut details = serde_json::Map::new();
    if let Some(on) = body.email_code_sign_in {
        details.insert("email_code_sign_in".into(), on.into());
    }
    if let Some(on) = body.workspace_creation_restricted {
        details.insert("workspace_creation_restricted".into(), on.into());
    }
    audit::record(
        &state,
        AuditEntry::new(AuditAction::InstanceSettingsChanged, auth.user_id)
            .ip(&ip)
            .details(details.into()),
    )
    .await;
    instance_settings(&state).await.map(Json)
}

async fn instance_settings(state: &AppState) -> AppResult<InstanceSettings> {
    Ok(InstanceSettings {
        email_code_sign_in: state.instance_settings_repo.email_code_sign_in().await?,
        email_configured: state.auth_service.can_send_email(),
        workspace_creation_restricted: state
            .instance_settings_repo
            .workspace_creation_restricted()
            .await?,
    })
}

#[utoipa::path(get, path = "/admin/stats", tag = "admin", responses((status = 200, body = InstanceStats)))]
async fn stats(State(state): State<Arc<AppState>>) -> AppResult<Json<InstanceStats>> {
    let user_count =
        sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM users WHERE NOT is_bot"#)
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
    pub instance_role: InstanceRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(get, path = "/admin/users", tag = "admin", params(PageQuery), responses((status = 200, body = DataList<AdminUser>)))]
async fn list_users(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PageQuery>,
) -> AppResult<Json<DataList<AdminUser>>> {
    let users = sqlx::query!(
        r#"SELECT id, email, display_name, status::text AS "status!",
                  is_instance_admin AS "is_instance_admin!",
                  workspace_creator AS "workspace_creator!", created_at
             FROM users WHERE NOT is_bot ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
        params.limit(),
        params.offset()
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|row| AdminUser {
        id: row.id,
        email: row.email,
        display_name: row.display_name,
        status: row.status,
        instance_role: InstanceRole::of(row.is_instance_admin, row.workspace_creator),
        created_at: row.created_at,
    })
    .collect::<Vec<_>>();

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
    state
        .integration_repo
        .delete_user_connections(user_id)
        .await?;
    crate::workspace::membership::suspend_creations(&state, user_id, auth.user_id, &ip).await;

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
    state.auth_service.repo().reactivate(user_id).await?;

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
    pub role: InstanceRole,
}

#[utoipa::path(patch, path = "/admin/users/{user_id}/instance-role", tag = "admin", request_body = UpdateInstanceRoleRequest, responses((status = 200, body = InstanceRoleResponse)))]
async fn update_instance_role(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(user_id): Path<Uuid>,
    Json(body): Json<UpdateInstanceRoleRequest>,
) -> AppResult<Json<InstanceRoleResponse>> {
    let updated = sqlx::query!(
        "UPDATE users SET is_instance_admin = $1, workspace_creator = $2, updated_at = NOW()
          WHERE id = $3 AND NOT is_bot",
        body.role == InstanceRole::Admin,
        body.role == InstanceRole::WorkspaceCreator,
        user_id
    )
    .execute(&state.pool)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }
    audit::record(
        &state,
        AuditEntry::new(AuditAction::InstanceRoleChanged, auth.user_id)
            .resource(user_id)
            .ip(&ip)
            .details(serde_json::json!({ "role": body.role })),
    )
    .await;
    let told = serde_json::json!({ "user_id": user_id, "role": body.role });
    if let Err(e) = state
        .publisher
        .publish("user.instance_role_changed", told)
        .await
    {
        tracing::warn!("failed to tell {user_id} their instance role changed: {e}");
    }
    Ok(Json(InstanceRoleResponse { role: body.role }))
}

#[utoipa::path(
    operation_id = "admin_list_workspaces",
    get, path = "/admin/workspaces", tag = "admin", params(PageQuery), responses((status = 200, body = DataList<AdminWorkspace>)))]
async fn list_workspaces(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PageQuery>,
) -> AppResult<Json<DataList<AdminWorkspace>>> {
    let data = state
        .workspace_service
        .repo
        .list_for_instance(params.limit(), params.offset())
        .await?;
    Ok(Json(DataList { data }))
}

#[utoipa::path(
    operation_id = "admin_list_workspace_people",
    get, path = "/admin/workspaces/{ws_id}/members", tag = "admin", responses((status = 200, body = DataList<WorkspacePerson>)))]
async fn list_workspace_people(
    State(state): State<Arc<AppState>>,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<WorkspacePerson>>> {
    let people = state
        .workspace_service
        .repo
        .people_for_instance(ws_id)
        .await?;
    Ok(Json(people.into()))
}

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct AppointOwnerRequest {
    pub user_id: Uuid,
}

#[utoipa::path(
    operation_id = "admin_appoint_workspace_owner",
    post, path = "/admin/workspaces/{ws_id}/owners", tag = "admin", request_body = AppointOwnerRequest, responses((status = 200, body = WorkspaceMember)))]
async fn appoint_workspace_owner(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(body): Json<AppointOwnerRequest>,
) -> AppResult<Json<WorkspaceMember>> {
    let appointed = ownership::appoint(&state, ws_id, body.user_id).await?;
    if appointed.was != WorkspaceRole::Owner {
        audit::record(
            &state,
            AuditEntry::new(AuditAction::WorkspaceRoleChanged, auth.user_id)
                .workspace(ws_id)
                .resource(body.user_id)
                .ip(&ip)
                .details(serde_json::json!({
                    "from": appointed.was,
                    "to": WorkspaceRole::Owner,
                    "instance_admin": true,
                })),
        )
        .await;
    }
    Ok(Json(appointed.member))
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
