use base64::Engine;
use chrono::{Duration, Utc};
use rand::distr::Alphanumeric;
use rand::RngExt;
use redis::AsyncCommands;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::jira::{self, Atlassian};
use super::models::*;
use super::repo::NewConnection;
use crate::messaging::models::Unfurl;
use crate::sealed::{self, Sealed};
use crate::state::AppState;

const SECRET_PURPOSE: &str = "workspace-app-secrets";
const PENDING_TTL_SECS: u64 = 600;
const REFRESH_MARGIN_SECS: i64 = 60;

pub enum AppUnfurl {
    Card(Box<Unfurl>),
    Connect(Provider),
    Unclaimed,
}

pub fn redirect_url(state: &AppState) -> String {
    format!(
        "{}/oauth/apps/callback",
        state.config.public_url.trim_end_matches('/')
    )
}

fn atlassian(state: &AppState) -> Atlassian {
    Atlassian {
        auth_base: state
            .config
            .atlassian_auth_url
            .trim_end_matches('/')
            .to_string(),
        api_base: state
            .config
            .atlassian_api_url
            .trim_end_matches('/')
            .to_string(),
    }
}

fn seal(state: &AppState, value: &str) -> AppResult<Sealed> {
    sealed::seal(&state.config.jwt_secret, SECRET_PURPOSE, value)
}

fn open(state: &AppState, ciphertext: &[u8], nonce: &[u8]) -> AppResult<String> {
    sealed::open(&state.config.jwt_secret, SECRET_PURPOSE, ciphertext, nonce)
}

fn random(len: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn pending_key(oauth_state: &str) -> String {
    format!("app_oauth:{oauth_state}")
}

fn safe_return_to(return_to: Option<String>) -> Option<String> {
    return_to
        .filter(|path| path.starts_with('/') && !path.starts_with("//") && !path.contains('\\'))
}

pub async fn statuses(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<Vec<AppStatus>> {
    let apps = state.integration_repo.apps(workspace_id).await?;
    let connected = state
        .integration_repo
        .connected_accounts(workspace_id, user_id)
        .await?;
    Ok(Provider::ALL
        .into_iter()
        .map(|provider| {
            let app = apps.iter().find(|a| a.provider == provider.as_str());
            AppStatus {
                provider,
                name: provider.name().into(),
                configured: app.is_some(),
                enabled: app.is_some_and(|a| a.enabled),
                client_id: app.map(|a| a.client_id.clone()),
                redirect_url: redirect_url(state),
                scopes: provider.scopes().iter().map(|s| s.to_string()).collect(),
                console_url: provider.console_url().into(),
                setup_steps: provider
                    .setup_steps()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                connected_account: connected
                    .iter()
                    .find(|(p, _)| p == provider.as_str())
                    .map(|(_, account)| account.clone()),
            }
        })
        .collect())
}

pub async fn configure(
    state: &AppState,
    workspace_id: Uuid,
    admin_id: Uuid,
    provider: Provider,
    req: &ConfigureAppRequest,
) -> AppResult<()> {
    let client_id = req.client_id.trim();
    if client_id.is_empty() || client_id.len() > 200 {
        return Err(AppError::Validation("A client ID is required".into()));
    }
    match req
        .client_secret
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(secret) => {
            state
                .integration_repo
                .save_app(
                    workspace_id,
                    provider.as_str(),
                    client_id,
                    &seal(state, secret)?,
                    req.enabled,
                    admin_id,
                )
                .await?;
        }
        None => {
            if !state
                .integration_repo
                .update_app_settings(
                    workspace_id,
                    provider.as_str(),
                    client_id,
                    req.enabled,
                    admin_id,
                )
                .await?
            {
                return Err(AppError::Validation(
                    "The client secret is required the first time an app is set up".into(),
                ));
            }
        }
    }
    Ok(())
}

pub async fn start_connection(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
    provider: Provider,
    return_to: Option<String>,
) -> AppResult<String> {
    let app = state
        .integration_repo
        .app(workspace_id, provider.as_str())
        .await?
        .filter(|a| a.enabled)
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "{} is not set up in this workspace",
                provider.name()
            ))
        })?;

    let oauth_state = random(40);
    let verifier = random(64);
    let pending = PendingConnection {
        workspace_id,
        user_id,
        provider,
        code_verifier: verifier.clone(),
        return_to: safe_return_to(return_to),
    };
    let payload = serde_json::to_string(&pending).map_err(|e| AppError::Internal(e.to_string()))?;
    let mut redis = state.redis.clone();
    redis
        .set_ex::<_, _, ()>(pending_key(&oauth_state), payload, PENDING_TTL_SECS)
        .await
        .map_err(|e| AppError::Internal(format!("redis: {e}")))?;

    Ok(match provider {
        Provider::Jira => atlassian(state).authorize_url(
            &app.client_id,
            &redirect_url(state),
            &oauth_state,
            &challenge(&verifier),
        ),
    })
}

pub async fn complete_connection(
    state: &AppState,
    user_id: Uuid,
    req: &CompleteConnectionRequest,
) -> AppResult<CompleteConnectionResponse> {
    let mut redis = state.redis.clone();
    let payload: Option<String> = redis::cmd("GETDEL")
        .arg(pending_key(&req.state))
        .query_async(&mut redis)
        .await
        .map_err(|e| AppError::Internal(format!("redis: {e}")))?;
    let pending: PendingConnection = payload
        .and_then(|p| serde_json::from_str(&p).ok())
        .ok_or_else(|| {
            AppError::BadRequest("This connection link has expired; start again".into())
        })?;
    if pending.user_id != user_id {
        return Err(AppError::Forbidden(
            "This connection was started by someone else".into(),
        ));
    }

    let app = state
        .integration_repo
        .app(pending.workspace_id, pending.provider.as_str())
        .await?
        .ok_or_else(|| AppError::NotFound("The app is no longer set up".into()))?;
    let secret = open(
        state,
        &app.client_secret_encrypted,
        &app.client_secret_nonce,
    )?;

    let external_account = match pending.provider {
        Provider::Jira => {
            let client = atlassian(state);
            let tokens = client
                .exchange(
                    &app.client_id,
                    &secret,
                    &req.code,
                    &redirect_url(state),
                    &pending.code_verifier,
                )
                .await?;
            let sites = client.sites(&tokens.access_token).await?;
            let account = client.account_name(&tokens.access_token).await?;
            let resources =
                serde_json::to_value(&sites).map_err(|e| AppError::Internal(e.to_string()))?;
            let access = seal(state, &tokens.access_token)?;
            let refresh = tokens
                .refresh_token
                .as_deref()
                .map(|t| seal(state, t))
                .transpose()?;
            state
                .integration_repo
                .upsert_connection(NewConnection {
                    workspace_id: pending.workspace_id,
                    user_id,
                    provider: pending.provider.as_str(),
                    external_account: &account,
                    access_token: &access,
                    refresh_token: refresh.as_ref(),
                    expires_at: tokens.expires_in.map(|s| Utc::now() + Duration::seconds(s)),
                    resources: &resources,
                })
                .await?;
            account
        }
    };

    Ok(CompleteConnectionResponse {
        workspace_id: pending.workspace_id,
        provider: pending.provider,
        external_account,
        return_to: pending.return_to,
    })
}

async fn access_token(
    state: &AppState,
    app: &WorkspaceApp,
    user_id: Uuid,
    connection: &AppConnection,
) -> AppResult<String> {
    let fresh = connection
        .expires_at
        .is_none_or(|at| at > Utc::now() + Duration::seconds(REFRESH_MARGIN_SECS));
    if fresh {
        return open(
            state,
            &connection.access_token_encrypted,
            &connection.access_token_nonce,
        );
    }
    let (Some(ciphertext), Some(nonce)) = (
        &connection.refresh_token_encrypted,
        &connection.refresh_token_nonce,
    ) else {
        return Err(AppError::Unauthorized(
            "The connection has expired; connect again".into(),
        ));
    };
    let refresh_token = open(state, ciphertext, nonce)?;
    let secret = open(
        state,
        &app.client_secret_encrypted,
        &app.client_secret_nonce,
    )?;
    let tokens = atlassian(state)
        .refresh(&app.client_id, &secret, &refresh_token)
        .await?;
    let access = seal(state, &tokens.access_token)?;
    let refresh = tokens
        .refresh_token
        .as_deref()
        .map(|t| seal(state, t))
        .transpose()?;
    state
        .integration_repo
        .store_refreshed_tokens(
            app.workspace_id,
            user_id,
            &app.provider,
            &access,
            refresh.as_ref(),
            tokens.expires_in.map(|s| Utc::now() + Duration::seconds(s)),
        )
        .await?;
    Ok(tokens.access_token)
}

pub async fn unfurl(
    state: &AppState,
    workspace_id: Uuid,
    poster_id: Uuid,
    url: &str,
) -> AppResult<AppUnfurl> {
    let Some(issue) = jira::match_issue(url) else {
        return Ok(AppUnfurl::Unclaimed);
    };
    let Some(app) = state
        .integration_repo
        .app(workspace_id, Provider::Jira.as_str())
        .await?
        .filter(|a| a.enabled)
    else {
        return Ok(AppUnfurl::Unclaimed);
    };
    let Some(connection) = state
        .integration_repo
        .connection(workspace_id, poster_id, Provider::Jira.as_str())
        .await?
    else {
        return Ok(AppUnfurl::Connect(Provider::Jira));
    };
    let sites: Vec<JiraSite> =
        serde_json::from_value(connection.resources.clone()).unwrap_or_default();
    let Some(site) = sites
        .iter()
        .find(|s| s.url.eq_ignore_ascii_case(&issue.site_url))
    else {
        return Ok(AppUnfurl::Unclaimed);
    };
    let token = access_token(state, &app, poster_id, &connection).await?;
    Ok(
        match atlassian(state)
            .issue_card(&token, &site.id, &issue, url)
            .await?
        {
            Some(card) => AppUnfurl::Card(Box::new(card)),
            None => AppUnfurl::Unclaimed,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{challenge, safe_return_to};

    #[test]
    fn return_to_stays_on_this_site() {
        assert_eq!(
            safe_return_to(Some("/app/ws/ch".into())),
            Some("/app/ws/ch".into())
        );
        assert_eq!(safe_return_to(Some("//evil.test".into())), None);
        assert_eq!(safe_return_to(Some("https://evil.test".into())), None);
        assert_eq!(safe_return_to(Some("/\\evil.test".into())), None);
    }

    #[test]
    fn pkce_challenge_is_unpadded_base64url_of_sha256() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gXk2fDGfM"),
            "qTgfmNfUZUKrkf4tzPv8ObRjisT9C329U6d9rCzVlNM"
        );
    }
}
