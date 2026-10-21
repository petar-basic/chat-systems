use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SidebarSection {
    pub id: Uuid,
    pub name: String,
    pub position: i32,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChannelPlacement {
    pub channel_id: Uuid,
    pub section_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SidebarLayout {
    pub sections: Vec<SidebarSection>,
    pub placements: Vec<ChannelPlacement>,
    pub collapsed_builtins: Vec<BuiltinSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BuiltinSection {
    Channels,
    Dms,
    People,
}

impl BuiltinSection {
    pub fn as_str(self) -> &'static str {
        match self {
            BuiltinSection::Channels => "channels",
            BuiltinSection::Dms => "dms",
            BuiltinSection::People => "people",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "channels" => Some(BuiltinSection::Channels),
            "dms" => Some(BuiltinSection::Dms),
            "people" => Some(BuiltinSection::People),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSectionRequest {
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSectionRequest {
    pub name: Option<String>,
    pub collapsed: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderSectionsRequest {
    pub section_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PlaceChannelRequest {
    pub section_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CollapseBuiltinRequest {
    pub section: BuiltinSection,
    pub collapsed: bool,
}
