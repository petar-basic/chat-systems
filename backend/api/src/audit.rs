use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use shared_common::errors::AppError;

use crate::state::AppState;

/// A closed set rather than free strings: the read side filters on `action`, and
/// a typo in a string literal would silently drop an entry out of every query
/// that looks for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    MessageDeleted,
    MessageHistoryRead,
    ChannelCreated,
    ChannelArchived,
    ChannelUpdated,
    ChannelMemberAdded,
    ChannelMemberRemoved,
    ChannelRoleChanged,
    WorkspaceCreated,
    WorkspaceDeleted,
    WorkspaceRestored,
    WorkspaceMemberRemoved,
    WorkspaceRoleChanged,
    InviteCreated,
    InviteRevoked,
    HookCreated,
    HookDeleted,
    HookRevealed,
    HookRotated,
    FileDeleted,
    UserProvisioned,
    UserSuspended,
    UserActivated,
    InstanceRoleChanged,
    ExportRequested,
    ExportCompleted,
    UserDataErased,
    SsoLinked,
    TotpEnrolled,
    TotpDisabled,
    TotpFailed,
    TotpRecoveryUsed,
    CommandInvoked,
    GroupCreated,
    GroupUpdated,
    GroupDeleted,
    GroupMemberAdded,
    GroupMemberRemoved,
    EmojiCreated,
    EmojiDeleted,
    ScimTokenCreated,
    ScimTokenRevoked,
    ScimTokenRotated,
    RetentionChanged,
    RetentionPurge,
}

impl AuditAction {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MessageDeleted => "message.deleted",
            Self::MessageHistoryRead => "message.history_read",
            Self::ChannelCreated => "channel.created",
            Self::ChannelArchived => "channel.archived",
            Self::ChannelUpdated => "channel.updated",
            Self::ChannelMemberAdded => "channel.member_added",
            Self::ChannelMemberRemoved => "channel.member_removed",
            Self::ChannelRoleChanged => "channel.role_changed",
            Self::WorkspaceCreated => "workspace.created",
            Self::WorkspaceDeleted => "workspace.deleted",
            Self::WorkspaceRestored => "workspace.restored",
            Self::WorkspaceMemberRemoved => "workspace.member_removed",
            Self::WorkspaceRoleChanged => "workspace.role_changed",
            Self::InviteCreated => "invite.created",
            Self::InviteRevoked => "invite.revoked",
            Self::HookCreated => "hook.created",
            Self::HookDeleted => "hook.deleted",
            Self::HookRevealed => "hook.revealed",
            Self::HookRotated => "hook.rotated",
            Self::FileDeleted => "file.deleted",
            Self::UserProvisioned => "user.provisioned",
            Self::UserSuspended => "user.suspended",
            Self::UserActivated => "user.activated",
            Self::InstanceRoleChanged => "user.instance_role_changed",
            Self::ExportRequested => "export.requested",
            Self::ExportCompleted => "export.completed",
            Self::UserDataErased => "user.data_erased",
            Self::SsoLinked => "sso.linked",
            Self::TotpEnrolled => "totp.enrolled",
            Self::TotpDisabled => "totp.disabled",
            Self::TotpFailed => "totp.failed",
            Self::TotpRecoveryUsed => "totp.recovery_used",
            Self::CommandInvoked => "command.invoked",
            Self::GroupCreated => "group.created",
            Self::GroupUpdated => "group.updated",
            Self::GroupDeleted => "group.deleted",
            Self::GroupMemberAdded => "group.member_added",
            Self::GroupMemberRemoved => "group.member_removed",
            Self::EmojiCreated => "emoji.created",
            Self::EmojiDeleted => "emoji.deleted",
            Self::ScimTokenCreated => "scim.token_created",
            Self::ScimTokenRevoked => "scim.token_revoked",
            Self::ScimTokenRotated => "scim.token_rotated",
            Self::RetentionChanged => "retention.changed",
            Self::RetentionPurge => "retention.purge",
        }
    }

    fn resource_type(self) -> &'static str {
        match self {
            Self::MessageDeleted | Self::MessageHistoryRead => "message",
            Self::ChannelCreated
            | Self::ChannelArchived
            | Self::ChannelUpdated
            | Self::ChannelMemberAdded
            | Self::ChannelMemberRemoved
            | Self::ChannelRoleChanged => "channel",
            Self::WorkspaceCreated
            | Self::WorkspaceDeleted
            | Self::WorkspaceRestored
            | Self::WorkspaceMemberRemoved
            | Self::WorkspaceRoleChanged => "workspace",
            Self::InviteCreated | Self::InviteRevoked => "invite",
            Self::HookCreated
            | Self::HookDeleted
            | Self::HookRevealed
            | Self::HookRotated
            | Self::CommandInvoked => "hook",
            Self::FileDeleted => "file",
            Self::EmojiCreated | Self::EmojiDeleted => "emoji",
            Self::GroupCreated
            | Self::GroupUpdated
            | Self::GroupDeleted
            | Self::GroupMemberAdded
            | Self::GroupMemberRemoved => "group",
            Self::UserProvisioned
            | Self::UserSuspended
            | Self::UserActivated
            | Self::InstanceRoleChanged
            | Self::UserDataErased
            | Self::SsoLinked
            | Self::TotpEnrolled
            | Self::TotpDisabled
            | Self::TotpFailed
            | Self::TotpRecoveryUsed => "user",
            Self::ScimTokenCreated | Self::ScimTokenRevoked | Self::ScimTokenRotated => "scim",
            Self::RetentionChanged | Self::RetentionPurge => "retention",
            Self::ExportRequested | Self::ExportCompleted => "export",
        }
    }
}

/// Resolved through the same trusted-proxy rules the rate limit uses, so a
/// caller cannot write a chosen address into the trail. Optional because the
/// extractor must never be the reason a request fails.
#[derive(Debug, Clone, Default)]
pub struct ClientIp(pub Option<String>);

impl FromRequestParts<Arc<AppState>> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0);
        let trusted = crate::net::parse_trusted_proxies(&state.config.trusted_proxies);
        Ok(Self(crate::net::client_ip(&parts.headers, peer, &trusted)))
    }
}

pub struct AuditEntry {
    action: AuditAction,
    actor_id: Option<Uuid>,
    workspace_id: Option<Uuid>,
    resource_id: Option<Uuid>,
    ip: Option<String>,
    details: serde_json::Value,
}

impl AuditEntry {
    #[must_use]
    pub fn new(action: AuditAction, actor_id: Uuid) -> Self {
        Self {
            action,
            actor_id: Some(actor_id),
            workspace_id: None,
            resource_id: None,
            ip: None,
            details: serde_json::json!({}),
        }
    }

    /// For callers that are not people. A provisioning system acting through a
    /// token has no user row to point at, and borrowing one would name somebody
    /// who did not do it.
    #[must_use]
    pub fn machine(action: AuditAction) -> Self {
        Self {
            action,
            actor_id: None,
            workspace_id: None,
            resource_id: None,
            ip: None,
            details: serde_json::json!({}),
        }
    }

    #[must_use]
    pub fn workspace(mut self, workspace_id: Uuid) -> Self {
        self.workspace_id = Some(workspace_id);
        self
    }

    #[must_use]
    pub fn resource(mut self, resource_id: Uuid) -> Self {
        self.resource_id = Some(resource_id);
        self
    }

    #[must_use]
    pub fn ip(mut self, ip: &ClientIp) -> Self {
        self.ip.clone_from(&ip.0);
        self
    }

    #[must_use]
    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }
}

/// Never fails the originating request. The action has already happened by the
/// time we get here, so a database hiccup on the trail must not turn a
/// successful deletion into a 500 the caller will retry.
pub async fn record(state: &AppState, entry: AuditEntry) {
    let result = sqlx::query(
        "INSERT INTO audit_log \
         (workspace_id, user_id, action, resource_type, resource_id, details, ip_address) \
         VALUES ($1, $2, $3, $4, $5, $6, $7::text::inet)",
    )
    .bind(entry.workspace_id)
    .bind(entry.actor_id)
    .bind(entry.action.as_str())
    .bind(entry.action.resource_type())
    .bind(entry.resource_id)
    .bind(&entry.details)
    .bind(&entry.ip)
    .execute(&state.pool)
    .await;

    if let Err(e) = result {
        tracing::warn!(action = entry.action.as_str(), "audit write failed: {}", e);
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditRow {
    pub id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub actor_email: Option<String>,
    pub actor_display_name: Option<String>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Keyset pagination on `(created_at, id)` — the pair the index is ordered by,
/// so a page boundary that lands inside a batch of same-timestamp rows does not
/// skip or repeat one.
#[derive(Debug, Default, Deserialize)]
pub struct AuditQuery {
    pub workspace_id: Option<Uuid>,
    pub action: Option<String>,
    pub user_id: Option<Uuid>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub before_id: Option<Uuid>,
    pub limit: Option<i64>,
}

pub async fn list(
    state: &AppState,
    workspace_id: Option<Uuid>,
    query: &AuditQuery,
) -> Result<Vec<AuditRow>, AppError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);

    let rows = sqlx::query_as::<_, AuditRow>(
        "SELECT a.id, a.workspace_id, a.user_id, u.email AS actor_email, \
                u.display_name AS actor_display_name, a.action, a.resource_type, \
                a.resource_id, a.details, host(a.ip_address) AS ip_address, a.created_at \
           FROM audit_log a \
           LEFT JOIN users u ON u.id = a.user_id \
          WHERE ($1::uuid IS NULL OR a.workspace_id = $1) \
            AND ($2::text IS NULL OR a.action = $2) \
            AND ($3::uuid IS NULL OR a.user_id = $3) \
            AND ($4::timestamptz IS NULL OR a.created_at >= $4) \
            AND ($5::timestamptz IS NULL OR a.created_at <= $5) \
            AND ($6::timestamptz IS NULL \
                 OR (a.created_at, a.id) < ($6, COALESCE($7::uuid, '00000000-0000-0000-0000-000000000000'))) \
          ORDER BY a.created_at DESC, a.id DESC \
          LIMIT $8",
    )
    .bind(workspace_id)
    .bind(query.action.as_deref())
    .bind(query.user_id)
    .bind(query.since)
    .bind(query.until)
    .bind(query.before)
    .bind(query.before_id)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows)
}
