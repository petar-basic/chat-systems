pub mod models;
pub mod repo;
pub mod routes;

use std::collections::HashMap;

use chrono::NaiveDate;
use serde_json::Value;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::state::AppState;
use crate::workflows::models::TEXT_MAX_CHARS;
use crate::workspace::models::ChannelType;
use models::{AppWorkflowStep, StepParam, StepParamKind, StepProvider};

pub async fn providers(state: &AppState, workspace_id: Uuid) -> AppResult<Vec<StepProvider>> {
    let (apps, steps) = state.app_workflow_step_repo.providers(workspace_id).await?;
    let mut by_app: HashMap<Uuid, Vec<AppWorkflowStep>> = HashMap::new();
    for row in steps {
        by_app
            .entry(row.app_id)
            .or_default()
            .push(AppWorkflowStep::from(row));
    }
    Ok(apps
        .into_iter()
        .map(|app| StepProvider {
            problem: app.problem(),
            steps: by_app.remove(&app.id).unwrap_or_default(),
            app_id: app.id,
            name: app.name,
            icon_url: app.icon_url,
        })
        .collect())
}

fn refused(name: &str, kind: StepParamKind) -> AppError {
    AppError::Validation(format!("outputs.{name} needs {}", kind.noun()))
}

pub async fn checked_outputs(
    state: &AppState,
    workspace_id: Uuid,
    declared: &[StepParam],
    given: &HashMap<String, Value>,
) -> AppResult<Value> {
    if let Some(name) = given
        .keys()
        .find(|name| !declared.iter().any(|param| &param.name == *name))
    {
        return Err(AppError::Validation(format!(
            "outputs.{name}: the step has no such output"
        )));
    }
    let mut checked = serde_json::Map::new();
    for param in declared {
        let name = param.name.as_str();
        let value = given.get(name).filter(|value| !value.is_null());
        let Some(value) = value else {
            if param.required {
                return Err(AppError::Validation(format!("outputs.{name} is required")));
            }
            checked.insert(name.to_string(), Value::Null);
            continue;
        };
        let wrong = || refused(name, param.kind);
        let value = match param.kind {
            StepParamKind::Text => {
                let text = value.as_str().ok_or_else(wrong)?;
                if text.chars().count() > TEXT_MAX_CHARS {
                    return Err(AppError::Validation(format!(
                        "outputs.{name} can be at most {TEXT_MAX_CHARS} characters"
                    )));
                }
                Value::String(text.to_string())
            }
            StepParamKind::Number => {
                if !value.as_f64().is_some_and(f64::is_finite) {
                    return Err(wrong());
                }
                value.clone()
            }
            StepParamKind::Boolean => Value::Bool(value.as_bool().ok_or_else(wrong)?),
            StepParamKind::Date => {
                let date = value
                    .as_str()
                    .and_then(|text| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok())
                    .ok_or_else(wrong)?;
                Value::String(date.format("%Y-%m-%d").to_string())
            }
            StepParamKind::Person => {
                let id: Uuid = value
                    .as_str()
                    .and_then(|text| text.parse().ok())
                    .ok_or_else(wrong)?;
                state
                    .workspace_service
                    .repo
                    .get_member(workspace_id, id)
                    .await?
                    .ok_or_else(|| {
                        AppError::Validation(format!(
                            "outputs.{name}: that person isn't in this workspace"
                        ))
                    })?;
                Value::String(id.to_string())
            }
            StepParamKind::Channel => {
                let id: Uuid = value
                    .as_str()
                    .and_then(|text| text.parse().ok())
                    .ok_or_else(wrong)?;
                let known = state
                    .workspace_service
                    .repo
                    .find_channel_by_id(id)
                    .await?
                    .is_some_and(|channel| {
                        channel.workspace_id == workspace_id
                            && matches!(
                                channel.channel_type,
                                ChannelType::Public | ChannelType::Private
                            )
                    });
                if !known {
                    return Err(AppError::Validation(format!(
                        "outputs.{name}: that channel isn't in this workspace"
                    )));
                }
                Value::String(id.to_string())
            }
        };
        checked.insert(name.to_string(), value);
    }
    Ok(Value::Object(checked))
}
