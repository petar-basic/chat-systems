use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};

use shared_common::errors::{AppError, AppResult};

use super::service::AuthService;
use super::totp;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/auth/totp", get(status))
        .route("/auth/totp/enrol", post(enrol))
        .route("/auth/totp/confirm", post(confirm))
        .route("/auth/totp/disable", post(disable));

    crate::protected(state, routes)
}

async fn status(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let enrolment = state.totp_repo.find(auth.user_id).await?;
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM totp_recovery_codes WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "enrolled": enrolment.map(|e| e.is_active()).unwrap_or(false),
        "recovery_codes_remaining": remaining,
        "required": state.config.require_admin_totp,
    })))
}

#[derive(serde::Deserialize)]
pub struct CodeRequest {
    pub code: String,
}

/// Hands back a secret and its provisioning URI. Nothing is enforced yet: the
/// enrolment is not active until a code proves the authenticator actually has
/// it, or people lock themselves out of accounts they never finished setting up.
async fn enrol(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let user = state
        .auth_service
        .repo()
        .find_by_id(auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    let secret = totp::generate_secret();
    let (ciphertext, nonce) = totp::encrypt_secret(&state.config.jwt_secret, &secret)?;
    state
        .totp_repo
        .start_enrolment(auth.user_id, &ciphertext, &nonce)
        .await?;

    let uri = totp::provisioning_uri(&secret, &user.email, &state.config.instance_name)?;
    Ok(Json(serde_json::json!({
        "secret": secret,
        "provisioning_uri": uri,
    })))
}

async fn confirm(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Json(req): Json<CodeRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let user = state
        .auth_service
        .repo()
        .find_by_id(auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    let enrolment = state
        .totp_repo
        .find(auth.user_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Start an enrolment first".into()))?;

    let secret = totp::decrypt_secret(
        &state.config.jwt_secret,
        &enrolment.secret_encrypted,
        &enrolment.nonce,
    )?;
    let Some(step) = totp::matching_step(
        &secret,
        &user.email,
        &state.config.instance_name,
        &req.code,
        chrono::Utc::now().timestamp(),
    )?
    else {
        audit::record(
            &state,
            AuditEntry::new(AuditAction::TotpFailed, auth.user_id)
                .resource(auth.user_id)
                .ip(&ip)
                .details(serde_json::json!({ "stage": "enrolment" })),
        )
        .await;
        return Err(AppError::Unauthorized("That code did not match".into()));
    };

    state.totp_repo.confirm(auth.user_id, step).await?;

    // Shown once. They are the way back in when the phone is gone, so they are
    // hashed exactly like a password and never retrievable afterwards.
    let codes = totp::generate_recovery_codes();
    let hashes: Vec<String> = codes
        .iter()
        .filter_map(|c| AuthService::hash_password(c).ok())
        .collect();
    state
        .totp_repo
        .replace_recovery_codes(auth.user_id, &hashes)
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::TotpEnrolled, auth.user_id)
            .resource(auth.user_id)
            .ip(&ip),
    )
    .await;

    Ok(Json(serde_json::json!({
        "status": "enrolled",
        "recovery_codes": codes,
    })))
}

async fn disable(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Json(req): Json<CodeRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let user = state
        .auth_service
        .repo()
        .find_by_id(auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    // An instance admin cannot take their own second factor off while the
    // instance requires one: that would be a supported way back to the weaker
    // account the requirement exists to prevent.
    if state.config.require_admin_totp && user.is_instance_admin {
        return Err(AppError::Forbidden(
            "This instance requires a second factor for admins".into(),
        ));
    }

    let enrolment = state
        .totp_repo
        .find(auth.user_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("No second factor is enrolled".into()))?;
    let secret = totp::decrypt_secret(
        &state.config.jwt_secret,
        &enrolment.secret_encrypted,
        &enrolment.nonce,
    )?;

    // Proving possession before removal: an attacker with a live session should
    // not be able to strip the factor that would have stopped them.
    if !totp::verify(&secret, &user.email, &state.config.instance_name, &req.code)? {
        return Err(AppError::Unauthorized("That code did not match".into()));
    }

    state.totp_repo.disable(auth.user_id).await?;
    audit::record(
        &state,
        AuditEntry::new(AuditAction::TotpDisabled, auth.user_id)
            .resource(auth.user_id)
            .ip(&ip),
    )
    .await;

    Ok(Json(serde_json::json!({ "status": "disabled" })))
}
