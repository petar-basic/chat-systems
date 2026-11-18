use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_PER_KIND: usize = 5;
const MAX_NAME: usize = 50;
const MAX_DESCRIPTION: usize = 150;
const MAX_CALLBACK_ID: usize = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutKind {
    Global,
    Message,
}

impl ShortcutKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Message => "message",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw {
            "message" => Self::Message,
            _ => Self::Global,
        }
    }

    pub fn payload_type(self) -> &'static str {
        match self {
            Self::Global => "shortcut",
            Self::Message => "message_action",
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AppShortcut {
    pub id: Uuid,
    pub app_id: Uuid,
    pub kind: ShortcutKind,
    pub name: String,
    pub description: String,
    pub callback_id: String,
}

pub struct AppShortcutRow {
    pub id: Uuid,
    pub app_id: Uuid,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub callback_id: String,
}

impl From<AppShortcutRow> for AppShortcut {
    fn from(row: AppShortcutRow) -> Self {
        AppShortcut {
            id: row.id,
            app_id: row.app_id,
            kind: ShortcutKind::parse(&row.kind),
            name: row.name,
            description: row.description,
            callback_id: row.callback_id,
        }
    }
}

fn bounded(value: &str, max: usize, what: &str) -> garde::Result {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(garde::Error::new(format!("{what} cannot be empty")));
    }
    if trimmed.chars().count() > max {
        return Err(garde::Error::new(format!(
            "{what} must be at most {max} characters"
        )));
    }
    Ok(())
}

fn shortcut_name(value: &str, _: &()) -> garde::Result {
    bounded(value, MAX_NAME, "A shortcut's name")
}

fn shortcut_description(value: &str, _: &()) -> garde::Result {
    bounded(value, MAX_DESCRIPTION, "A shortcut's description")
}

fn shortcut_callback_id(value: &str, _: &()) -> garde::Result {
    bounded(value, MAX_CALLBACK_ID, "A callback ID")?;
    if value.chars().any(char::is_whitespace) {
        return Err(garde::Error::new("A callback ID has no spaces"));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct AppShortcutInput {
    #[garde(skip)]
    pub kind: ShortcutKind,
    #[garde(custom(shortcut_name))]
    pub name: String,
    #[garde(custom(shortcut_description))]
    pub description: String,
    /// Sent back in the payload, so the app knows which shortcut was used.
    #[garde(custom(shortcut_callback_id))]
    pub callback_id: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct SaveShortcutsRequest {
    /// Every shortcut the app has, in the order they are listed; at most five
    /// of each kind.
    #[garde(dive)]
    pub shortcuts: Vec<AppShortcutInput>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct RunShortcutRequest {
    /// The conversation it was used from, if any.
    #[serde(default)]
    pub channel_id: Option<Uuid>,
    /// The message a message shortcut was used on.
    #[serde(default)]
    pub message_id: Option<Uuid>,
}
