use serde::Serialize;
use utoipa::ToSchema;

use crate::hooks::models::{ChannelIncomingWebhook, ChannelOutgoingWebhook, ChannelSlashCommand};
use crate::workflows::models::ChannelWorkflow;

#[derive(Debug, Serialize, ToSchema)]
pub struct ChannelIntegrations {
    pub incoming_webhooks: Vec<ChannelIncomingWebhook>,
    pub outgoing_webhooks: Vec<ChannelOutgoingWebhook>,
    pub slash_commands: Vec<ChannelSlashCommand>,
    pub workflows: Vec<ChannelWorkflow>,
    pub can_add_webhook: bool,
}
