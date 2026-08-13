use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use shared_common::errors::{AppError, AppResult};

use super::models::*;
use crate::audit::{self, AuditAction, AuditEntry};
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::{ChannelRole, WorkspaceRole};
use crate::workspace::repo::WorkspaceRepo;

pub fn router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/users/me", get(get_me))
        .route("/users/me", patch(update_me))
        .route("/users/me/password", patch(change_password));

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
        .merge(public.with_state(state.clone()))
        .merge(crate::protected(state, protected))
}

async fn login(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    peer: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> AppResult<(CookieJar, Json<AuthSession>)> {
    let key = format!("rate_limit:login:{}", req.email.to_lowercase());
    let window = state.config.login_attempts_window_secs;
    check_rate_limit(&state, &key, state.config.login_attempts_per_email, window).await?;
    if let Some(ip) = crate::net::client_ip(
        &headers,
        peer.map(|p| p.0),
        &crate::net::parse_trusted_proxies(&state.config.trusted_proxies),
    ) {
        check_rate_limit(
            &state,
            &format!("rate_limit:login_ip:{ip}"),
            state.config.login_attempts_per_ip,
            window,
        )
        .await?;
    }

    let user = state
        .auth_service
        .verify_password_only(&req.email, &req.password)
        .await?;

    // The password is only the first half now. Tokens are minted after the
    // second factor, never before — a step-up that hands out a session first has
    // not stepped anything up.
    let enrolment = state.totp_repo.find(user.id).await?;
    let requires_totp = enrolment.as_ref().is_some_and(|e| e.is_active());

    if state.config.require_admin_totp && user.is_instance_admin && !requires_totp {
        return Err(AppError::Forbidden(
            "This instance requires admins to enrol a second factor before signing in".into(),
        ));
    }

    if requires_totp {
        let Some(code) = req.totp_code.as_deref().filter(|c| !c.is_empty()) else {
            // A distinct answer on purpose: the password was right, and the
            // client needs to know to ask for the code rather than for it again.
            return Err(AppError::Conflict("totp_required".into()));
        };

        let enrolment = enrolment.expect("checked above");
        let secret = crate::auth::totp::decrypt_secret(
            &state.config.jwt_secret,
            &enrolment.secret_encrypted,
            &enrolment.nonce,
        )?;

        let accepted = if crate::auth::totp::verify(
            &secret,
            &user.email,
            &state.config.instance_name,
            code,
        )? {
            // A correct code is only accepted once: without claiming the step,
            // the same six digits work for the rest of their window.
            let step = crate::auth::totp::current_step(chrono::Utc::now().timestamp());
            state.totp_repo.claim_step(user.id, step).await?
        } else {
            consume_recovery_code(&state, user.id, code).await?
        };

        if !accepted {
            audit::record(
                &state,
                AuditEntry::new(AuditAction::TotpFailed, user.id)
                    .resource(user.id)
                    .details(serde_json::json!({ "stage": "login" })),
            )
            .await;
            return Err(AppError::Unauthorized("That code did not match".into()));
        }
    }

    let tokens = state.auth_service.tokens_for(&user).await?;
    let secure = state.config.public_url.starts_with("https://");
    let jar = set_auth_cookies(jar, &tokens, secure);

    Ok((jar, Json(tokens.into())))
}

/// A recovery code stands in for the authenticator exactly once.
async fn consume_recovery_code(
    state: &AppState,
    user_id: uuid::Uuid,
    candidate: &str,
) -> AppResult<bool> {
    for (id, hash) in state.totp_repo.unused_recovery_hashes(user_id).await? {
        if crate::auth::service::AuthService::verify_password(candidate, &hash).unwrap_or(false)
            && state.totp_repo.consume_recovery_code(id).await?
        {
            audit::record(
                state,
                AuditEntry::new(AuditAction::TotpRecoveryUsed, user_id).resource(user_id),
            )
            .await;
            return Ok(true);
        }
    }
    Ok(false)
}

async fn verify_invite(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let claims = state.auth_service.verify_registration_token(&token)?;

    let invalid = || AppError::Unauthorized("Invalid or expired invite".into());

    let user = state
        .auth_service
        .repo()
        .find_by_id(claims.sub)
        .await?
        .ok_or_else(invalid)?;

    let workspace_id = claims.workspace_id.ok_or_else(invalid)?;

    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(workspace_id)
        .await?
        .ok_or_else(invalid)?;

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

        let mut tx = state.workspace_service.repo.begin().await?;

        WorkspaceRepo::add_member_tx(&mut tx, workspace_id, claims.sub, &role).await?;

        let channels = WorkspaceRepo::list_default_channels_tx(&mut tx, workspace_id).await?;

        for ch in channels {
            WorkspaceRepo::add_channel_member_tx(&mut tx, ch.id, claims.sub, &ChannelRole::Member)
                .await?;
        }

        tx.commit().await?;
    }

    let secure = state.config.public_url.starts_with("https://");
    let jar = set_auth_cookies(jar, &tokens, secure);

    Ok((jar, Json(tokens.into())))
}

async fn refresh(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
) -> AppResult<(CookieJar, Json<AuthSession>)> {
    let refresh_token = jar
        .get("refresh_token")
        .map(|c| c.value().to_string())
        .or_else(|| bearer_token(&headers))
        .ok_or_else(|| AppError::Unauthorized("No refresh token".into()))?;

    let tokens = state
        .auth_service
        .refresh_access_token(&refresh_token)
        .await?;
    let secure = state.config.public_url.starts_with("https://");
    let jar = set_auth_cookies(jar, &tokens, secure);

    Ok((jar, Json(tokens.into())))
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
    let user_id = state
        .auth_service
        .reset_password(&req.token, &req.password)
        .await?;

    // A reset is how somebody recovers a compromised account. Leaving the
    // attacker's still-unexpired access token alive would defeat the point.
    crate::sessions::revoke(
        &state,
        user_id,
        crate::sessions::SessionScope::All,
        "password reset",
    )
    .await?;

    Ok(Json(serde_json::json!({ "status": "reset" })))
}

async fn instance_info(state: Arc<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": state.config.instance_name,
        "icon_url": state.config.instance_icon_url,
        "sso_enabled": super::oidc_routes::settings_from(&state.config).is_configured()
            && super::oidc::Provisioning::parse(
                &state.config.oidc_provisioning,
                &state.config.oidc_allowed_domains,
            )
            .may_sign_in(),
    }))
}

async fn get_me(State(state): State<Arc<AppState>>, auth: AuthUser) -> AppResult<Json<UserPublic>> {
    let user = state
        .auth_service
        .repo()
        .find_by_id(auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    Ok(Json(user.into()))
}

async fn update_me(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<UpdateProfileRequest>,
) -> AppResult<Json<UserPublic>> {
    if let Some(name) = &req.display_name {
        shared_common::validation::validate_display_name(name)?;
    }
    if let Some(avatar_url) = req.avatar_url.as_deref().filter(|url| !url.is_empty()) {
        shared_common::validation::validate_avatar_url(avatar_url)?;
    }
    if let Some(bio) = &req.bio {
        shared_common::validation::validate_bio(bio)?;
    }
    if let Some(timezone) = &req.timezone {
        shared_common::validation::validate_timezone(timezone)?;
    }
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
        .await?;
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

    // Every other device is signed out; the one doing the change is not, or
    // people learn to avoid changing their password.
    let scope = match auth.jti {
        Some(jti) => crate::sessions::SessionScope::AllExcept(jti),
        None => crate::sessions::SessionScope::All,
    };
    crate::sessions::revoke(&state, auth.user_id, scope, "password changed").await?;

    Ok(Json(serde_json::json!({ "status": "password_changed" })))
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(std::string::ToString::to_string)
}

pub(crate) fn set_auth_cookies(jar: CookieJar, tokens: &AuthTokens, secure: bool) -> CookieJar {
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
    // Fail closed: a Redis outage must not silently remove brute-force
    // protection from the one endpoint that guards passwords.
    crate::rate_limit::enforce(
        &mut conn,
        key,
        max_attempts,
        window_secs,
        crate::rate_limit::LimiterFailure::Closed,
    )
    .await
}
