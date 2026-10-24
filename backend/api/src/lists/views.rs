use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use utoipa::ToSchema;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{ColumnKind, ListColumn, MAX_CHOICES};

pub const MAX_VIEWS: usize = 20;
pub const MAX_SORTS: usize = 3;
pub const MAX_FILTERS: usize = 10;
pub const VIEW_NAME_MAX_CHARS: usize = 50;
pub const FILTER_TEXT_MAX_CHARS: usize = 200;
pub const ME: &str = "me";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "list_view_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    Table,
    Board,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SortRule {
    pub column_id: Uuid,
    #[serde(default)]
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Contains,
    Is,
    IsNot,
    Lt,
    Gt,
    ThisWeek,
    Overdue,
    Empty,
    NotEmpty,
    Checked,
    Unchecked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FilterRule {
    pub column_id: Uuid,
    pub op: FilterOp,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub value: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ViewConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Uuid>,
    #[serde(default)]
    pub sort: Vec<SortRule>,
    #[serde(default)]
    pub filters: Vec<FilterRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListView {
    pub id: Uuid,
    pub list_id: Uuid,
    pub position: i32,
    pub name: String,
    pub kind: ViewKind,
    #[schema(value_type = ViewConfig)]
    pub config: Json<ViewConfig>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateViewRequest {
    pub name: String,
    pub kind: ViewKind,
    #[serde(default)]
    pub config: ViewConfig,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateViewRequest {
    pub name: Option<String>,
    pub kind: Option<ViewKind>,
    pub config: Option<ViewConfig>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ViewOrder {
    pub view_ids: Vec<Uuid>,
}

pub fn clean_view_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("A view needs a name".into()));
    }
    if name.chars().count() > VIEW_NAME_MAX_CHARS {
        return Err(AppError::Validation(format!(
            "A view name can be at most {VIEW_NAME_MAX_CHARS} characters"
        )));
    }
    Ok(name.to_string())
}

fn ids_of(value: &Value, strict: bool, known: impl Fn(&str) -> bool) -> Result<Value, String> {
    let given = value
        .as_array()
        .filter(|ids| !ids.is_empty() && ids.len() <= MAX_CHOICES)
        .ok_or("needs at least one value to compare with")?;
    let mut ids: Vec<Value> = Vec::with_capacity(given.len());
    for id in given {
        match id.as_str().filter(|id| known(id)) {
            Some(id) => {
                let id = Value::String(id.to_string());
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            None if strict => return Err("has no such choice".into()),
            None => {}
        }
    }
    if ids.is_empty() {
        return Err("needs at least one value to compare with".into());
    }
    Ok(Value::Array(ids))
}

fn filter_value(rule: &FilterRule, column: &ListColumn, strict: bool) -> Result<Value, String> {
    use ColumnKind::*;
    use FilterOp::*;
    match (rule.op, column.kind) {
        (Contains, Text | Link) => {
            let text = rule
                .value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .ok_or("needs text to look for")?;
            if text.chars().count() > FILTER_TEXT_MAX_CHARS {
                return Err(format!(
                    "looks for at most {FILTER_TEXT_MAX_CHARS} characters"
                ));
            }
            Ok(Value::String(text.to_string()))
        }
        (Is | IsNot, Select | MultiSelect) => ids_of(&rule.value, strict, |id| {
            column.options.choice(id).is_some()
        }),
        (Is | IsNot, Person) => ids_of(&rule.value, strict, |id| {
            id == ME || id.parse::<Uuid>().is_ok()
        }),
        (Lt | Gt, Number) => match &rule.value {
            Value::Number(n) if n.as_f64().is_some_and(f64::is_finite) => Ok(rule.value.clone()),
            _ => Err("needs a number to compare with".into()),
        },
        (Lt | Gt, Date) => rule
            .value
            .as_str()
            .and_then(|text| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok())
            .map(|date| Value::String(date.format("%Y-%m-%d").to_string()))
            .ok_or_else(|| "needs a date as YYYY-MM-DD".into()),
        (ThisWeek | Overdue, Date) | (Checked | Unchecked, Checkbox) => Ok(Value::Null),
        (Empty | NotEmpty, kind) if kind != Checkbox => Ok(Value::Null),
        _ => Err("cannot be filtered that way".into()),
    }
}

fn not_in_list() -> String {
    "That column is not in this list".into()
}

impl ViewConfig {
    pub fn check(&self, kind: ViewKind, columns: &[ListColumn]) -> AppResult<ViewConfig> {
        self.resolve(kind, columns, true)
            .map_err(AppError::Validation)
    }

    pub fn pruned(&self, kind: ViewKind, columns: &[ListColumn]) -> ViewConfig {
        self.resolve(kind, columns, false).unwrap_or_default()
    }

    fn resolve(
        &self,
        kind: ViewKind,
        columns: &[ListColumn],
        strict: bool,
    ) -> Result<ViewConfig, String> {
        let find = |id: Uuid| columns.iter().find(|column| column.id == id);
        if strict && self.sort.len() > MAX_SORTS {
            return Err(format!("A view sorts by at most {MAX_SORTS} columns"));
        }
        if strict && self.filters.len() > MAX_FILTERS {
            return Err(format!("A view has at most {MAX_FILTERS} filters"));
        }
        let mut config = ViewConfig::default();
        if let Some(id) = self.group_by {
            match (kind, find(id)) {
                (ViewKind::Board, Some(column)) if column.kind == ColumnKind::Select => {
                    config.group_by = Some(id);
                }
                (ViewKind::Table, _) if strict => {
                    return Err("Only a board groups its items".into());
                }
                _ if strict => {
                    return Err("A board groups by a single-select column".into());
                }
                _ => {}
            }
        }
        for rule in self.sort.iter().take(MAX_SORTS) {
            let Some(column) = find(rule.column_id) else {
                if strict {
                    return Err(not_in_list());
                }
                continue;
            };
            if config.sort.iter().any(|s| s.column_id == rule.column_id) {
                if strict {
                    return Err(format!("{} is sorted by twice", column.name));
                }
                continue;
            }
            config.sort.push(rule.clone());
        }
        for rule in self.filters.iter().take(MAX_FILTERS) {
            let Some(column) = find(rule.column_id) else {
                if strict {
                    return Err(not_in_list());
                }
                continue;
            };
            match filter_value(rule, column, strict) {
                Ok(value) => config.filters.push(FilterRule {
                    value,
                    ..rule.clone()
                }),
                Err(problem) if strict => return Err(format!("{} {problem}", column.name)),
                Err(_) => {}
            }
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lists::models::{Choice, ChoiceColor, ColumnOptions};
    use serde_json::json;

    fn column(name: &str, kind: ColumnKind, choices: Vec<Choice>) -> ListColumn {
        ListColumn {
            id: Uuid::new_v4(),
            list_id: Uuid::nil(),
            position: 0,
            name: name.into(),
            kind,
            options: Json(ColumnOptions {
                choices,
                number: None,
            }),
            hidden: false,
            due: false,
        }
    }

    fn choice(label: &str) -> Choice {
        Choice {
            id: Uuid::new_v4(),
            label: label.into(),
            color: ChoiceColor::Gray,
        }
    }

    fn filter(column: &ListColumn, op: FilterOp, value: Value) -> FilterRule {
        FilterRule {
            column_id: column.id,
            op,
            value,
        }
    }

    #[test]
    fn a_board_groups_by_a_single_select_only() {
        let status = column("Status", ColumnKind::Select, vec![choice("Done")]);
        let tags = column("Tags", ColumnKind::MultiSelect, vec![choice("Bug")]);
        let columns = vec![status.clone(), tags.clone()];
        let by = |id| ViewConfig {
            group_by: Some(id),
            ..Default::default()
        };

        assert_eq!(
            by(status.id).check(ViewKind::Board, &columns).unwrap(),
            by(status.id)
        );
        assert!(by(tags.id).check(ViewKind::Board, &columns).is_err());
        assert!(by(status.id).check(ViewKind::Table, &columns).is_err());
        assert_eq!(
            by(status.id).pruned(ViewKind::Table, &columns),
            ViewConfig::default()
        );
    }

    #[test]
    fn each_filter_fits_its_columns_kind() {
        let title = column("Task", ColumnKind::Text, vec![]);
        let due = column("Due", ColumnKind::Date, vec![]);
        let done = column("Done", ColumnKind::Checkbox, vec![]);
        let owner = column("Owner", ColumnKind::Person, vec![]);
        let columns = vec![title.clone(), due.clone(), done.clone(), owner.clone()];
        let check = |rule: FilterRule| {
            ViewConfig {
                filters: vec![rule],
                ..Default::default()
            }
            .check(ViewKind::Table, &columns)
        };

        let trimmed = check(filter(&title, FilterOp::Contains, json!("  login "))).unwrap();
        assert_eq!(trimmed.filters[0].value, json!("login"));
        assert!(check(filter(&title, FilterOp::Contains, json!(" "))).is_err());
        assert!(check(filter(&due, FilterOp::ThisWeek, Value::Null)).is_ok());
        assert!(check(filter(&due, FilterOp::Lt, json!("2026-02-30"))).is_err());
        assert!(check(filter(&title, FilterOp::Overdue, Value::Null)).is_err());
        assert!(check(filter(&done, FilterOp::Empty, Value::Null)).is_err());
        assert!(check(filter(&done, FilterOp::Checked, Value::Null)).is_ok());
        assert!(check(filter(&owner, FilterOp::Is, json!(["me"]))).is_ok());
        assert!(check(filter(&owner, FilterOp::Is, json!(["nobody"]))).is_err());
        assert!(check(filter(&owner, FilterOp::Is, json!([]))).is_err());
    }

    #[test]
    fn a_choice_filter_names_real_choices() {
        let done = choice("Done");
        let status = column("Status", ColumnKind::Select, vec![done.clone()]);
        let gone = Uuid::new_v4().to_string();
        let config = ViewConfig {
            filters: vec![filter(
                &status,
                FilterOp::Is,
                json!([done.id.to_string(), gone]),
            )],
            ..Default::default()
        };

        assert!(config
            .check(ViewKind::Table, std::slice::from_ref(&status))
            .is_err());
        let pruned = config.pruned(ViewKind::Table, std::slice::from_ref(&status));
        assert_eq!(pruned.filters[0].value, json!([done.id.to_string()]));
    }

    #[test]
    fn pruning_drops_what_no_longer_fits() {
        let status = column("Status", ColumnKind::Select, vec![choice("Done")]);
        let due = column("Due", ColumnKind::Date, vec![]);
        let config = ViewConfig {
            group_by: Some(status.id),
            sort: vec![SortRule {
                column_id: due.id,
                direction: SortDirection::Desc,
            }],
            filters: vec![filter(&due, FilterOp::Overdue, Value::Null)],
        };
        let mut due_as_text = due.clone();
        due_as_text.kind = ColumnKind::Text;

        let pruned = config.pruned(ViewKind::Board, &[due_as_text]);
        assert_eq!(pruned.group_by, None);
        assert_eq!(pruned.sort, config.sort);
        assert!(pruned.filters.is_empty());
    }

    #[test]
    fn a_view_keeps_its_rules_few_and_distinct() {
        let title = column("Task", ColumnKind::Text, vec![]);
        let sort = SortRule {
            column_id: title.id,
            direction: SortDirection::Asc,
        };
        let twice = ViewConfig {
            sort: vec![sort.clone(), sort],
            ..Default::default()
        };
        assert!(twice
            .check(ViewKind::Table, std::slice::from_ref(&title))
            .is_err());

        let many = ViewConfig {
            filters: vec![filter(&title, FilterOp::NotEmpty, Value::Null); MAX_FILTERS + 1],
            ..Default::default()
        };
        assert!(many
            .check(ViewKind::Table, std::slice::from_ref(&title))
            .is_err());
    }
}
