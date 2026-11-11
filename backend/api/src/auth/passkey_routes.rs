use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use chrono::{DateTime, Utc};
use shared_common::errors::{AppError, AppResult};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;
use webauthn_rs::prelude::{
    AuthenticationResult, Passkey, PublicKeyCredential, RegisterPublicKeyCredential, WebauthnError,
};

use super::email_code_routes;
use super::models::{AuthSession, User, UserStatus};
use super::oidc_routes::issue_desktop_code;
use super::passkey::{self, Ceremony};
use super::passkey_repo::StoredPasskey;
use super::routes::{check_rate_limit, first_factor, set_auth_cookies};
use super::service::AuthService;
use super::totp;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::dto::StatusResponse;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_passkeys, add_passkey))
        .routes(routes!(start_passkey_registration))
        .routes(routes!(rename_passkey))
        .routes(routes!(remove_passkey))
}

pub fn public_router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(start_passkey_sign_in))
        .routes(routes!(passkey_sign_in))
        .routes(routes!(passkey_sign_in_for_desktop))
        .routes(routes!(start_passkey_second_factor))
        .routes(routes!(passkey_second_factor))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PasskeySummary {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl From<StoredPasskey> for PasskeySummary {
    fn from(row: StoredPasskey) -> Self {
        Self {
            id: row.id,
            name: row.name,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
        }
    }
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PasskeyChallenge {
    pub challenge_id: Uuid,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub options: serde_json::Value,
}

#[derive(Debug, Default, serde::Deserialize, utoipa::ToSchema)]
pub struct PresenceProof {
    pub password: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct AddPasskeyRequest {
    pub challenge_id: Uuid,
    pub name: String,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub credential: RegisterPublicKeyCredential,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct RenamePasskeyRequest {
    pub name: String,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct PasskeyAssertion {
    pub challenge_id: Uuid,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub credential: PublicKeyCredential,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct DesktopPasskeyCode {
    pub code: String,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct PasskeySecondFactorStart {
    pub email: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub email_code: Option<String>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PasskeyList {
    pub offered: bool,
    pub data: Vec<PasskeySummary>,
}

#[utoipa::path(get, path = "/auth/passkeys", tag = "passkeys", responses((status = 200, body = PasskeyList)))]
async fn list_passkeys(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<PasskeyList>> {
    let user = load_user(&state, auth.user_id).await?;
    let rows = state.passkey_repo.list(user.id).await?;
    Ok(Json(PasskeyList {
        offered: user.password_hash.is_some(),
        data: rows.into_iter().map(PasskeySummary::from).collect(),
    }))
}

#[utoipa::path(post, path = "/auth/passkeys/registration", tag = "passkeys", request_body = PresenceProof, responses((status = 200, body = PasskeyChallenge)))]
async fn start_passkey_registration(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(proof): Json<PresenceProof>,
) -> AppResult<Json<PasskeyChallenge>> {
    let user = load_user(&state, auth.user_id).await?;
    refuse_sso(&user)?;
    confirm_presence(&state, &user, &proof).await?;

    let existing = state.passkey_repo.list(user.id).await?;
    if existing.len() as i64 >= passkey::MAX_PER_ACCOUNT {
        return Err(too_many());
    }
    let exclude = existing
        .iter()
        .map(|row| row.credential.0.cred_id().clone())
        .collect();

    let display_name = user.display_name.as_deref().unwrap_or(&user.email);
    let (challenge, registration) = passkey::relying_party(&state.config)?
        .start_passkey_registration(user.id, &user.email, display_name, Some(exclude))
        .map_err(rejected)?;

    let mut options = passkey::browser_options(&challenge)?;
    passkey::require_discoverable(&mut options);
    let challenge_id = passkey::stash(
        &state,
        &Ceremony::Registration {
            user_id: user.id,
            state: registration,
        },
    )
    .await?;

    Ok(Json(PasskeyChallenge {
        challenge_id,
        options,
    }))
}

#[utoipa::path(post, path = "/auth/passkeys", tag = "passkeys", request_body = AddPasskeyRequest, responses((status = 200, body = PasskeySummary)))]
async fn add_passkey(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Json(req): Json<AddPasskeyRequest>,
) -> AppResult<Json<PasskeySummary>> {
    let name = passkey_name(&req.name)?;
    let Ceremony::Registration {
        user_id,
        state: registration,
    } = passkey::take(&state, req.challenge_id).await?
    else {
        return Err(expired());
    };
    if user_id != auth.user_id {
        return Err(expired());
    }

    let key = passkey::relying_party(&state.config)?
        .finish_passkey_registration(&req.credential, &registration)
        .map_err(rejected)?;

    let row = state
        .passkey_repo
        .insert(user_id, &key, name, passkey::MAX_PER_ACCOUNT)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                AppError::Conflict("That passkey is already added".into())
            }
            e => e.into(),
        })?
        .ok_or_else(too_many)?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::PasskeyAdded, user_id)
            .resource(user_id)
            .ip(&ip)
            .details(serde_json::json!({ "passkey_id": row.id, "name": row.name })),
    )
    .await;

    Ok(Json(row.into()))
}

#[utoipa::path(patch, path = "/auth/passkeys/{id}", tag = "passkeys", request_body = RenamePasskeyRequest, responses((status = 200, body = StatusResponse)))]
async fn rename_passkey(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<RenamePasskeyRequest>,
) -> AppResult<Json<StatusResponse>> {
    let name = passkey_name(&req.name)?;
    if !state.passkey_repo.rename(id, auth.user_id, name).await? {
        return Err(AppError::NotFound("Passkey not found".into()));
    }
    Ok(Json(StatusResponse::new("renamed")))
}

#[utoipa::path(post, path = "/auth/passkeys/{id}/remove", tag = "passkeys", request_body = PresenceProof, responses((status = 200, body = StatusResponse)))]
async fn remove_passkey(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(id): Path<Uuid>,
    Json(proof): Json<PresenceProof>,
) -> AppResult<Json<StatusResponse>> {
    let user = load_user(&state, auth.user_id).await?;
    confirm_presence(&state, &user, &proof).await?;

    let rows = state.passkey_repo.list(user.id).await?;
    if !rows.iter().any(|row| row.id == id) {
        return Err(AppError::NotFound("Passkey not found".into()));
    }
    let has_totp = state
        .totp_repo
        .find(user.id)
        .await?
        .is_some_and(|e| e.is_active());
    if state.config.require_admin_totp && user.is_instance_admin && !has_totp && rows.len() == 1 {
        return Err(AppError::Forbidden(
            "This instance requires a second factor for admins".into(),
        ));
    }

    if !state.passkey_repo.remove(id, user.id).await? {
        return Err(AppError::NotFound("Passkey not found".into()));
    }
    audit::record(
        &state,
        AuditEntry::new(AuditAction::PasskeyRemoved, user.id)
            .resource(user.id)
            .ip(&ip)
            .details(serde_json::json!({ "passkey_id": id })),
    )
    .await;

    Ok(Json(StatusResponse::new("removed")))
}

#[utoipa::path(post, path = "/auth/passkeys/sign-in/start", tag = "passkeys", responses((status = 200, body = PasskeyChallenge)), security(()))]
async fn start_passkey_sign_in(
    State(state): State<Arc<AppState>>,
    ip: ClientIp,
) -> AppResult<Json<PasskeyChallenge>> {
    limit_by_ip(&state, &ip).await?;
    let (challenge, sign_in) = passkey::relying_party(&state.config)?
        .start_discoverable_authentication()
        .map_err(rejected)?;
    let options = passkey::browser_options(&challenge)?;
    let challenge_id = passkey::stash(&state, &Ceremony::SignIn { state: sign_in }).await?;
    Ok(Json(PasskeyChallenge {
        challenge_id,
        options,
    }))
}

#[utoipa::path(post, path = "/auth/passkeys/sign-in", tag = "passkeys", request_body = PasskeyAssertion, responses((status = 200, body = AuthSession)), security(()))]
async fn passkey_sign_in(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    ip: ClientIp,
    Json(req): Json<PasskeyAssertion>,
) -> AppResult<(CookieJar, Json<AuthSession>)> {
    let user = discoverable_sign_in(&state, &ip, &req, "sign_in").await?;
    session_for(&state, jar, &user).await
}

#[utoipa::path(post, path = "/auth/passkeys/sign-in/desktop", tag = "passkeys", request_body = PasskeyAssertion, responses((status = 200, body = DesktopPasskeyCode)), security(()))]
async fn passkey_sign_in_for_desktop(
    State(state): State<Arc<AppState>>,
    ip: ClientIp,
    Json(req): Json<PasskeyAssertion>,
) -> AppResult<Json<DesktopPasskeyCode>> {
    let user = discoverable_sign_in(&state, &ip, &req, "desktop_sign_in").await?;
    let code = issue_desktop_code(&state, user.id, "/app").await?;
    Ok(Json(DesktopPasskeyCode { code }))
}

async fn discoverable_sign_in(
    state: &AppState,
    ip: &ClientIp,
    req: &PasskeyAssertion,
    stage: &str,
) -> AppResult<User> {
    limit_by_ip(state, ip).await?;
    let Ceremony::SignIn { state: sign_in } = passkey::take(state, req.challenge_id).await? else {
        return Err(expired());
    };

    let webauthn = passkey::relying_party(&state.config)?;
    let (user_id, credential_id) = webauthn
        .identify_discoverable_authentication(&req.credential)
        .map_err(rejected)?;
    let row = state
        .passkey_repo
        .find_by_credential(credential_id)
        .await?
        .filter(|row| row.user_id == user_id)
        .ok_or_else(unknown_passkey)?;

    let result = webauthn
        .finish_discoverable_authentication_v2(&req.credential, sign_in, &row.credential.0)
        .map_err(rejected)?;

    signed_in(state, ip, row, &result, stage).await
}

#[utoipa::path(post, path = "/auth/passkeys/second-factor/start", tag = "passkeys", request_body = PasskeySecondFactorStart, responses((status = 200, body = PasskeyChallenge)), security(()))]
async fn start_passkey_second_factor(
    State(state): State<Arc<AppState>>,
    ip: ClientIp,
    Json(req): Json<PasskeySecondFactorStart>,
) -> AppResult<Json<PasskeyChallenge>> {
    let (user, email_code_hash) = match (req.password.as_deref(), req.email_code.as_deref()) {
        (Some(password), _) => (
            first_factor(&state, ip.0.as_deref(), &req.email, password).await?,
            None,
        ),
        (None, Some(code)) => {
            let checked = email_code_routes::check_code(&state, &ip, &req.email, code).await?;
            (checked.user, Some(checked.code_hash))
        }
        (None, None) => {
            return Err(AppError::BadRequest(
                "Send the password or the emailed code".into(),
            ))
        }
    };
    let keys: Vec<Passkey> = state
        .passkey_repo
        .list(user.id)
        .await?
        .into_iter()
        .map(|row| row.credential.0)
        .collect();
    if keys.is_empty() {
        return Err(AppError::BadRequest("This account has no passkeys".into()));
    }

    let (challenge, second_factor) = passkey::relying_party(&state.config)?
        .start_passkey_authentication(&keys)
        .map_err(rejected)?;
    let options = passkey::browser_options(&challenge)?;
    let challenge_id = passkey::stash(
        &state,
        &Ceremony::SecondFactor {
            user_id: user.id,
            state: second_factor,
            email_code_hash,
        },
    )
    .await?;
    Ok(Json(PasskeyChallenge {
        challenge_id,
        options,
    }))
}

#[utoipa::path(post, path = "/auth/passkeys/second-factor", tag = "passkeys", request_body = PasskeyAssertion, responses((status = 200, body = AuthSession)), security(()))]
async fn passkey_second_factor(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    ip: ClientIp,
    Json(req): Json<PasskeyAssertion>,
) -> AppResult<(CookieJar, Json<AuthSession>)> {
    let Ceremony::SecondFactor {
        user_id,
        state: second_factor,
        email_code_hash,
    } = passkey::take(&state, req.challenge_id).await?
    else {
        return Err(expired());
    };

    let result = passkey::relying_party(&state.config)?
        .finish_passkey_authentication(&req.credential, &second_factor)
        .map_err(rejected)?;
    let credential_id: &[u8] = result.cred_id();
    let row = state
        .passkey_repo
        .find_by_credential(credential_id)
        .await?
        .filter(|row| row.user_id == user_id)
        .ok_or_else(unknown_passkey)?;

    let user = signed_in(&state, &ip, row, &result, "second_factor").await?;
    if let Some(code_hash) = email_code_hash {
        let checked = email_code_routes::CheckedCode { user, code_hash };
        email_code_routes::spend_code(&state, &ip, &checked, "second_factor").await?;
        return session_for(&state, jar, &checked.user).await;
    }
    session_for(&state, jar, &user).await
}

async fn signed_in(
    state: &AppState,
    ip: &ClientIp,
    row: StoredPasskey,
    result: &AuthenticationResult,
    stage: &str,
) -> AppResult<User> {
    let user = load_user(state, row.user_id).await?;
    match user.status {
        UserStatus::Active => {}
        UserStatus::Suspended => {
            return Err(AppError::Unauthorized("This account is suspended".into()))
        }
        UserStatus::Pending => {
            return Err(AppError::Unauthorized(
                "This account is not active yet".into(),
            ))
        }
    }
    refuse_sso(&user)?;

    let mut key = row.credential.0;
    key.update_credential(result);
    state.passkey_repo.record_use(row.id, &key).await?;

    audit::record(
        state,
        AuditEntry::new(AuditAction::PasskeyUsed, user.id)
            .resource(user.id)
            .ip(ip)
            .details(serde_json::json!({ "passkey_id": row.id, "stage": stage })),
    )
    .await;

    Ok(user)
}

async fn session_for(
    state: &AppState,
    jar: CookieJar,
    user: &User,
) -> AppResult<(CookieJar, Json<AuthSession>)> {
    let tokens = state.auth_service.tokens_for(user).await?;
    let secure = state.config.public_url.starts_with("https://");
    let jar = set_auth_cookies(jar, &tokens, secure);
    Ok((jar, Json(tokens.into())))
}

async fn confirm_presence(state: &AppState, user: &User, proof: &PresenceProof) -> AppResult<()> {
    check_rate_limit(
        state,
        &format!("rate_limit:presence:{}", user.id),
        state.config.login_attempts_per_email,
        state.config.login_attempts_window_secs,
    )
    .await?;

    if let Some(password) = proof.password.as_deref().filter(|p| !p.is_empty()) {
        let matches = match user.password_hash.as_deref() {
            Some(hash) => AuthService::verify_password(password, hash)
                .await
                .unwrap_or(false),
            None => false,
        };
        if matches {
            return Ok(());
        }
        return Err(AppError::Unauthorized("That password is not right".into()));
    }

    if let Some(code) = proof.code.as_deref().filter(|c| !c.is_empty()) {
        let enrolment = state
            .totp_repo
            .find(user.id)
            .await?
            .filter(|e| e.is_active())
            .ok_or_else(|| AppError::BadRequest("No second factor is enrolled".into()))?;
        let secret = totp::decrypt_secret(&state.config.secrets_keys, &enrolment)?;
        let step = totp::matching_step(
            &secret,
            &user.email,
            &state.config.instance_name,
            code,
            Utc::now().timestamp(),
        )?;
        if let Some(step) = step {
            if state.totp_repo.claim_step(user.id, step).await? {
                return Ok(());
            }
        }
        return Err(AppError::Unauthorized("That code did not match".into()));
    }

    Err(AppError::BadRequest(
        "Confirm with your password or a code".into(),
    ))
}

async fn limit_by_ip(state: &AppState, ip: &ClientIp) -> AppResult<()> {
    let Some(ip) = ip.0.as_deref() else {
        return Ok(());
    };
    check_rate_limit(
        state,
        &format!("rate_limit:passkey_ip:{ip}"),
        state.config.login_attempts_per_ip,
        state.config.login_attempts_window_secs,
    )
    .await
}

async fn load_user(state: &AppState, user_id: Uuid) -> AppResult<User> {
    state
        .auth_service
        .repo()
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))
}

fn refuse_sso(user: &User) -> AppResult<()> {
    if user.password_hash.is_none() {
        return Err(AppError::Forbidden(
            "This account signs in through SSO, so passkeys are not offered".into(),
        ));
    }
    Ok(())
}

fn passkey_name(raw: &str) -> AppResult<&str> {
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(AppError::BadRequest(
            "A passkey name is 1 to 64 characters".into(),
        ));
    }
    Ok(name)
}

fn rejected(error: WebauthnError) -> AppError {
    tracing::info!(%error, "passkey ceremony rejected");
    AppError::Unauthorized("That passkey could not be verified".into())
}

fn expired() -> AppError {
    AppError::Unauthorized("That passkey request expired, please try again".into())
}

fn unknown_passkey() -> AppError {
    AppError::Unauthorized("That passkey is not registered here".into())
}

fn too_many() -> AppError {
    AppError::Conflict(format!(
        "An account holds up to {} passkeys",
        passkey::MAX_PER_ACCOUNT
    ))
}
