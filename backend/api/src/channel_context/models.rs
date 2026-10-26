use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::lists::models::List;
use crate::messaging::models::Message;
use crate::workflows::models::Shortcut;
use crate::workspace::models::{ChannelBookmark, ChannelMember};

#[derive(Debug, Serialize, ToSchema)]
pub struct ChannelContext {
    pub workspace_id: Uuid,
    pub members: Vec<ChannelMember>,
    pub pins: Vec<Message>,
    pub bookmarks: Vec<ChannelBookmark>,
    pub lists: Vec<List>,
    pub workflow_shortcuts: Vec<Shortcut>,
}
