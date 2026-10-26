use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use tracing::info;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation;

use super::models::{SetupComplete, SetupRequest};
use crate::auth::routes::set_auth_cookies;
use crate::state::AppState;

pub fn public_router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(complete_setup))
}

fn already_set_up() -> AppError {
    AppError::Conflict("This instance is already set up. Sign in instead".into())
}

#[utoipa::path(post, path = "/setup", tag = "instance", request_body = SetupRequest, responses((status = 200, body = SetupComplete)), security(()))]
async fn complete_setup(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(req): Json<SetupRequest>,
) -> AppResult<(CookieJar, Json<SetupComplete>)> {
    if state.setup_repo.has_instance_admin().await? {
        return Err(already_set_up());
    }
    let nonce = state
        .setup_repo
        .current_nonce()
        .await?
        .filter(|nonce| super::token_matches(&state.config.jwt_secret, nonce, &req.token))
        .ok_or_else(|| {
            AppError::Forbidden(
                "That setup token is not right. Copy it again from the API log".into(),
            )
        })?;

    let email = req.email.trim();
    let display_name = req.display_name.trim();
    let workspace_name = req.workspace_name.trim();
    validation::validate_email(email)?;
    validation::validate_display_name(display_name)?;
    validation::validate_password(&req.password)?;
    validation::validate_workspace_name(workspace_name)?;

    if state
        .auth_service
        .repo()
        .find_by_email(email)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(
            "An account with this email already exists".into(),
        ));
    }

    if !state.setup_repo.claim(&nonce).await? {
        return Err(already_set_up());
    }

    let user = match state
        .auth_service
        .create_instance_admin(email, &req.password, display_name)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            state.setup_repo.restore(&nonce).await?;
            return Err(e);
        }
    };

    let workspace = state
        .workspace_service
        .create_workspace(workspace_name, None, user.id)
        .await?;
    info!(email = %email, workspace = %workspace.id, "instance set up");

    let tokens = state.auth_service.tokens_for(&user).await?;
    let secure = state.config.public_url.starts_with("https://");
    let jar = set_auth_cookies(jar, &tokens, secure);

    Ok((
        jar,
        Json(SetupComplete {
            session: tokens.into(),
            workspace_id: workspace.id,
        }),
    ))
}
