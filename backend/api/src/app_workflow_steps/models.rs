use std::collections::{HashMap, HashSet};

use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

pub const MAX_STEPS: usize = 20;
pub const MAX_PARAMS: usize = 10;
pub const STEP_TIMEOUT_SECS: f64 = 60.0 * 60.0;
pub const ERROR_MAX_CHARS: usize = 500;
const TITLE_MAX_CHARS: usize = 80;
const DESCRIPTION_MAX_CHARS: usize = 300;
const CALLBACK_ID_MAX_CHARS: usize = 255;
const PARAM_NAME_MAX_CHARS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepParamKind {
    Text,
    Number,
    Boolean,
    Person,
    Channel,
    Date,
}

impl StepParamKind {
    pub fn noun(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Number => "a number",
            Self::Boolean => "true or false",
            Self::Person => "a person",
            Self::Channel => "a channel",
            Self::Date => "a date",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Validate, ToSchema)]
pub struct StepParam {
    /// Its key in `inputs` or `outputs`: lowercase letters, digits and `_`.
    #[garde(custom(param_name))]
    pub name: String,
    /// What the workflow builder calls it.
    #[garde(custom(param_title))]
    pub title: String,
    #[garde(skip)]
    pub kind: StepParamKind,
    #[serde(default)]
    #[garde(skip)]
    pub required: bool,
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

fn param_name(value: &str, _: &()) -> garde::Result {
    let mut chars = value.chars();
    let fits = value.chars().count() <= PARAM_NAME_MAX_CHARS
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !fits {
        return Err(garde::Error::new(format!(
            "\"{value}\" can't be a name: use up to {PARAM_NAME_MAX_CHARS} lowercase letters, digits and _"
        )));
    }
    Ok(())
}

fn param_title(value: &str, _: &()) -> garde::Result {
    bounded(value, TITLE_MAX_CHARS, "An input's or output's title")
}

fn step_title(value: &str, _: &()) -> garde::Result {
    bounded(value, TITLE_MAX_CHARS, "A step's title")
}

fn step_description(value: &str, _: &()) -> garde::Result {
    if value.trim().chars().count() > DESCRIPTION_MAX_CHARS {
        return Err(garde::Error::new(format!(
            "A step's description must be at most {DESCRIPTION_MAX_CHARS} characters"
        )));
    }
    Ok(())
}

fn step_callback_id(value: &str, _: &()) -> garde::Result {
    bounded(value, CALLBACK_ID_MAX_CHARS, "A callback ID")?;
    if value.chars().any(char::is_whitespace) {
        return Err(garde::Error::new("A callback ID has no spaces"));
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct WorkflowStepInput {
    /// Sent back in `function_executed`, so the app knows which step to run.
    #[garde(custom(step_callback_id))]
    pub callback_id: String,
    #[garde(custom(step_title))]
    pub title: String,
    #[serde(default)]
    #[garde(custom(step_description))]
    pub description: String,
    /// What a workflow hands the step; at most ten.
    #[serde(default)]
    #[garde(dive)]
    pub inputs: Vec<StepParam>,
    /// What the step hands back for later steps; at most ten.
    #[serde(default)]
    #[garde(dive)]
    pub outputs: Vec<StepParam>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct SaveWorkflowStepsRequest {
    /// Every step the app offers, in the order the workflow builder lists
    /// them; at most 20.
    #[garde(dive)]
    pub steps: Vec<WorkflowStepInput>,
}

impl SaveWorkflowStepsRequest {
    pub fn check(&self) -> AppResult<()> {
        self.validate()?;
        if self.steps.len() > MAX_STEPS {
            return Err(AppError::Validation(format!(
                "An app offers at most {MAX_STEPS} workflow steps"
            )));
        }
        let mut callbacks = HashSet::new();
        for step in &self.steps {
            let title = step.title.trim();
            if !callbacks.insert(step.callback_id.as_str()) {
                return Err(AppError::Validation(
                    "Each workflow step needs its own callback ID".into(),
                ));
            }
            for (params, what) in [(&step.inputs, "inputs"), (&step.outputs, "outputs")] {
                if params.len() > MAX_PARAMS {
                    return Err(AppError::Validation(format!(
                        "\u{201c}{title}\u{201d} has at most {MAX_PARAMS} {what}"
                    )));
                }
                let mut names = HashSet::new();
                if let Some(twice) = params.iter().find(|p| !names.insert(p.name.as_str())) {
                    return Err(AppError::Validation(format!(
                        "\u{201c}{title}\u{201d} has two {what} called \"{}\"",
                        twice.name
                    )));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AppWorkflowStep {
    pub id: Uuid,
    pub app_id: Uuid,
    pub callback_id: String,
    pub title: String,
    pub description: String,
    pub inputs: Vec<StepParam>,
    pub outputs: Vec<StepParam>,
}

pub struct AppWorkflowStepRow {
    pub id: Uuid,
    pub app_id: Uuid,
    pub callback_id: String,
    pub title: String,
    pub description: String,
    pub inputs: serde_json::Value,
    pub outputs: serde_json::Value,
}

impl From<AppWorkflowStepRow> for AppWorkflowStep {
    fn from(row: AppWorkflowStepRow) -> Self {
        AppWorkflowStep {
            id: row.id,
            app_id: row.app_id,
            callback_id: row.callback_id,
            title: row.title,
            description: row.description,
            inputs: serde_json::from_value(row.inputs).unwrap_or_default(),
            outputs: serde_json::from_value(row.outputs).unwrap_or_default(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AppWorkflowSteps {
    pub steps: Vec<AppWorkflowStep>,
}

pub struct ProviderRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub icon_url: Option<String>,
    pub disabled: bool,
    pub scoped: bool,
    pub listening: bool,
    pub reachable: bool,
}

impl ProviderRow {
    pub fn problem(&self) -> Option<String> {
        let name = &self.name;
        if self.disabled {
            Some(format!("{name} was turned off"))
        } else if !self.scoped {
            Some(format!(
                "{name} doesn't have the workflow.steps:execute scope"
            ))
        } else if !self.listening {
            Some(format!("{name} doesn't subscribe to function_executed"))
        } else if !self.reachable {
            Some(format!(
                "{name} has no request URL and isn't in socket mode"
            ))
        } else {
            None
        }
    }
}

/// An app that offers workflow steps.
#[derive(Debug, Serialize, ToSchema)]
pub struct StepProvider {
    pub app_id: Uuid,
    pub name: String,
    pub icon_url: Option<String>,
    /// Why workflows can't use its steps right now; none when they can.
    pub problem: Option<String>,
    pub steps: Vec<AppWorkflowStep>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CompleteStepRequest {
    /// A value for each output the step declares, by name. A required output
    /// can't be missing or null.
    #[serde(default)]
    #[schema(value_type = Object)]
    pub outputs: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FailStepRequest {
    /// Why the step failed, shown in the workflow's run history; at most 500
    /// characters.
    pub error: String,
}
