use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::workspace::models::{CreatedInvite, GuestKind, WorkspaceRole};

pub const LIFETIME_DAYS: i32 = 30;
pub const MAX_EMAILS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "invite_request_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum InviteRequestStatus {
    Pending,
    Approved,
    Declined,
    Expired,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkspaceInviteRequest {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub requested_by: Uuid,
    pub requester_name: String,
    pub emails: Vec<String>,
    pub role: WorkspaceRole,
    pub guest_kind: Option<GuestKind>,
    pub guest_until: Option<DateTime<Utc>>,
    pub channel_ids: Vec<Uuid>,
    pub channel_names: Vec<String>,
    pub reason: Option<String>,
    pub status: InviteRequestStatus,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct RequestTerms {
    #[garde(length(min = 1, max = MAX_EMAILS), inner(custom(shared_common::validation::rules::email)))]
    pub emails: Vec<String>,
    pub role: WorkspaceRole,
    pub guest_kind: Option<GuestKind>,
    pub guest_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub channel_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct AskForInvites {
    #[serde(flatten)]
    #[garde(dive)]
    pub terms: RequestTerms,
    #[garde(inner(custom(shared_common::validation::rules::description)))]
    pub reason: Option<String>,
}

#[derive(Debug, Default, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct ApproveInviteRequest {
    #[garde(dive)]
    pub terms: Option<RequestTerms>,
}

#[derive(Debug, Default, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct DeclineInviteRequest {
    #[garde(inner(custom(shared_common::validation::rules::description)))]
    pub note: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApprovedInviteRequest {
    pub request: WorkspaceInviteRequest,
    pub invites: Vec<CreatedInvite>,
}

pub struct ExpiredRequest {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub requested_by: Uuid,
    pub emails: Vec<String>,
}
