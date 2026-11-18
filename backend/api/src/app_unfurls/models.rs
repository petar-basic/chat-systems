use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_DOMAINS: usize = 5;
pub const SHARE_TTL_SECS: f64 = 30.0 * 60.0;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UnfurlDomains {
    pub domains: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AnswerUnfurlsRequest {
    /// The preview of each link, by the URL as it was sent in `link_shared`.
    #[schema(value_type = BTreeMap<String, UnfurlAnswer>)]
    pub unfurls: BTreeMap<String, UnfurlAnswer>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UnfurlAnswer {
    /// Header, section, divider, image and context blocks.
    #[schema(value_type = Vec<Object>)]
    pub blocks: serde_json::Value,
}

pub struct Claim {
    pub app_id: Uuid,
    pub bot_user_id: Uuid,
    pub domain: String,
    pub scopes: Vec<String>,
    pub events: Vec<String>,
    pub reachable: bool,
    pub is_member: bool,
}
