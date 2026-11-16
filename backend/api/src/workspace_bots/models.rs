use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "workspace_bot_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceBotKind {
    Responses,
}

impl WorkspaceBotKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Responses => "responses",
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkspaceBot {
    pub user_id: Option<Uuid>,
    pub name: String,
    pub avatar_url: Option<String>,
}
