use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use utoipa::ToSchema;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::views::ListView;

pub const MAX_COLUMNS: usize = 30;
pub const MAX_ITEMS: i64 = 10_000;
pub const MAX_CHOICES: usize = 50;
pub const TITLE_MAX_CHARS: usize = 200;
pub const DESCRIPTION_MAX_CHARS: usize = 2000;
pub const COLUMN_NAME_MAX_CHARS: usize = 100;
pub const CHOICE_LABEL_MAX_CHARS: usize = 100;
pub const TEXT_MAX_CHARS: usize = 4000;
pub const LINK_MAX_CHARS: usize = 2000;
pub const MAX_DECIMALS: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "list_column_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    Text,
    Number,
    Select,
    MultiSelect,
    Person,
    Date,
    Checkbox,
    Link,
}

impl ColumnKind {
    pub fn has_choices(self) -> bool {
        matches!(self, Self::Select | Self::MultiSelect)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChoiceColor {
    #[default]
    Gray,
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Choice {
    pub id: Uuid,
    pub label: String,
    pub color: ChoiceColor,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NumberStyle {
    #[default]
    Plain,
    Percent,
    Currency,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct NumberFormat {
    #[serde(default)]
    pub style: NumberStyle,
    #[serde(default)]
    pub decimals: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ColumnOptions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Choice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<NumberFormat>,
}

impl ColumnOptions {
    pub fn choice(&self, id: &str) -> Option<&Choice> {
        let id = id.parse::<Uuid>().ok()?;
        self.choices.iter().find(|choice| choice.id == id)
    }

    pub fn choice_labelled(&self, label: &str) -> Option<&Choice> {
        let label = label.trim().to_lowercase();
        self.choices
            .iter()
            .find(|choice| choice.label.to_lowercase() == label)
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ChoiceInput {
    pub id: Option<Uuid>,
    pub label: String,
    #[serde(default)]
    pub color: ChoiceColor,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct ColumnOptionsInput {
    #[serde(default)]
    pub choices: Vec<ChoiceInput>,
    pub number: Option<NumberFormat>,
}

impl ColumnOptionsInput {
    pub fn resolve(&self, kind: ColumnKind) -> AppResult<ColumnOptions> {
        let mut options = ColumnOptions::default();
        if kind.has_choices() {
            if self.choices.len() > MAX_CHOICES {
                return Err(AppError::Validation(format!(
                    "A column can offer at most {MAX_CHOICES} choices"
                )));
            }
            let mut seen: Vec<String> = Vec::with_capacity(self.choices.len());
            for input in &self.choices {
                let label = input.label.trim();
                if label.is_empty() {
                    return Err(AppError::Validation("A choice needs a label".into()));
                }
                if label.chars().count() > CHOICE_LABEL_MAX_CHARS {
                    return Err(AppError::Validation(format!(
                        "A choice can be at most {CHOICE_LABEL_MAX_CHARS} characters"
                    )));
                }
                let folded = label.to_lowercase();
                if seen.contains(&folded) {
                    return Err(AppError::Validation(format!(
                        "\"{label}\" is offered twice"
                    )));
                }
                seen.push(folded);
                options.choices.push(Choice {
                    id: input.id.unwrap_or_else(Uuid::new_v4),
                    label: label.to_string(),
                    color: input.color,
                });
            }
        }
        if kind == ColumnKind::Number {
            let format = self.number.clone().unwrap_or_default();
            if format.decimals > MAX_DECIMALS {
                return Err(AppError::Validation(format!(
                    "A number shows at most {MAX_DECIMALS} decimals"
                )));
            }
            let currency = match format.style {
                NumberStyle::Currency => {
                    let code = format.currency.as_deref().unwrap_or("").trim();
                    if code.len() != 3 || !code.chars().all(|c| c.is_ascii_alphabetic()) {
                        return Err(AppError::Validation(
                            "A currency is a three-letter code, like EUR".into(),
                        ));
                    }
                    Some(code.to_ascii_uppercase())
                }
                _ => None,
            };
            options.number = Some(NumberFormat { currency, ..format });
        }
        Ok(options)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct List {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListColumn {
    pub id: Uuid,
    pub list_id: Uuid,
    pub position: i32,
    pub name: String,
    pub kind: ColumnKind,
    #[schema(value_type = ColumnOptions)]
    pub options: Json<ColumnOptions>,
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListItem {
    pub id: Uuid,
    pub list_id: Uuid,
    pub rank: String,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub values: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListDetail {
    pub list: List,
    pub columns: Vec<ListColumn>,
    pub views: Vec<ListView>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewColumn {
    pub name: String,
    pub kind: ColumnKind,
    #[serde(default)]
    pub options: ColumnOptionsInput,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateListRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub channel_id: Option<Uuid>,
    #[serde(default)]
    pub columns: Vec<NewColumn>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateListRequest {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListsQuery {
    pub channel_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateColumnRequest {
    pub name: Option<String>,
    pub kind: Option<ColumnKind>,
    pub options: Option<ColumnOptionsInput>,
    pub hidden: Option<bool>,
    pub confirm_cleared: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ColumnChangePreview {
    pub kind: ColumnKind,
    #[serde(default)]
    pub options: ColumnOptionsInput,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConversionCounts {
    pub converted: i64,
    pub cleared: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ColumnOrder {
    pub column_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateItemRequest {
    #[serde(default)]
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub values: serde_json::Map<String, serde_json::Value>,
    pub after_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateItemRequest {
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub values: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveItemRequest {
    pub after_id: Option<Uuid>,
}

pub fn clean_title(title: &str) -> AppResult<String> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("A list needs a title".into()));
    }
    if title.chars().count() > TITLE_MAX_CHARS {
        return Err(AppError::Validation(format!(
            "A title can be at most {TITLE_MAX_CHARS} characters"
        )));
    }
    Ok(title.to_string())
}

pub fn clean_description(description: &str) -> AppResult<String> {
    let description = description.trim();
    if description.chars().count() > DESCRIPTION_MAX_CHARS {
        return Err(AppError::Validation(format!(
            "A description can be at most {DESCRIPTION_MAX_CHARS} characters"
        )));
    }
    Ok(description.to_string())
}

pub fn clean_column_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("A column needs a name".into()));
    }
    if name.chars().count() > COLUMN_NAME_MAX_CHARS {
        return Err(AppError::Validation(format!(
            "A column name can be at most {COLUMN_NAME_MAX_CHARS} characters"
        )));
    }
    Ok(name.to_string())
}
