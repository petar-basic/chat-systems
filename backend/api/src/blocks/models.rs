use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextKind {
    PlainText,
    Mrkdwn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct BlockText {
    #[serde(rename = "type")]
    pub kind: TextKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct BlockOption {
    pub text: BlockText,
    #[serde(default)]
    pub value: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ButtonStyle {
    Primary,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockElement {
    Button {
        text: BlockText,
        url: Option<String>,
        style: Option<ButtonStyle>,
        action_id: Option<String>,
        value: Option<String>,
    },
    StaticSelect {
        placeholder: Option<BlockText>,
        #[serde(default)]
        options: Vec<BlockOption>,
        initial_option: Option<BlockOption>,
        action_id: Option<String>,
    },
    UsersSelect {
        placeholder: Option<BlockText>,
        initial_user: Option<String>,
        action_id: Option<String>,
    },
    ConversationsSelect {
        placeholder: Option<BlockText>,
        initial_conversation: Option<String>,
        action_id: Option<String>,
    },
    Overflow {
        #[serde(default)]
        options: Vec<BlockOption>,
        action_id: Option<String>,
    },
    Datepicker {
        placeholder: Option<BlockText>,
        initial_date: Option<String>,
        action_id: Option<String>,
    },
    Image {
        image_url: String,
        alt_text: String,
    },
    PlainTextInput {
        placeholder: Option<BlockText>,
        initial_value: Option<String>,
        #[serde(default)]
        multiline: bool,
        min_length: Option<u32>,
        max_length: Option<u32>,
        action_id: Option<String>,
    },
    Checkboxes {
        #[serde(default)]
        options: Vec<BlockOption>,
        #[serde(default)]
        initial_options: Vec<BlockOption>,
        action_id: Option<String>,
    },
    RadioButtons {
        #[serde(default)]
        options: Vec<BlockOption>,
        initial_option: Option<BlockOption>,
        action_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextElement {
    PlainText { text: String },
    Mrkdwn { text: String },
    Image { image_url: String, alt_text: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Header {
        text: BlockText,
        block_id: Option<String>,
    },
    Section {
        text: Option<BlockText>,
        #[serde(default)]
        fields: Vec<BlockText>,
        accessory: Option<BlockElement>,
        block_id: Option<String>,
    },
    Divider {
        block_id: Option<String>,
    },
    Image {
        image_url: String,
        alt_text: String,
        title: Option<BlockText>,
        block_id: Option<String>,
    },
    Context {
        elements: Vec<ContextElement>,
        block_id: Option<String>,
    },
    Actions {
        elements: Vec<BlockElement>,
        block_id: Option<String>,
    },
    Input {
        label: BlockText,
        element: BlockElement,
        hint: Option<BlockText>,
        #[serde(default)]
        optional: bool,
        block_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct LegacyField {
    pub title: String,
    pub value: String,
    pub short: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct LegacyAttachment {
    pub color: Option<String>,
    pub pretext: Option<String>,
    pub author_name: Option<String>,
    pub author_link: Option<String>,
    pub title: Option<String>,
    pub title_link: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub fields: Vec<LegacyField>,
    pub image_url: Option<String>,
    pub thumb_url: Option<String>,
    pub footer: Option<String>,
    pub ts: Option<i64>,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub blocks: Vec<Block>,
    pub legacy_attachments: Vec<LegacyAttachment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Composed {
    pub content: String,
    pub layout: Option<Layout>,
    pub flattened: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ViewType {
    Modal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct View {
    #[serde(rename = "type")]
    pub kind: ViewType,
    pub title: BlockText,
    pub submit: Option<BlockText>,
    pub close: Option<BlockText>,
    pub blocks: Vec<Block>,
    pub private_metadata: Option<String>,
    pub callback_id: Option<String>,
    pub external_id: Option<String>,
    #[serde(default)]
    pub notify_on_close: bool,
    #[serde(default)]
    pub clear_on_close: bool,
}
