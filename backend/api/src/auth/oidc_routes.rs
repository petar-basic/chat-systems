use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use rand::RngExt;
use redis::AsyncCommands;
use serde::Deserialize;

use shared_common::errors::{AppError, AppResult};

use super::oidc::{self, OidcSettings, PendingLogin, Provisioning};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::state::AppState;

const HANDLE_COOKIE: &str = "oidc_handle";
/// Long enough to type a password at the provider, short enough that an
/// abandoned attempt is not a standing invitation.
const PENDING_TTL_SECS: u64 = 600;

pub fn router(state: Arc<AppState>) -> Router {
    // Outside `auth_middleware`: nobody has a session yet, that being the point.
    Router::new()
        .route("/auth/oidc/start", get(start))
        .route("/auth/oidc/callback", get(callback))
        .with_state(state)
}

pub fn settings_from(config: &crate::config::AppConfig) -> OidcSettings {
    OidcSettings {
        issuer: config.oidc_issuer.clone(),
        client_id: config.oidc_client_id.clone(),
        client_secret: config.oidc_client_secret.clone(),
        redirect_url: format!("{}/api/auth/oidc/callback", config.public_url),
        provisioning: Provisioning::parse(&config.oidc_provisioning, &config.oidc_allowed_domains),
    }
}

fn handle() -> String {
    use base64::Engine;
    let mut bytes = [0u8; 24];
    rand::rng().fill(&mut bytes[..]);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn pending_key(handle: &str) -> String {
    format!("oidc:pending:{handle}")
}

async fn start(State(state): State<Arc<AppState>>, jar: CookieJar) -> AppResult<Response> {
    let settings = settings_from(&state.config);
    if !settings.is_configured() {
        return Err(AppError::BadRequest("SSO is not configured".into()));
    }
    if !settings.provisioning.may_sign_in() {
        return Err(AppError::Forbidden(
            "SSO is disabled on this instance".into(),
        ));
    }

    let (url, pending) = oidc::start(&settings).await?;

    let handle = handle();
    let mut conn = state.redis.clone();
    let payload = serde_json::to_string(&pending)
        .map_err(|e| AppError::Internal(format!("Could not store the login: {e}")))?;
    let _: () = conn
        .set_ex(pending_key(&handle), payload, PENDING_TTL_SECS)
        .await
        .map_err(|e| AppError::Internal(format!("Could not store the login: {e}")))?;

    // The verifier stays server-side; the cookie only names it. Lax rather than
    // Strict because the provider redirects back across sites.
    let secure = state.config.public_url.starts_with("https://");
    let cookie = Cookie::build((HANDLE_COOKIE, handle))
        .path("/")
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(PENDING_TTL_SECS as i64))
        .build();

    Ok((jar.add(cookie), Redirect::temporary(&url)).into_response())
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn callback(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    ip: ClientIp,
    Query(query): Query<CallbackQuery>,
) -> AppResult<Response> {
    if let Some(error) = query.error {
        return Err(AppError::Unauthorized(format!("SSO failed: {error}")));
    }

    let settings = settings_from(&state.config);
    let handle = jar
        .get(HANDLE_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(|| AppError::Unauthorized("This sign-in did not start here".into()))?;

    let mut conn = state.redis.clone();
    // Taken, not read: a login attempt is good for one callback.
    let stored: Option<String> = conn
        .get_del(pending_key(&handle))
        .await
        .map_err(|e| AppError::Internal(format!("Could not read the login: {e}")))?;
    let pending: PendingLogin = stored
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .ok_or_else(|| AppError::Unauthorized("This sign-in expired, please try again".into()))?;

    let returned_state = query
        .state
        .ok_or_else(|| AppError::Unauthorized("Missing state".into()))?;
    if returned_state != pending.csrf {
        return Err(AppError::Unauthorized("State did not match".into()));
    }

    let code = query
        .code
        .ok_or_else(|| AppError::Unauthorized("Missing authorization code".into()))?;
    let identity = oidc::exchange(&settings, &pending, &code).await?;

    let user = link_or_create(&state, &settings, &identity, &ip).await?;
    let tokens = state.auth_service.tokens_for(&user).await?;

    let secure = state.config.public_url.starts_with("https://");
    let jar = jar.remove(Cookie::from(HANDLE_COOKIE));
    let jar = super::routes::set_auth_cookies(jar, &tokens, secure);

    Ok((
        jar,
        Redirect::temporary(&format!("{}/app?sso=1", state.config.public_url)),
    )
        .into_response())
}

pub(crate) async fn link_or_create(
    state: &AppState,
    settings: &OidcSettings,
    identity: &oidc::VerifiedIdentity,
    ip: &ClientIp,
) -> AppResult<crate::auth::models::User> {
    let repo = state.auth_service.repo();

    // An existing link wins: the provider's subject is stable even when somebody
    // changes their email there.
    if let Some(user_id) = find_linked_user(state, &identity.subject).await? {
        return repo
            .find_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Linked account no longer exists".into()));
    }

    let existing = repo.find_by_email(&identity.email).await?;
    let user = match existing {
        Some(user) => user,
        None => {
            if !settings.provisioning.may_create(&identity.email) {
                return Err(AppError::Forbidden(
                    "This instance is invite-only; ask an admin for an invite".into(),
                ));
            }
            repo.create_sso_user(&identity.email).await?
        }
    };

    if user.status == crate::auth::models::UserStatus::Suspended {
        return Err(AppError::Unauthorized("This account is suspended".into()));
    }

    sqlx::query(
        r"
        INSERT INTO oauth_accounts (user_id, provider, provider_id, email)
        VALUES ($1, 'oidc', $2, $3)
        ON CONFLICT (provider, provider_id) DO UPDATE SET email = EXCLUDED.email
        ",
    )
    .bind(user.id)
    .bind(&identity.subject)
    .bind(&identity.email)
    .execute(&state.pool)
    .await?;

    // A shadow password on every SSO account is a second way in that nobody is
    // watching. The break-glass local admin is the deliberate exception.
    if !user.is_instance_admin && user.password_hash.is_some() {
        sqlx::query("UPDATE users SET password_hash = NULL, updated_at = NOW() WHERE id = $1")
            .bind(user.id)
            .execute(&state.pool)
            .await?;
    }

    audit::record(
        state,
        AuditEntry::new(AuditAction::SsoLinked, user.id)
            .resource(user.id)
            .ip(ip)
            .details(serde_json::json!({ "provider": "oidc", "email": identity.email })),
    )
    .await;

    repo.find_by_id(user.id)
        .await?
        .ok_or_else(|| AppError::Internal("User vanished mid-login".into()))
}

async fn find_linked_user(state: &AppState, subject: &str) -> AppResult<Option<uuid::Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT user_id FROM oauth_accounts WHERE provider = 'oidc' AND provider_id = $1",
    )
    .bind(subject)
    .fetch_optional(&state.pool)
    .await?)
}
