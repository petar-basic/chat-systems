use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use rand::RngExt;
use shared_common::errors::{AppError, AppResult};
use shared_common::validation;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::models::{AuthSession, User, UserStatus};
use super::routes::{check_rate_limit, limit_tries, second_factor, set_auth_cookies};
use super::service::AuthService;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::dto::StatusResponse;
use crate::state::AppState;

pub const CODE_LIFETIME_MINS: i32 = 10;
pub const MAX_TRIES: i32 = 5;
pub const DAILY_CEILING: i32 = 20;

pub fn public_router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(request_email_code))
        .routes(routes!(verify_email_code))
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct EmailCodeRequest {
    pub email: String,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct VerifyEmailCodeRequest {
    pub email: String,
    pub code: String,
    #[serde(default)]
    pub totp_code: Option<String>,
}

pub struct CheckedCode {
    pub user: User,
    pub code_hash: String,
}

pub(crate) async fn offered(state: &AppState) -> AppResult<bool> {
    Ok(state.auth_service.can_send_email()
        && state.instance_settings_repo.email_code_sign_in().await?)
}

#[utoipa::path(post, path = "/auth/email-code", tag = "auth", request_body = EmailCodeRequest, responses((status = 200, body = StatusResponse)), security(()))]
async fn request_email_code(
    State(state): State<Arc<AppState>>,
    ip: ClientIp,
    Json(req): Json<EmailCodeRequest>,
) -> AppResult<Json<StatusResponse>> {
    refuse_when_off(&state).await?;
    validation::validate_email(&req.email)?;
    check_rate_limit(
        &state,
        &format!("rate_limit:email_code:{}", req.email.to_lowercase()),
        5,
        900,
    )
    .await?;
    if let Some(ip) = ip.0.as_deref() {
        check_rate_limit(
            &state,
            &format!("rate_limit:email_code_ip:{ip}"),
            state.config.login_attempts_per_ip,
            900,
        )
        .await?;
    }

    tokio::spawn(async move {
        if let Err(e) = answer_request(&state, &req.email, &ip).await {
            tracing::warn!("failed to answer a sign-in code request: {}", e);
        }
    });

    Ok(Json(StatusResponse::new("sent")))
}

async fn answer_request(state: &AppState, email: &str, ip: &ClientIp) -> AppResult<()> {
    let Some(user) = state.auth_service.repo().find_by_email(email).await? else {
        return Ok(());
    };

    match user.status {
        UserStatus::Pending => {
            state
                .workspace_service
                .resend_invite(&user.email, &state.auth_service)
                .await?
        }
        UserStatus::Suspended => {}
        UserStatus::Active if user.password_hash.is_none() => {
            state.auth_service.send_sso_reminder(&user.email).await?
        }
        UserStatus::Active => {
            if !state
                .email_code_repo
                .locked_out(user.id, DAILY_CEILING)
                .await?
            {
                send_code(state, &user, ip).await?
            }
        }
    }
    Ok(())
}

#[utoipa::path(post, path = "/auth/email-code/verify", tag = "auth", request_body = VerifyEmailCodeRequest, responses((status = 200, body = AuthSession)), security(()))]
async fn verify_email_code(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    ip: ClientIp,
    Json(req): Json<VerifyEmailCodeRequest>,
) -> AppResult<(CookieJar, Json<AuthSession>)> {
    let checked = check_code(&state, &ip, &req.email, &req.code).await?;
    second_factor(&state, &checked.user, req.totp_code.as_deref()).await?;
    spend_code(&state, &ip, &checked, "sign_in").await?;

    let tokens = state.auth_service.tokens_for(&checked.user).await?;
    let secure = state.config.public_url.starts_with("https://");
    let jar = set_auth_cookies(jar, &tokens, secure);
    Ok((jar, Json(tokens.into())))
}

pub(crate) async fn check_code(
    state: &AppState,
    ip: &ClientIp,
    email: &str,
    code: &str,
) -> AppResult<CheckedCode> {
    refuse_when_off(state).await?;
    limit_tries(state, "email_code_try", ip.0.as_deref(), email).await?;

    let user = state
        .auth_service
        .repo()
        .find_by_email(email)
        .await?
        .filter(|u| u.status == UserStatus::Active && u.password_hash.is_some())
        .ok_or_else(wrong_code)?;
    if state
        .email_code_repo
        .locked_out(user.id, DAILY_CEILING)
        .await?
    {
        return Err(wrong_code());
    }
    let attempt = state
        .email_code_repo
        .claim_try(user.id, MAX_TRIES)
        .await?
        .ok_or_else(wrong_code)?;

    if !AuthService::verify_password(code.trim(), &attempt.code_hash).unwrap_or(false) {
        state.email_code_repo.record_failure(user.id).await?;
        if attempt.attempts >= MAX_TRIES {
            audit::record(
                state,
                AuditEntry::new(AuditAction::EmailCodeExhausted, user.id)
                    .resource(user.id)
                    .ip(ip),
            )
            .await;
        }
        return Err(wrong_code());
    }

    Ok(CheckedCode {
        user,
        code_hash: attempt.code_hash,
    })
}

pub(crate) async fn spend_code(
    state: &AppState,
    ip: &ClientIp,
    checked: &CheckedCode,
    stage: &str,
) -> AppResult<()> {
    if !state
        .email_code_repo
        .consume(checked.user.id, &checked.code_hash)
        .await?
    {
        return Err(wrong_code());
    }
    audit::record(
        state,
        AuditEntry::new(AuditAction::EmailCodeUsed, checked.user.id)
            .resource(checked.user.id)
            .ip(ip)
            .details(serde_json::json!({ "stage": stage })),
    )
    .await;
    Ok(())
}

async fn send_code(state: &AppState, user: &User, ip: &ClientIp) -> AppResult<()> {
    let code = format!("{:06}", rand::rng().random_range(0..1_000_000u32));
    let hash = AuthService::hash_password(&code)?;
    state
        .email_code_repo
        .replace(user.id, &hash, CODE_LIFETIME_MINS)
        .await?;
    if let Err(e) = state
        .auth_service
        .send_sign_in_code(&user.email, &code, CODE_LIFETIME_MINS)
        .await
    {
        tracing::warn!("failed to send a sign-in code: {}", e);
        return Ok(());
    }
    audit::record(
        state,
        AuditEntry::new(AuditAction::EmailCodeSent, user.id)
            .resource(user.id)
            .ip(ip),
    )
    .await;
    Ok(())
}

async fn refuse_when_off(state: &AppState) -> AppResult<()> {
    if offered(state).await? {
        return Ok(());
    }
    Err(AppError::BadRequest(
        "Signing in with an emailed code is off on this instance".into(),
    ))
}

fn wrong_code() -> AppError {
    AppError::Unauthorized("That code is wrong or has expired".into())
}
