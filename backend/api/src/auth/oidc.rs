use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};

use shared_common::errors::{AppError, AppResult};

/// Who may end up with an account by signing in.
///
/// `InviteOnly` is the default because it matches how the instance already
/// works: SSO changes how you prove who you are, not who is allowed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provisioning {
    Disabled,
    InviteOnly,
    DomainAllowlist(Vec<String>),
}

impl Provisioning {
    pub fn parse(mode: &str, domains: &str) -> Self {
        match mode {
            "disabled" => Self::Disabled,
            "domain_allowlist" => Self::DomainAllowlist(
                domains
                    .split(',')
                    .map(|d| d.trim().to_lowercase())
                    .filter(|d| !d.is_empty())
                    .collect(),
            ),
            _ => Self::InviteOnly,
        }
    }

    /// Whether an address with no account yet may have one created for it.
    pub fn may_create(&self, email: &str) -> bool {
        match self {
            Self::Disabled | Self::InviteOnly => false,
            Self::DomainAllowlist(domains) => email
                .rsplit('@')
                .next()
                .map(|d| domains.iter().any(|allowed| allowed == &d.to_lowercase()))
                .unwrap_or(false),
        }
    }

    /// Whether somebody who already has an account may sign in this way.
    pub fn may_sign_in(&self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

#[derive(Debug, Clone)]
pub struct OidcSettings {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub provisioning: Provisioning,
}

impl OidcSettings {
    pub fn is_configured(&self) -> bool {
        !self.issuer.is_empty() && !self.client_id.is_empty()
    }
}

/// What the browser has to bring back with the authorization code. Held in
/// Redis under an opaque handle rather than in the cookie itself, so the
/// verifier never leaves the server.
#[derive(Debug, Serialize, Deserialize)]
pub struct PendingLogin {
    pub csrf: String,
    pub pkce_verifier: String,
    pub nonce: String,
}

pub struct VerifiedIdentity {
    pub subject: String,
    pub email: String,
}

/// What discovery leaves us with: an authorization endpoint the metadata always
/// carries, and a token endpoint it only usually does.
type DiscoveredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

/// Redirects are refused rather than followed. These requests are made by the
/// server to a URL that ultimately comes from configuration, and a client that
/// chases redirects turns a misconfigured issuer into a request forgery against
/// whatever else this host can reach.
fn http_client() -> AppResult<openidconnect::reqwest::Client> {
    openidconnect::reqwest::ClientBuilder::new()
        .redirect(openidconnect::reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::Internal(format!("Could not build the OIDC HTTP client: {e}")))
}

async fn client(
    settings: &OidcSettings,
) -> AppResult<(DiscoveredClient, openidconnect::reqwest::Client)> {
    let issuer = IssuerUrl::new(settings.issuer.clone())
        .map_err(|e| AppError::Internal(format!("Invalid OIDC issuer: {e}")))?;

    let http = http_client()?;

    // Endpoints come from the provider's own document rather than being
    // hard-coded, which is what lets the same configuration work against Google,
    // Entra and Okta without a per-provider branch.
    let metadata = CoreProviderMetadata::discover_async(issuer, &http)
        .await
        .map_err(|e| AppError::Internal(format!("OIDC discovery failed: {e}")))?;

    let redirect = RedirectUrl::new(settings.redirect_url.clone())
        .map_err(|e| AppError::Internal(format!("Invalid OIDC redirect URL: {e}")))?;

    Ok((
        CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(settings.client_id.clone()),
            Some(ClientSecret::new(settings.client_secret.clone())),
        )
        .set_redirect_uri(redirect),
        http,
    ))
}

pub async fn start(settings: &OidcSettings) -> AppResult<(String, PendingLogin)> {
    let (client, _http) = client(settings).await?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();

    let (url, csrf, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("email".into()))
        .add_scope(Scope::new("profile".into()))
        .set_pkce_challenge(challenge)
        .url();

    Ok((
        url.to_string(),
        PendingLogin {
            csrf: csrf.secret().clone(),
            pkce_verifier: verifier.secret().clone(),
            nonce: nonce.secret().clone(),
        },
    ))
}

pub async fn exchange(
    settings: &OidcSettings,
    pending: &PendingLogin,
    code: &str,
) -> AppResult<VerifiedIdentity> {
    let (client, http) = client(settings).await?;

    let tokens = client
        .exchange_code(AuthorizationCode::new(code.to_string()))
        .map_err(|e| AppError::Internal(format!("Provider has no token endpoint: {e}")))?
        .set_pkce_verifier(PkceCodeVerifier::new(pending.pkce_verifier.clone()))
        .request_async(&http)
        .await
        .map_err(|e| AppError::Unauthorized(format!("OIDC code exchange failed: {e}")))?;

    let id_token = tokens
        .id_token()
        .ok_or_else(|| AppError::Unauthorized("Provider returned no id token".into()))?;

    let claims = id_token
        .claims(
            &client.id_token_verifier(),
            &Nonce::new(pending.nonce.clone()),
        )
        .map_err(|e| AppError::Unauthorized(format!("Invalid id token: {e}")))?;

    let email = claims
        .email()
        .map(|e| e.as_str().to_lowercase())
        .ok_or_else(|| AppError::Unauthorized("Provider returned no email".into()))?;

    // Linking on an unverified address is account takeover by signup: anyone who
    // can make their provider assert somebody else's email inherits that
    // account.
    if !claims.email_verified().unwrap_or(false) {
        return Err(AppError::Unauthorized(
            "The provider has not verified this email address".into(),
        ));
    }

    let _ = tokens.access_token();

    Ok(VerifiedIdentity {
        subject: claims.subject().as_str().to_string(),
        email,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_only_lets_existing_people_in_and_creates_nobody() {
        let policy = Provisioning::parse("invite_only", "");
        assert!(policy.may_sign_in());
        assert!(!policy.may_create("someone@example.com"));
    }

    #[test]
    fn disabled_turns_the_whole_door_off() {
        let policy = Provisioning::parse("disabled", "example.com");
        assert!(!policy.may_sign_in());
        assert!(!policy.may_create("someone@example.com"));
    }

    #[test]
    fn the_allowlist_matches_the_domain_and_nothing_else() {
        let policy = Provisioning::parse("domain_allowlist", "example.com, partner.test");
        assert!(policy.may_create("someone@example.com"));
        assert!(
            policy.may_create("SOMEONE@Example.com"),
            "case is not a domain"
        );
        assert!(policy.may_create("x@partner.test"));
        assert!(!policy.may_create("someone@evil.com"));
        assert!(
            !policy.may_create("someone@notexample.com"),
            "a suffix is not a domain"
        );
        assert!(!policy.may_create("nonsense"));
    }

    #[test]
    fn an_unknown_mode_falls_back_to_the_safe_one() {
        assert_eq!(Provisioning::parse("typo", ""), Provisioning::InviteOnly);
    }
}
