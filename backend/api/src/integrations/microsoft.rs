use serde_json::Value;

use shared_common::errors::{AppError, AppResult};

use super::http::{self, TokenSet};

pub const SCOPES: [&str; 2] = ["Calendars.Read", "offline_access"];
const TENANT: &str = "organizations";

pub struct Microsoft {
    pub login_base: String,
    pub graph_base: String,
}

fn provider_error(what: &str, e: impl std::fmt::Display) -> AppError {
    http::provider_error("Microsoft", what, e)
}

impl Microsoft {
    pub fn authorize_url(
        &self,
        client_id: &str,
        redirect_uri: &str,
        state: &str,
        challenge: &str,
    ) -> String {
        let mut url = reqwest::Url::parse(&format!(
            "{}/{TENANT}/oauth2/v2.0/authorize",
            self.login_base
        ))
        .expect("the Microsoft login base is a valid url");
        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("response_mode", "query")
            .append_pair("scope", &SCOPES.join(" "))
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
        url.to_string()
    }

    async fn token(&self, form: &[(&str, &str)]) -> AppResult<TokenSet> {
        let response = http::client()?
            .post(format!("{}/{TENANT}/oauth2/v2.0/token", self.login_base))
            .form(form)
            .send()
            .await
            .map_err(|e| provider_error("token request", e))?;
        if !response.status().is_success() {
            return Err(provider_error("token request", response.status()));
        }
        response
            .json::<TokenSet>()
            .await
            .map_err(|e| provider_error("token response", e))
    }

    pub async fn exchange(
        &self,
        client_id: &str,
        client_secret: &str,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> AppResult<TokenSet> {
        let scope = SCOPES.join(" ");
        self.token(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
            ("scope", &scope),
        ])
        .await
    }

    pub async fn refresh(
        &self,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> AppResult<TokenSet> {
        let scope = SCOPES.join(" ");
        self.token(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("scope", &scope),
        ])
        .await
    }

    pub async fn account_name(&self, token: &str) -> AppResult<String> {
        let response = http::client()?
            .get(format!("{}/v1.0/me/calendar", self.graph_base))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| provider_error("request", e))?;
        if !response.status().is_success() {
            return Err(provider_error("request", response.status()));
        }
        let calendar = response
            .json::<Value>()
            .await
            .map_err(|e| provider_error("response", e))?;
        let name = calendar.pointer("/owner/name").and_then(Value::as_str);
        let address = calendar.pointer("/owner/address").and_then(Value::as_str);
        Ok(match (name, address) {
            (Some(name), Some(address)) if !name.is_empty() => format!("{name} ({address})"),
            (_, Some(address)) => address.to_string(),
            (Some(name), None) => name.to_string(),
            (None, None) => "Microsoft account".to_string(),
        })
    }
}
