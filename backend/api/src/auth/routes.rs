use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, patch, post};
use axum::{middleware, Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use shared_common::errors::{AppError, AppResult};

use super::models::*;
use crate::middleware::{auth_middleware, AuthUser};
use crate::state::AppState;
use crate::workspace::models::{ChannelRole, WorkspaceRole};
use crate::workspace::repo::WorkspaceRepo;

pub fn router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/users/me", get(get_me))
        .route("/users/me", patch(update_me))
        .route("/users/me/password", patch(change_password))
        .layer(middleware::from_fn(auth_middleware));

    let public = Router::new()
        .route("/auth/login", post(login))
        .route("/auth/invites/:token/verify", get(verify_invite))
        .route("/auth/complete-registration", post(complete_registration))
        .route("/auth/refresh", post(refresh))
        .route("/auth/logout", post(logout))
        .route("/auth/forgot-password", post(forgot_password))
        .route("/auth/reset-password", post(reset_password))
        .route(
            "/instance/info",
            get({
                let s = state.clone();
                move || instance_info(s)
            }),
        );

    Router::new()
        .merge(public)
        .merge(protected)
        .with_state(state)
}

async fn login(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> AppResult<(CookieJar, Json<AuthSession>)> {
    let key = format!("rate_limit:login:{}", req.email.to_lowercase());
    check_rate_limit(&state, &key, 10, 900).await?;

    let tokens = state.auth_service.login(&req.email, &req.password).await?;
    let secure = state.config.public_url.starts_with("https://");
    let jar = set_auth_cookies(jar, &tokens, secure);

    Ok((
        jar,
        Json(AuthSession {
            access_token: tokens.access_token.clone(),
            user: tokens.user,
            expires_in: tokens.expires_in,
        }),
    ))
}

async fn verify_invite(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let claims = state.auth_service.verify_registration_token(&token)?;

    let user = state
        .auth_service
        .repo()
        .find_by_id(claims.sub)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Invite is no longer valid".into()))?;

    let workspace_id = claims
        .workspace_id
        .ok_or_else(|| AppError::BadRequest("Invalid or expired invite".into()))?;

    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(workspace_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Workspace no longer exists".into()))?;

    Ok(Json(serde_json::json!({
        "email": user.email,
        "workspace_name": workspace.name,
        "workspace_id": workspace_id,
    })))
}

async fn complete_registration(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(req): Json<RegisterCompleteRequest>,
) -> AppResult<(CookieJar, Json<AuthSession>)> {
    let claims = state.auth_service.verify_registration_token(&req.token)?;

    let tokens = state
        .auth_service
        .complete_registration(claims.sub, &req.password, &req.display_name)
        .await?;

    if let Some(workspace_id) = claims.workspace_id {
        let role: WorkspaceRole = claims
            .invite_role
            .as_deref()
            .and_then(|r| serde_json::from_value(serde_json::Value::String(r.to_string())).ok())
            .unwrap_or(WorkspaceRole::Member);

        let mut tx = state
            .workspace_service
            .repo
            .begin()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        WorkspaceRepo::add_member_tx(&mut tx, workspace_id, claims.sub, &role)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        let channels = WorkspaceRepo::list_default_channels_tx(&mut tx, workspace_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        for ch in channels {
            WorkspaceRepo::add_channel_member_tx(&mut tx, ch.id, claims.sub, &ChannelRole::Member)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
    }

    let secure = state.config.public_url.starts_with("https://");
    let jar = set_auth_cookies(jar, &tokens, secure);

    Ok((
        jar,
        Json(AuthSession {
            access_token: tokens.access_token.clone(),
            user: tokens.user,
            expires_in: tokens.expires_in,
        }),
    ))
}

async fn refresh(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> AppResult<(CookieJar, Json<AuthSession>)> {
    let refresh_token = jar
        .get("refresh_token")
        .map(|c| c.value().to_string())
        .ok_or_else(|| AppError::Unauthorized("No refresh token cookie".into()))?;

    let tokens = state
        .auth_service
        .refresh_access_token(&refresh_token)
        .await?;
    let secure = state.config.public_url.starts_with("https://");
    let jar = set_auth_cookies(jar, &tokens, secure);

    Ok((
        jar,
        Json(AuthSession {
            access_token: tokens.access_token.clone(),
            user: tokens.user,
            expires_in: tokens.expires_in,
        }),
    ))
}

async fn logout(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> AppResult<(CookieJar, Json<serde_json::Value>)> {
    if let Some(cookie) = jar.get("refresh_token") {
        let _ = state.auth_service.logout(cookie.value()).await;
    }
    let jar = clear_auth_cookies(jar);
    Ok((jar, Json(serde_json::json!({ "status": "logged_out" }))))
}

async fn forgot_password(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ForgotPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let key = format!("rate_limit:forgot:{}", req.email.to_lowercase());
    check_rate_limit(&state, &key, 5, 900).await?;
    state.auth_service.forgot_password(&req.email).await?;
    Ok(Json(serde_json::json!({ "status": "sent" })))
}

async fn reset_password(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ResetPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state
        .auth_service
        .reset_password(&req.token, &req.password)
        .await?;
    Ok(Json(serde_json::json!({ "status": "reset" })))
}

async fn instance_info(state: Arc<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": state.config.instance_name,
        "icon_url": state.config.instance_icon_url,
    }))
}

async fn get_me(State(state): State<Arc<AppState>>, auth: AuthUser) -> AppResult<Json<UserPublic>> {
    let user = state
        .auth_service
        .repo()
        .find_by_id(auth.user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    Ok(Json(user.into()))
}

async fn update_me(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<UpdateProfileRequest>,
) -> AppResult<Json<UserPublic>> {
    let user = state
        .auth_service
        .repo()
        .update_profile(
            auth.user_id,
            req.display_name.as_deref(),
            req.avatar_url.as_deref(),
            req.bio.as_deref(),
            req.timezone.as_deref(),
        )
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(user.into()))
}

async fn change_password(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state
        .auth_service
        .change_password(auth.user_id, &req.current_password, &req.new_password)
        .await?;
    Ok(Json(serde_json::json!({ "status": "password_changed" })))
}

fn set_auth_cookies(jar: CookieJar, tokens: &AuthTokens, secure: bool) -> CookieJar {
    let access = Cookie::build(("access_token", tokens.access_token.clone()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .secure(secure)
        .max_age(time::Duration::seconds(tokens.expires_in))
        .build();

    let refresh = Cookie::build(("refresh_token", tokens.refresh_token.clone()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/api/auth")
        .secure(secure)
        .max_age(time::Duration::days(7))
        .build();

    jar.add(access).add(refresh)
}

fn clear_auth_cookies(jar: CookieJar) -> CookieJar {
    let access = Cookie::build(("access_token", ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    let refresh = Cookie::build(("refresh_token", ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/api/auth")
        .max_age(time::Duration::seconds(0))
        .build();

    jar.add(access).add(refresh)
}

async fn check_rate_limit(
    state: &AppState,
    key: &str,
    max_attempts: u64,
    window_secs: u64,
) -> AppResult<()> {
    let mut conn = state.redis.clone();
    crate::rate_limit::enforce(&mut conn, key, max_attempts, window_secs).await
}
