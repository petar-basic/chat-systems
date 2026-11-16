use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::files::models::FileKind;
use crate::messaging::models::SearchHas;

pub const KEPT: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "search_tab", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum SearchTab {
    Messages,
    Files,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchFilters {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Uuid>,
    #[serde(rename = "in", default, skip_serializing_if = "Option::is_none")]
    pub place: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has: Option<SearchHas>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<FileKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<NaiveDate>,
}

impl SearchFilters {
    pub fn for_tab(self, tab: SearchTab) -> Self {
        let dated = self.on.is_none();
        Self {
            has: self.has.filter(|_| tab == SearchTab::Messages),
            kind: self.kind.filter(|_| tab == SearchTab::Files),
            after: self.after.filter(|_| dated),
            before: self.before.filter(|_| dated),
            ..self
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RecentSearch {
    pub id: Uuid,
    pub query: String,
    pub tab: SearchTab,
    #[schema(value_type = SearchFilters)]
    pub filters: Json<SearchFilters>,
    pub searched_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct RecordSearchRequest {
    #[garde(length(chars, max = 500))]
    pub query: String,
    pub tab: SearchTab,
    #[serde(default)]
    pub filters: SearchFilters,
}
