use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use chrono::Utc;
use serde_json::json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{is_unique_violation, AppError, AppResult};

use super::models::{AnswerUnfurlsRequest, UnfurlDomains, MAX_DOMAINS};
use super::{normalize_domain, providers_claiming, providers_on};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::blocks;
use crate::bots::models::{BotApp, Scope};
use crate::bots::Caller;
use crate::dto::StatusResponse;
use crate::messaging::models::{Unfurl, UnfurlApp};
use crate::messaging::unfurl::links;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_unfurl_domains, save_unfurl_domains))
        .routes(routes!(answer_unfurls))
}

async fn app_as_admin(state: &AppState, app_id: Uuid, user_id: Uuid) -> AppResult<BotApp> {
    let app = state
        .bot_repo
        .find(app_id)
        .await?
        .ok_or_else(|| AppError::NotFound("App not found".into()))?;
    authz::require_workspace_role(state, app.workspace_id, user_id, &WorkspaceRole::Admin).await?;
    Ok(app)
}

#[utoipa::path(get, path = "/bot-apps/{app_id}/unfurl-domains", tag = "bot-apps",
    responses((status = 200, body = UnfurlDomains)))]
async fn list_unfurl_domains(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(app_id): Path<Uuid>,
) -> AppResult<Json<UnfurlDomains>> {
    let app = app_as_admin(&state, app_id, auth.user_id).await?;
    Ok(Json(UnfurlDomains {
        domains: state.app_unfurl_repo.domains(app.id).await?,
    }))
}

/// Replaces the domains whose links the app previews itself. A domain covers
/// its subdomains, no two apps of a workspace claim the same one, and a
/// built-in preview's domain is refused while that integration is on.
#[utoipa::path(put, path = "/bot-apps/{app_id}/unfurl-domains", tag = "bot-apps",
    request_body = UnfurlDomains,
    responses((status = 200, body = UnfurlDomains)))]
async fn save_unfurl_domains(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(app_id): Path<Uuid>,
    Json(req): Json<UnfurlDomains>,
) -> AppResult<Json<UnfurlDomains>> {
    let app = app_as_admin(&state, app_id, auth.user_id).await?;
    if req.domains.len() > MAX_DOMAINS {
        return Err(AppError::Validation(format!(
            "domains: an app claims at most {MAX_DOMAINS}"
        )));
    }
    let domains = req
        .domains
        .iter()
        .map(|domain| normalize_domain(domain))
        .collect::<AppResult<Vec<_>>>()?;
    let mut seen = HashSet::new();
    if !domains.iter().all(|domain| seen.insert(domain.as_str())) {
        return Err(AppError::Validation("domains: each domain once".into()));
    }
    if !domains.is_empty()
        && !(app.scopes.contains(&Scope::LinksRead) && app.scopes.contains(&Scope::LinksWrite))
    {
        return Err(AppError::Validation(
            "domains: an app needs the links:read and links:write scopes to preview links".into(),
        ));
    }
    for domain in &domains {
        if let Some(provider) = providers_on(&state, app.workspace_id, &providers_claiming(domain))
            .await?
            .first()
        {
            return Err(AppError::Conflict(format!(
                "{domain} is previewed by {} while that integration is on",
                provider.name()
            )));
        }
    }
    if let Some(domain) = state
        .app_unfurl_repo
        .taken(app.workspace_id, app.id, &domains)
        .await?
        .first()
    {
        return Err(AppError::Conflict(format!(
            "{domain} is already claimed by another app"
        )));
    }

    let saved = state
        .app_unfurl_repo
        .replace(app.id, app.workspace_id, &domains)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                AppError::Conflict("A domain is already claimed by another app".into())
            } else {
                e.into()
            }
        })?;
    audit::record(
        &state,
        AuditEntry::new(AuditAction::BotAppUpdated, auth.user_id)
            .workspace(app.workspace_id)
            .resource(app.id)
            .ip(&ip)
            .details(json!({ "unfurl_domains": saved })),
    )
    .await;
    Ok(Json(UnfurlDomains { domains: saved }))
}

/// The app's previews of the links `link_shared` told it about, each a set of
/// blocks by URL. It answers within 30 minutes of the event, for that
/// message only, and may answer again to change a preview.
#[utoipa::path(post, path = "/messages/{msg_id}/unfurls", tags = ["messages", "bot"],
    request_body = AnswerUnfurlsRequest,
    security(("bot" = ["links:write"])),
    responses((status = 200, body = StatusResponse)))]
async fn answer_unfurls(
    State(state): State<Arc<AppState>>,
    caller: Caller,
    Path(msg_id): Path<Uuid>,
    Json(req): Json<AnswerUnfurlsRequest>,
) -> AppResult<Json<StatusResponse>> {
    let app_id = caller
        .bot()
        .map(|bot| bot.app_id)
        .ok_or_else(|| AppError::Forbidden("Only an app's bot token previews links".into()))?;
    caller.scoped(Scope::LinksWrite)?;
    let app = state
        .bot_repo
        .find(app_id)
        .await?
        .ok_or_else(|| AppError::NotFound("App not found".into()))?;
    if req.unfurls.is_empty() {
        return Err(AppError::Validation(
            "unfurls: give at least one link".into(),
        ));
    }
    let target = state
        .message_repo
        .find_unfurl_target(msg_id)
        .await?
        .filter(|target| target.workspace_id == app.workspace_id)
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
    let shared = state
        .app_unfurl_repo
        .shared(app.id, msg_id)
        .await?
        .ok_or_else(|| {
            AppError::Forbidden(
                "This message's links were not shared with the app in the last 30 minutes".into(),
            )
        })?;
    let present = links::extract(&target.content);
    if let Some(url) = req
        .unfurls
        .keys()
        .find(|url| !shared.contains(url) || !present.contains(url))
    {
        return Err(AppError::Validation(format!(
            "unfurls: {url} is not a link of this message that was shared with the app"
        )));
    }
    if !target.previews_enabled {
        return Err(AppError::Conflict(
            "Link previews are off in this workspace".into(),
        ));
    }

    let mut previews = Vec::new();
    for (url, answer) in &req.unfurls {
        let blocks = blocks::compose_unfurl(
            &state,
            app.workspace_id,
            &answer.blocks,
            &format!("unfurls[{url:?}].blocks"),
        )
        .await?;
        previews.push(Unfurl {
            url: url.clone(),
            title: app.name.clone(),
            site_name: Some(app.name.clone()),
            kind: Some("blocks".into()),
            blocks,
            app: Some(UnfurlApp {
                id: app.id,
                bot_user_id: app.bot_user_id,
                name: app.name.clone(),
                icon_url: app.icon_url.clone(),
            }),
            fetched_at: Some(Utc::now()),
            ..Unfurl::default()
        });
    }
    let urls: Vec<String> = req.unfurls.keys().cloned().collect();
    let previews =
        serde_json::to_value(&previews).map_err(|e| AppError::Internal(e.to_string()))?;

    let mut tx = state.pool.begin().await?;
    let message = state
        .message_repo
        .put_app_unfurls_in(&mut tx, msg_id, &urls, &previews, &present)
        .await?
        .ok_or_else(|| AppError::Conflict("The author removed this message's previews".into()))?;
    let staged = state
        .publisher
        .stage_message_updated(&mut tx, &message, app.workspace_id)
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(Json(StatusResponse::new("ok")))
}
