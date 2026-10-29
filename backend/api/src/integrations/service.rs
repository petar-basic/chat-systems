use base64::Engine;
use chrono::{Duration, Utc};
use rand::distr::Alphanumeric;
use rand::RngExt;
use redis::AsyncCommands;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::confluence;
use super::figma::{self, Figma};
use super::github::{self, Fetched, GitHub};
use super::google::{self, Google};
use super::http::TokenSet;
use super::jira::{self, Atlassian};
use super::models::*;
use super::repo::NewConnection;
use crate::messaging::models::Unfurl;
use crate::sealed::{self, Sealed};
use crate::state::AppState;

const PENDING_TTL_SECS: u64 = 600;
const REFRESH_MARGIN_SECS: i64 = 60;
const PUBLIC_CACHE_SECS: u64 = 600;

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

fn github_client(state: &AppState) -> GitHub {
    GitHub {
        web_base: state
            .config
            .github_web_url
            .trim_end_matches('/')
            .to_string(),
        api_base: state
            .config
            .github_api_url
            .trim_end_matches('/')
            .to_string(),
    }
}

fn google_client(state: &AppState) -> Google {
    Google {
        accounts_base: state
            .config
            .google_accounts_url
            .trim_end_matches('/')
            .to_string(),
        oauth_base: state
            .config
            .google_oauth_url
            .trim_end_matches('/')
            .to_string(),
        api_base: state
            .config
            .google_api_url
            .trim_end_matches('/')
            .to_string(),
    }
}

fn figma_client(state: &AppState) -> Figma {
    Figma {
        web_base: state.config.figma_web_url.trim_end_matches('/').to_string(),
        api_base: state.config.figma_api_url.trim_end_matches('/').to_string(),
    }
}

fn seal(state: &AppState, value: &str) -> AppResult<Sealed> {
    sealed::seal(&state.config.secrets_keys, sealed::APP_SECRETS, value)
}

fn open(
    state: &AppState,
    ciphertext: &[u8],
    nonce: &[u8],
    key_id: Option<&str>,
) -> AppResult<String> {
    sealed::open(
        &state.config.secrets_keys,
        sealed::APP_SECRETS,
        ciphertext,
        nonce,
        key_id,
    )
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
    let prompts_off = state
        .integration_repo
        .prompt_optouts(workspace_id, user_id)
        .await?;
    Ok(Provider::ALL
        .into_iter()
        .map(|provider| {
            let app = apps.iter().find(|a| a.provider == provider.as_str());
            AppStatus {
                provider,
                name: provider.name().into(),
                summary: provider.summary().into(),
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
                prompts_off: prompts_off.iter().any(|p| p == provider.as_str()),
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
    if let Some(source) = req.reuse_from {
        if (source, provider) != (Provider::Jira, Provider::Confluence) {
            return Err(AppError::Validation(
                "Only Confluence can reuse another app's credentials, and only Jira's".into(),
            ));
        }
        let jira = state
            .integration_repo
            .app(workspace_id, source.as_str())
            .await?
            .ok_or_else(|| AppError::Validation("Set Jira up first".into()))?;
        state
            .integration_repo
            .save_app(
                workspace_id,
                provider.as_str(),
                &jira.client_id,
                &Sealed {
                    ciphertext: jira.client_secret_encrypted,
                    nonce: jira.client_secret_nonce,
                    key_id: jira.client_secret_key_id.ok_or_else(|| {
                        AppError::Internal(
                            "Jira's secret has not been re-sealed since the upgrade".into(),
                        )
                    })?,
                },
                req.enabled,
                admin_id,
            )
            .await?;
        return Ok(());
    }
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

    let redirect = redirect_url(state);
    let challenge = challenge(&verifier);
    Ok(match provider {
        Provider::Jira | Provider::Confluence => atlassian(state).authorize_url(
            &app.client_id,
            &redirect,
            &oauth_state,
            &challenge,
            provider.scopes(),
        ),
        Provider::Github => {
            github_client(state).authorize_url(&app.client_id, &redirect, &oauth_state, &challenge)
        }
        Provider::GoogleDrive => {
            google_client(state).authorize_url(&app.client_id, &redirect, &oauth_state, &challenge)
        }
        Provider::Figma => {
            figma_client(state).authorize_url(&app.client_id, &redirect, &oauth_state, &challenge)
        }
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
        app.client_secret_key_id.as_deref(),
    )?;

    let redirect = redirect_url(state);
    let (tokens, account, resources): (TokenSet, String, serde_json::Value) = match pending.provider
    {
        Provider::Jira | Provider::Confluence => {
            let client = atlassian(state);
            let tokens = client
                .exchange(
                    &app.client_id,
                    &secret,
                    &req.code,
                    &redirect,
                    &pending.code_verifier,
                )
                .await?;
            let sites = client.sites(&tokens.access_token).await?;
            let account = client.account_name(&tokens.access_token).await?;
            let resources =
                serde_json::to_value(&sites).map_err(|e| AppError::Internal(e.to_string()))?;
            (tokens, account, resources)
        }
        Provider::Github => {
            let client = github_client(state);
            let tokens = client
                .exchange(
                    &app.client_id,
                    &secret,
                    &req.code,
                    &redirect,
                    &pending.code_verifier,
                )
                .await?;
            let account = client.account_name(&tokens.access_token).await?;
            (tokens, account, serde_json::json!([]))
        }
        Provider::GoogleDrive => {
            let client = google_client(state);
            let tokens = client
                .exchange(
                    &app.client_id,
                    &secret,
                    &req.code,
                    &redirect,
                    &pending.code_verifier,
                )
                .await?;
            let account = client.account_name(&tokens.access_token).await?;
            (tokens, account, serde_json::json!([]))
        }
        Provider::Figma => {
            let client = figma_client(state);
            let tokens = client
                .exchange(
                    &app.client_id,
                    &secret,
                    &req.code,
                    &redirect,
                    &pending.code_verifier,
                )
                .await?;
            let account = client.account_name(&tokens.access_token).await?;
            (tokens, account, serde_json::json!([]))
        }
    };
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
    state
        .integration_repo
        .turn_prompts_on(pending.workspace_id, user_id, pending.provider.as_str())
        .await?;
    let external_account = account;

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
            connection.access_token_key_id.as_deref(),
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
    let refresh_token = open(
        state,
        ciphertext,
        nonce,
        connection.refresh_token_key_id.as_deref(),
    )?;
    let secret = open(
        state,
        &app.client_secret_encrypted,
        &app.client_secret_nonce,
        app.client_secret_key_id.as_deref(),
    )?;
    let tokens = match Provider::parse(&app.provider) {
        Some(Provider::Jira | Provider::Confluence) => {
            atlassian(state)
                .refresh(&app.client_id, &secret, &refresh_token)
                .await?
        }
        Some(Provider::Github) => {
            github_client(state)
                .refresh(&app.client_id, &secret, &refresh_token)
                .await?
        }
        Some(Provider::GoogleDrive) => {
            google_client(state)
                .refresh(&app.client_id, &secret, &refresh_token)
                .await?
        }
        Some(Provider::Figma) => {
            figma_client(state)
                .refresh(&app.client_id, &secret, &refresh_token)
                .await?
        }
        None => {
            return Err(AppError::Internal(format!(
                "unknown provider {}",
                app.provider
            )))
        }
    };
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
    Ok(
        match provider_unfurl(state, workspace_id, poster_id, url).await? {
            AppUnfurl::Connect(provider)
                if state
                    .integration_repo
                    .prompts_off(workspace_id, poster_id, provider.as_str())
                    .await? =>
            {
                AppUnfurl::Unclaimed
            }
            other => other,
        },
    )
}

async fn provider_unfurl(
    state: &AppState,
    workspace_id: Uuid,
    poster_id: Uuid,
    url: &str,
) -> AppResult<AppUnfurl> {
    if let Some(issue) = jira::match_issue(url) {
        return jira_unfurl(state, workspace_id, poster_id, &issue, url).await;
    }
    if let Some(target) = github::match_url(url) {
        return github_unfurl(state, workspace_id, poster_id, &target, url).await;
    }
    if let Some(file_id) = google::match_file(url) {
        return google_unfurl(state, workspace_id, poster_id, &file_id, url).await;
    }
    if let Some(content) = confluence::match_content(url) {
        return confluence_unfurl(state, workspace_id, poster_id, &content, url).await;
    }
    if let Some(key) = figma::match_file(url) {
        return figma_unfurl(state, workspace_id, poster_id, &key, url).await;
    }
    Ok(AppUnfurl::Unclaimed)
}

async fn enabled_app(
    state: &AppState,
    workspace_id: Uuid,
    provider: Provider,
) -> AppResult<Option<WorkspaceApp>> {
    Ok(state
        .integration_repo
        .app(workspace_id, provider.as_str())
        .await?
        .filter(|a| a.enabled))
}

async fn jira_unfurl(
    state: &AppState,
    workspace_id: Uuid,
    poster_id: Uuid,
    issue: &jira::IssueRef,
    url: &str,
) -> AppResult<AppUnfurl> {
    let Some(app) = enabled_app(state, workspace_id, Provider::Jira).await? else {
        return Ok(AppUnfurl::Unclaimed);
    };
    let Some(connection) = state
        .integration_repo
        .connection(workspace_id, poster_id, Provider::Jira.as_str())
        .await?
    else {
        return Ok(AppUnfurl::Connect(Provider::Jira));
    };
    let sites: Vec<AtlassianSite> =
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
            .issue_card(&token, &site.id, issue, url)
            .await?
        {
            Some(card) => AppUnfurl::Card(Box::new(card)),
            None => AppUnfurl::Unclaimed,
        },
    )
}

async fn github_unfurl(
    state: &AppState,
    workspace_id: Uuid,
    poster_id: Uuid,
    target: &github::Target,
    url: &str,
) -> AppResult<AppUnfurl> {
    let client = github_client(state);
    let app = enabled_app(state, workspace_id, Provider::Github).await?;
    if let Some(app) = &app {
        if let Some(connection) = state
            .integration_repo
            .connection(workspace_id, poster_id, Provider::Github.as_str())
            .await?
        {
            let token = access_token(state, app, poster_id, &connection).await?;
            return Ok(match client.card(target, url, Some(&token)).await? {
                Fetched::Card(card) => AppUnfurl::Card(card),
                Fetched::NotVisible => AppUnfurl::Unclaimed,
            });
        }
    }
    if let Some(card) = public_github_card(state, &client, target, url).await? {
        return Ok(AppUnfurl::Card(card));
    }
    Ok(match app {
        Some(_) => AppUnfurl::Connect(Provider::Github),
        None => AppUnfurl::Unclaimed,
    })
}

async fn google_unfurl(
    state: &AppState,
    workspace_id: Uuid,
    poster_id: Uuid,
    file_id: &str,
    url: &str,
) -> AppResult<AppUnfurl> {
    let Some(app) = enabled_app(state, workspace_id, Provider::GoogleDrive).await? else {
        return Ok(AppUnfurl::Unclaimed);
    };
    let Some(connection) = state
        .integration_repo
        .connection(workspace_id, poster_id, Provider::GoogleDrive.as_str())
        .await?
    else {
        return Ok(AppUnfurl::Connect(Provider::GoogleDrive));
    };
    let token = access_token(state, &app, poster_id, &connection).await?;
    Ok(
        match google_client(state).file_card(&token, file_id, url).await? {
            Some(card) => AppUnfurl::Card(Box::new(card)),
            None => AppUnfurl::Unclaimed,
        },
    )
}

async fn confluence_unfurl(
    state: &AppState,
    workspace_id: Uuid,
    poster_id: Uuid,
    content: &confluence::ContentRef,
    url: &str,
) -> AppResult<AppUnfurl> {
    let Some(app) = enabled_app(state, workspace_id, Provider::Confluence).await? else {
        return Ok(AppUnfurl::Unclaimed);
    };
    let Some(connection) = state
        .integration_repo
        .connection(workspace_id, poster_id, Provider::Confluence.as_str())
        .await?
    else {
        return Ok(AppUnfurl::Connect(Provider::Confluence));
    };
    let sites: Vec<AtlassianSite> =
        serde_json::from_value(connection.resources.clone()).unwrap_or_default();
    let Some(site) = sites
        .iter()
        .find(|s| s.url.eq_ignore_ascii_case(&content.site_url))
    else {
        return Ok(AppUnfurl::Unclaimed);
    };
    let token = access_token(state, &app, poster_id, &connection).await?;
    Ok(
        match atlassian(state)
            .content_card(&token, &site.id, content, url)
            .await?
        {
            Some(card) => AppUnfurl::Card(Box::new(card)),
            None => AppUnfurl::Unclaimed,
        },
    )
}

async fn figma_unfurl(
    state: &AppState,
    workspace_id: Uuid,
    poster_id: Uuid,
    key: &str,
    url: &str,
) -> AppResult<AppUnfurl> {
    let Some(app) = enabled_app(state, workspace_id, Provider::Figma).await? else {
        return Ok(AppUnfurl::Unclaimed);
    };
    let Some(connection) = state
        .integration_repo
        .connection(workspace_id, poster_id, Provider::Figma.as_str())
        .await?
    else {
        return Ok(AppUnfurl::Connect(Provider::Figma));
    };
    let token = access_token(state, &app, poster_id, &connection).await?;
    Ok(
        match figma_client(state).file_card(&token, key, url).await? {
            Some(card) => AppUnfurl::Card(Box::new(card)),
            None => AppUnfurl::Unclaimed,
        },
    )
}

async fn public_github_card(
    state: &AppState,
    client: &GitHub,
    target: &github::Target,
    url: &str,
) -> AppResult<Option<Box<Unfurl>>> {
    let key = format!(
        "gh-public:v1:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(url.as_bytes()))
    );
    let mut redis = state.redis.clone();
    let cached: Option<String> = redis.get(&key).await.unwrap_or_default();
    if let Some(cached) = cached.and_then(|c| serde_json::from_str::<Option<Unfurl>>(&c).ok()) {
        return Ok(cached.map(Box::new));
    }
    let card = match client.card(target, url, None).await? {
        Fetched::Card(card) => Some(*card),
        Fetched::NotVisible => None,
    };
    if let Ok(encoded) = serde_json::to_string(&card) {
        let stored: redis::RedisResult<()> = redis.set_ex(&key, encoded, PUBLIC_CACHE_SECS).await;
        if let Err(e) = stored {
            tracing::warn!("apps: failed to cache a public GitHub card: {e}");
        }
    }
    Ok(card.map(Box::new))
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
