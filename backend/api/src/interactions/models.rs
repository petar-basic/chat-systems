use std::collections::HashMap;

use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::blocks::models::View;

pub type Values = HashMap<String, HashMap<String, Vec<String>>>;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ActionRequest {
    pub block_id: String,
    pub action_id: String,
    /// The chosen option's value, person, conversation or `YYYY-MM-DD` date;
    /// nothing for a button.
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ViewActionRequest {
    pub block_id: String,
    pub action_id: String,
    #[serde(default)]
    pub value: Option<String>,
    /// What the inputs hold now, by `block_id` and `action_id`.
    #[serde(default)]
    #[schema(value_type = HashMap<String, HashMap<String, Vec<String>>>)]
    pub values: Values,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitViewRequest {
    /// Each input's value by `block_id` and `action_id`: one item for a single
    /// choice, every chosen value for checkboxes, none when it is empty.
    #[serde(default)]
    #[schema(value_type = HashMap<String, HashMap<String, Vec<String>>>)]
    pub values: Values,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Submitted {
    Closed,
    Updated,
    Pushed,
    Cleared,
    Errors,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SubmitOutcome {
    pub outcome: Submitted,
    /// The message to show under each input, by `block_id`.
    pub errors: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct CloseViewRequest {
    /// Close every view in the stack rather than going back one.
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OpenViewRequest {
    pub trigger_id: Uuid,
    #[schema(value_type = View)]
    pub view: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAppViewRequest {
    #[serde(default)]
    pub view_id: Option<Uuid>,
    #[serde(default)]
    pub external_id: Option<String>,
    /// The `hash` the app last saw; the update is refused when the view has
    /// changed since.
    #[serde(default)]
    pub hash: Option<String>,
    #[schema(value_type = View)]
    pub view: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AppView {
    pub id: Uuid,
    pub app_id: Uuid,
    pub root_view_id: Uuid,
    pub previous_view_id: Option<Uuid>,
    pub hash: String,
    #[serde(flatten)]
    pub view: View,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InteractiveApp {
    pub app_id: Uuid,
    pub bot_user_id: Uuid,
    pub hook_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct InteractionAnswer {
    #[garde(inner(custom(shared_common::validation::rules::message_body)))]
    #[serde(default)]
    pub text: Option<String>,
    #[garde(skip)]
    #[schema(value_type = Option<Vec<crate::blocks::models::Block>>)]
    pub blocks: Option<serde_json::Value>,
    #[garde(skip)]
    #[schema(value_type = Option<Vec<Object>>)]
    pub attachments: Option<serde_json::Value>,
    /// `in_channel` posts a new message; anything else is shown only to
    /// whoever used the element.
    #[garde(skip)]
    #[serde(default)]
    pub response_type: Option<String>,
    #[garde(skip)]
    #[serde(default)]
    pub replace_original: bool,
    #[garde(skip)]
    #[serde(default)]
    pub delete_original: bool,
}

pub struct StoredView {
    pub id: Uuid,
    pub stack_id: Uuid,
    pub position: i16,
    pub app_id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub view: serde_json::Value,
    pub hash: String,
}

pub struct Trigger {
    pub user_id: Uuid,
    pub stack_id: Option<Uuid>,
    pub invocation_id: Option<Uuid>,
}

pub struct UsedInteraction {
    pub app_id: Uuid,
    pub user_id: Uuid,
    pub message_id: Uuid,
}
