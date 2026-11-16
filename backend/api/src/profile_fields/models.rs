use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub const MAX_FIELDS: i64 = 50;
pub const MAX_ON_CARD: i64 = 2;
pub const MAX_OPTIONS: usize = 100;
pub const MAX_LABEL: usize = 50;
pub const MAX_HINT: usize = 150;
pub const MAX_OPTION_LABEL: usize = 80;
pub const MAX_TEXT: usize = 200;
pub const MAX_LINK: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "profile_field_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ProfileFieldKind {
    Text,
    Link,
    Date,
    Options,
    Person,
}

impl ProfileFieldKind {
    pub fn filterable(self) -> bool {
        matches!(self, Self::Options | Self::Person)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProfileFieldOption {
    pub id: Uuid,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProfileField {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub label: String,
    pub hint: Option<String>,
    pub kind: ProfileFieldKind,
    #[schema(value_type = Vec<ProfileFieldOption>)]
    pub options: Json<Vec<ProfileFieldOption>>,
    pub admins_only: bool,
    pub on_card: bool,
    pub position: i32,
    pub value_count: i64,
}

impl ProfileField {
    pub fn has_option(&self, id: Uuid) -> bool {
        self.options.iter().any(|o| o.id == id)
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ProfileFieldOptionInput {
    pub id: Option<Uuid>,
    pub label: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewProfileField {
    pub label: String,
    pub hint: Option<String>,
    pub kind: ProfileFieldKind,
    #[serde(default)]
    pub options: Vec<ProfileFieldOptionInput>,
    #[serde(default)]
    pub admins_only: bool,
    #[serde(default)]
    pub on_card: bool,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct UpdateProfileField {
    pub label: Option<String>,
    pub hint: Option<String>,
    pub options: Option<Vec<ProfileFieldOptionInput>>,
    pub admins_only: Option<bool>,
    pub on_card: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OrderProfileFields {
    pub field_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetProfileValues {
    #[schema(value_type = HashMap<String, Option<String>>)]
    pub values: HashMap<Uuid, Option<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProfileValues {
    pub user_id: Uuid,
    #[schema(value_type = HashMap<String, String>)]
    pub values: HashMap<Uuid, String>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct MembersQuery {
    pub q: Option<String>,
    pub filter: Option<String>,
}
