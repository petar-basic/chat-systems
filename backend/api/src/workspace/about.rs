use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::Channel;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::state::AppState;

pub struct About<'a> {
    pub name: Option<&'a str>,
    pub topic: Option<&'a str>,
    pub description: Option<&'a str>,
}

pub async fn update(
    state: &AppState,
    channel: &Channel,
    actor: Uuid,
    ip: &ClientIp,
    about: About<'_>,
) -> AppResult<Channel> {
    let updated = state
        .workspace_service
        .repo
        .update_channel(channel.id, about.name, about.topic, about.description)
        .await
        .map_err(duplicate_channel_name)?;

    audit::record(
        state,
        AuditEntry::new(AuditAction::ChannelUpdated, actor)
            .workspace(channel.workspace_id)
            .resource(channel.id)
            .ip(ip)
            .details(serde_json::json!({ "from": channel.name, "to": updated.name })),
    )
    .await;

    Ok(updated)
}

pub fn duplicate_channel_name(err: sqlx::Error) -> AppError {
    if shared_common::errors::is_unique_violation(&err) {
        AppError::Conflict("A channel with that name already exists".into())
    } else {
        err.into()
    }
}
