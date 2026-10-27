use std::collections::HashSet;

use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

pub const NAME_MAX_CHARS: usize = 80;
pub const DESCRIPTION_MAX_CHARS: usize = 300;
pub const TEXT_MAX_CHARS: usize = 4000;
pub const MAX_STEPS: usize = 20;
pub const MAX_WEBHOOK_VARIABLES: usize = 10;
pub const VARIABLE_NAME_MAX_CHARS: usize = 40;
pub const HISTORY_LIMIT: i64 = 50;
pub const FORM_TITLE_MAX_CHARS: usize = 80;
pub const MAX_FORM_FIELDS: usize = 10;
pub const FIELD_LABEL_MAX_CHARS: usize = 80;
pub const MAX_SELECT_OPTIONS: usize = 20;
pub const OPTION_MAX_CHARS: usize = 80;
pub const SHORT_ANSWER_MAX_CHARS: usize = 200;
pub const LONG_ANSWER_MAX_CHARS: usize = 4000;
pub const FORM_EXPIRY_DAYS: i64 = 7;
pub const MAX_CELLS: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub fn iso(self) -> i16 {
        match self {
            Self::Mon => 1,
            Self::Tue => 2,
            Self::Wed => 3,
            Self::Thu => 4,
            Self::Fri => 5,
            Self::Sat => 6,
            Self::Sun => 7,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Schedule {
        days: Vec<Weekday>,
        time: String,
        timezone: String,
    },
    Webhook {
        variables: Vec<String>,
    },
    ReactionAdded {
        channel_id: Uuid,
        emoji: String,
    },
    MemberJoined {
        channel_id: Uuid,
    },
    Shortcut {
        channel_id: Uuid,
    },
}

pub const REACTION_VARIABLES: [&str; 4] = [
    "trigger.user",
    "trigger.channel",
    "trigger.message",
    "trigger.message_link",
];
pub const JOIN_VARIABLES: [&str; 2] = ["trigger.user", "trigger.channel"];

impl Trigger {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Schedule { .. } => "schedule",
            Self::Webhook { .. } => "webhook",
            Self::ReactionAdded { .. } => "reaction_added",
            Self::MemberJoined { .. } => "member_joined",
            Self::Shortcut { .. } => "shortcut",
        }
    }

    pub fn variables(&self) -> Vec<String> {
        match self {
            Self::Schedule { .. } => Vec::new(),
            Self::Webhook { variables } => variables
                .iter()
                .map(|name| format!("webhook.{name}"))
                .collect(),
            Self::ReactionAdded { .. } => REACTION_VARIABLES.map(String::from).to_vec(),
            Self::MemberJoined { .. } | Self::Shortcut { .. } => {
                JOIN_VARIABLES.map(String::from).to_vec()
            }
        }
    }

    pub fn has_person(&self) -> bool {
        self.channel().is_some()
    }

    pub fn channel(&self) -> Option<Uuid> {
        match self {
            Self::ReactionAdded { channel_id, .. }
            | Self::MemberJoined { channel_id }
            | Self::Shortcut { channel_id } => Some(*channel_id),
            Self::Schedule { .. } | Self::Webhook { .. } => None,
        }
    }

    pub fn schedule(&self) -> Option<Schedule<'_>> {
        match self {
            Self::Schedule {
                days,
                time,
                timezone,
            } => Some(Schedule {
                days: days.iter().map(|day| day.iso()).collect(),
                time: parse_time(time)?,
                timezone,
            }),
            _ => None,
        }
    }
}

pub struct Schedule<'a> {
    pub days: Vec<i16>,
    pub time: NaiveTime,
    pub timezone: &'a str,
}

fn parse_time(time: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(time, "%H:%M").ok()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Recipient {
    Person { user_id: Uuid },
    TriggerUser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    ShortText,
    LongText,
    Select,
    Person,
    Date,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FormField {
    pub name: String,
    pub label: String,
    pub kind: FieldKind,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CellValue {
    pub column_id: Uuid,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariableKind<'a> {
    Text,
    Person,
    Date,
    Choice(&'a [String]),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Step {
    SendMessage {
        channel_id: Uuid,
        text: String,
        #[serde(default)]
        summary: bool,
    },
    SendDm {
        to: Recipient,
        text: String,
        #[serde(default)]
        summary: bool,
    },
    Form {
        to: Recipient,
        title: String,
        fields: Vec<FormField>,
    },
    AddListItem {
        list_id: Uuid,
        cells: Vec<CellValue>,
    },
    UpdateListItem {
        from_step: usize,
        cells: Vec<CellValue>,
    },
}

impl Step {
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::SendMessage { text, .. } | Self::SendDm { text, .. } => Some(text),
            _ => None,
        }
    }

    pub fn cells(&self) -> &[CellValue] {
        match self {
            Self::AddListItem { cells, .. } | Self::UpdateListItem { cells, .. } => cells,
            _ => &[],
        }
    }

    pub fn summary(&self) -> bool {
        match self {
            Self::SendMessage { summary, .. } | Self::SendDm { summary, .. } => *summary,
            _ => false,
        }
    }

    pub fn recipient(&self) -> Option<&Recipient> {
        match self {
            Self::SendDm { to, .. } | Self::Form { to, .. } => Some(to),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::SendMessage { .. } => "Send a message",
            Self::SendDm { .. } => "Send a DM",
            Self::Form { .. } => "Collect a form",
            Self::AddListItem { .. } => "Add a list item",
            Self::UpdateListItem { .. } => "Update the list item",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Definition {
    pub trigger: Trigger,
    pub steps: Vec<Step>,
}

impl Definition {
    pub fn fields_before(&self, position: usize) -> impl Iterator<Item = &FormField> {
        self.steps[..position.min(self.steps.len())]
            .iter()
            .flat_map(|step| match step {
                Step::Form { fields, .. } => fields.as_slice(),
                _ => &[],
            })
    }

    pub fn variable_kind(&self, position: usize, name: &str) -> VariableKind<'_> {
        if name == "trigger.user" {
            return VariableKind::Person;
        }
        let Some(field) = name
            .strip_prefix("form.")
            .and_then(|field| self.fields_before(position).find(|f| f.name == field))
        else {
            return VariableKind::Text;
        };
        match field.kind {
            FieldKind::Person => VariableKind::Person,
            FieldKind::Date => VariableKind::Date,
            FieldKind::Select => VariableKind::Choice(&field.options),
            FieldKind::ShortText | FieldKind::LongText => VariableKind::Text,
        }
    }

    pub fn list_of(&self, position: usize) -> Option<Uuid> {
        match self.steps.get(position)? {
            Step::AddListItem { list_id, .. } => Some(*list_id),
            Step::UpdateListItem { from_step, .. } if *from_step < position => {
                self.list_of(*from_step)
            }
            _ => None,
        }
    }

    pub fn check(&self) -> AppResult<()> {
        match &self.trigger {
            Trigger::Schedule {
                days,
                time,
                timezone,
            } => {
                if days.is_empty() {
                    return invalid("Pick at least one day for the schedule");
                }
                if parse_time(time).is_none() {
                    return invalid("The time has to look like 09:30");
                }
                if timezone.trim().is_empty() {
                    return invalid("Pick a timezone for the schedule");
                }
            }
            Trigger::Webhook { variables } => {
                if variables.len() > MAX_WEBHOOK_VARIABLES {
                    return invalid(&format!(
                        "A webhook takes at most {MAX_WEBHOOK_VARIABLES} variables"
                    ));
                }
                let mut seen = HashSet::new();
                for name in variables {
                    if !is_variable_name(name) {
                        return invalid(&format!(
                            "\"{name}\" can't be a variable name: use lowercase letters, digits and _"
                        ));
                    }
                    if !seen.insert(name) {
                        return invalid(&format!("\"{name}\" is declared twice"));
                    }
                }
            }
            Trigger::ReactionAdded { emoji, .. } => {
                if emoji.trim().is_empty() {
                    return invalid("Pick the emoji to listen for");
                }
                shared_common::validation::validate_reaction_emoji(emoji)?;
            }
            Trigger::MemberJoined { .. } | Trigger::Shortcut { .. } => {}
        }

        if self.steps.is_empty() {
            return invalid("Add at least one step");
        }
        if self.steps.len() > MAX_STEPS {
            return invalid(&format!("A workflow has at most {MAX_STEPS} steps"));
        }

        let mut available = self.trigger.variables();
        let mut asked = HashSet::new();
        for (index, step) in self.steps.iter().enumerate() {
            let at = format!("Step {}", index + 1);
            if let Some(text) = step.text() {
                if text.trim().is_empty() {
                    return invalid(&format!("{at} needs some text"));
                }
                if text.chars().count() > TEXT_MAX_CHARS {
                    return invalid(&format!(
                        "{at}: the text can be at most {TEXT_MAX_CHARS} characters"
                    ));
                }
                for name in placeholders(text) {
                    if !available.iter().any(|known| known == name) {
                        return invalid(&format!(
                            "{at}: {{{{{name}}}}} isn't available for this trigger"
                        ));
                    }
                }
            }
            if let Step::UpdateListItem { from_step, .. } = step {
                if *from_step >= index
                    || !matches!(self.steps[*from_step], Step::AddListItem { .. })
                {
                    return invalid(&format!("{at}: pick an earlier step that adds a list item"));
                }
            }
            if matches!(step, Step::AddListItem { .. } | Step::UpdateListItem { .. }) {
                check_cells(&at, step.cells(), &available)?;
            }
            if step.summary() && asked.is_empty() {
                return invalid(&format!("{at}: there is no form before it to sum up"));
            }
            if step.recipient() == Some(&Recipient::TriggerUser) && !self.trigger.has_person() {
                return invalid(&format!("{at}: this trigger has no person to send it to"));
            }
            if let Step::Form { title, fields, .. } = step {
                check_form(&at, title, fields, &mut asked)?;
                available.extend(fields.iter().map(|field| format!("form.{}", field.name)));
            }
        }
        Ok(())
    }
}

fn check_cells(at: &str, cells: &[CellValue], available: &[String]) -> AppResult<()> {
    if cells.is_empty() {
        return invalid(&format!("{at}: fill in at least one column"));
    }
    if cells.len() > MAX_CELLS {
        return invalid(&format!("{at}: it fills at most {MAX_CELLS} columns"));
    }
    let mut seen = HashSet::new();
    for cell in cells {
        if !seen.insert(cell.column_id) {
            return invalid(&format!("{at}: a column is filled twice"));
        }
        if cell.value.trim().is_empty() {
            return invalid(&format!("{at}: every column it fills needs a value"));
        }
        if cell.value.chars().count() > TEXT_MAX_CHARS {
            return invalid(&format!(
                "{at}: a value can be at most {TEXT_MAX_CHARS} characters"
            ));
        }
        for name in placeholders(&cell.value) {
            if !available.iter().any(|known| known == name) {
                return invalid(&format!(
                    "{at}: {{{{{name}}}}} isn't available for this trigger"
                ));
            }
        }
    }
    Ok(())
}

fn check_form<'a>(
    at: &str,
    title: &str,
    fields: &'a [FormField],
    asked: &mut HashSet<&'a str>,
) -> AppResult<()> {
    if title.trim().is_empty() {
        return invalid(&format!("{at}: give the form a title"));
    }
    if title.chars().count() > FORM_TITLE_MAX_CHARS {
        return invalid(&format!(
            "{at}: a form title can be at most {FORM_TITLE_MAX_CHARS} characters"
        ));
    }
    if fields.is_empty() || fields.len() > MAX_FORM_FIELDS {
        return invalid(&format!(
            "{at}: a form has 1 to {MAX_FORM_FIELDS} questions"
        ));
    }
    for field in fields {
        let label = field.label.trim();
        if label.is_empty() {
            return invalid(&format!("{at}: every question needs a label"));
        }
        if label.chars().count() > FIELD_LABEL_MAX_CHARS {
            return invalid(&format!(
                "{at}: a question can be at most {FIELD_LABEL_MAX_CHARS} characters"
            ));
        }
        if !is_variable_name(&field.name) {
            return invalid(&format!(
                "{at}: \"{}\" can't be a variable name: use lowercase letters, digits and _",
                field.name
            ));
        }
        if !asked.insert(&field.name) {
            return invalid(&format!("{at}: \"{}\" is asked twice", field.name));
        }
        if field.kind == FieldKind::Select {
            let options: HashSet<&str> = field.options.iter().map(|o| o.trim()).collect();
            if field.options.is_empty() || field.options.len() > MAX_SELECT_OPTIONS {
                return invalid(&format!(
                    "{at}: \u{201c}{label}\u{201d} needs 1 to {MAX_SELECT_OPTIONS} choices"
                ));
            }
            if options.len() != field.options.len()
                || options
                    .iter()
                    .any(|o| o.is_empty() || o.chars().count() > OPTION_MAX_CHARS)
            {
                return invalid(&format!(
                    "{at}: \u{201c}{label}\u{201d} has a blank, repeated or too long choice"
                ));
            }
        }
    }
    Ok(())
}

fn invalid(message: &str) -> AppResult<()> {
    Err(AppError::Validation(message.into()))
}

fn is_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    name.chars().count() <= VARIABLE_NAME_MAX_CHARS
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub fn placeholders(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        found.push(after[..end].trim());
        rest = &after[end + 2..];
    }
    found
}

pub fn render(text: &str, value: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        out.push_str(&rest[..start]);
        out.push_str(&value(after[..end].trim()));
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveWorkflowRequest {
    pub name: String,
    pub description: Option<String>,
    pub definition: Definition,
}

pub struct CheckedWorkflow {
    pub name: String,
    pub description: Option<String>,
    pub definition: serde_json::Value,
}

impl SaveWorkflowRequest {
    pub fn checked(&self) -> AppResult<CheckedWorkflow> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(AppError::Validation("Give the workflow a name".into()));
        }
        if name.chars().count() > NAME_MAX_CHARS {
            return Err(AppError::Validation(format!(
                "A name can be at most {NAME_MAX_CHARS} characters"
            )));
        }
        let description = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty());
        if description.is_some_and(|d| d.chars().count() > DESCRIPTION_MAX_CHARS) {
            return Err(AppError::Validation(format!(
                "A description can be at most {DESCRIPTION_MAX_CHARS} characters"
            )));
        }
        self.definition.check()?;
        Ok(CheckedWorkflow {
            name: name.to_string(),
            description: description.map(str::to_string),
            definition: serde_json::to_value(&self.definition)
                .map_err(|e| AppError::Internal(e.to_string()))?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "workflow_run_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Waiting,
    Expired,
}

#[derive(Debug, Clone)]
pub struct WorkflowRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub definition: serde_json::Value,
    pub version: i32,
    pub enabled: bool,
    pub webhook_token: Option<String>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub creator_gone: bool,
    pub last_status: Option<RunStatus>,
    pub last_started_at: Option<DateTime<Utc>>,
}

impl WorkflowRow {
    pub fn definition(&self) -> Option<Definition> {
        serde_json::from_value(self.definition.clone()).ok()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChannelWorkflow {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub created_by: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LastRun {
    pub status: RunStatus,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Workflow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub definition: Definition,
    pub version: i32,
    pub enabled: bool,
    pub creator_gone: bool,
    pub webhook_url: Option<String>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run: Option<LastRun>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RunRow {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub workflow_version: i32,
    pub status: RunStatus,
    pub context: serde_json::Value,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunStep {
    pub position: i32,
    pub succeeded: bool,
    pub message_id: Option<Uuid>,
    pub list_item_id: Option<Uuid>,
    pub error: Option<String>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WorkflowRun {
    pub id: Uuid,
    pub workflow_version: i32,
    pub status: RunStatus,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub steps: Vec<RunStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FormRequest {
    pub run_id: Uuid,
    pub position: i32,
    pub title: String,
    pub recipient_id: Uuid,
    pub fields: Vec<FormField>,
    pub expires_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub expired: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitFormRequest {
    pub answers: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Shortcut {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RunAccepted {
    pub status: &'static str,
    pub run_id: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn definition(value: serde_json::Value) -> Definition {
        serde_json::from_value(value).expect("a definition")
    }

    fn error(value: serde_json::Value) -> String {
        match definition(value).check() {
            Err(AppError::Validation(message)) => message,
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    fn post(text: &str) -> serde_json::Value {
        json!({ "type": "send_message", "channel_id": Uuid::nil(), "text": text })
    }

    fn schedule() -> serde_json::Value {
        json!({ "type": "schedule", "days": ["mon", "fri"], "time": "09:50", "timezone": "Europe/Belgrade" })
    }

    #[test]
    fn a_schedule_and_a_webhook_with_their_steps_are_valid() {
        definition(json!({ "trigger": schedule(), "steps": [post("standup in 10")] }))
            .check()
            .unwrap();
        definition(json!({
            "trigger": { "type": "webhook", "variables": ["service", "status_code"] },
            "steps": [
                post("{{webhook.service}} is down ({{ webhook.status_code }})"),
                { "type": "send_dm", "to": { "kind": "person", "user_id": Uuid::nil() }, "text": "look" },
            ],
        }))
        .check()
        .unwrap();
    }

    #[test]
    fn a_schedule_needs_days_a_time_and_a_timezone() {
        let mut no_days = schedule();
        no_days["days"] = json!([]);
        assert_eq!(
            error(json!({ "trigger": no_days, "steps": [post("x")] })),
            "Pick at least one day for the schedule"
        );
        for time in ["9.50", "25:00", "noon", ""] {
            let mut bad = schedule();
            bad["time"] = json!(time);
            assert_eq!(
                error(json!({ "trigger": bad, "steps": [post("x")] })),
                "The time has to look like 09:30",
                "{time}"
            );
        }
        let mut no_zone = schedule();
        no_zone["timezone"] = json!(" ");
        assert_eq!(
            error(json!({ "trigger": no_zone, "steps": [post("x")] })),
            "Pick a timezone for the schedule"
        );
    }

    #[test]
    fn webhook_variables_are_short_lowercase_and_unique() {
        let trigger = |names: serde_json::Value| json!({ "trigger": { "type": "webhook", "variables": names }, "steps": [post("x")] });
        assert!(error(trigger(json!(["Service"]))).contains("can't be a variable name"));
        assert!(error(trigger(json!(["1st"]))).contains("can't be a variable name"));
        assert!(error(trigger(json!(["a-b"]))).contains("can't be a variable name"));
        assert!(error(trigger(json!(["x".repeat(41)]))).contains("can't be a variable name"));
        assert_eq!(error(trigger(json!(["a", "a"]))), "\"a\" is declared twice");
        assert_eq!(
            error(trigger(json!((0..11)
                .map(|i| format!("v{i}"))
                .collect::<Vec<_>>()))),
            "A webhook takes at most 10 variables"
        );
    }

    #[test]
    fn steps_need_text_and_only_the_variables_their_trigger_has() {
        assert_eq!(
            error(json!({ "trigger": schedule(), "steps": [] })),
            "Add at least one step"
        );
        assert_eq!(
            error(
                json!({ "trigger": schedule(), "steps": (0..21).map(|_| post("x")).collect::<Vec<_>>() })
            ),
            "A workflow has at most 20 steps"
        );
        assert_eq!(
            error(json!({ "trigger": schedule(), "steps": [post("ok"), post("  ")] })),
            "Step 2 needs some text"
        );
        assert_eq!(
            error(json!({ "trigger": schedule(), "steps": [post(&"x".repeat(4001))] })),
            "Step 1: the text can be at most 4000 characters"
        );
        assert_eq!(
            error(json!({ "trigger": schedule(), "steps": [post("hi {{webhook.name}}")] })),
            "Step 1: {{webhook.name}} isn't available for this trigger"
        );
        assert_eq!(
            error(json!({
                "trigger": { "type": "webhook", "variables": ["name"] },
                "steps": [post("{{trigger.user}} joined")],
            })),
            "Step 1: {{trigger.user}} isn't available for this trigger"
        );
        assert_eq!(
            error(json!({
                "trigger": schedule(),
                "steps": [{ "type": "send_dm", "to": { "kind": "trigger_user" }, "text": "hi" }],
            })),
            "Step 1: this trigger has no person to send it to"
        );
    }

    #[test]
    fn event_triggers_offer_the_person_and_the_channel_that_set_them_off() {
        let reaction =
            json!({ "type": "reaction_added", "channel_id": Uuid::nil(), "emoji": "\u{1F6A8}" });
        definition(json!({
            "trigger": reaction.clone(),
            "steps": [
                post("{{trigger.user}} flagged {{trigger.message_link}} in {{trigger.channel}}: {{trigger.message}}"),
                { "type": "send_dm", "to": { "kind": "trigger_user" }, "text": "thanks" },
            ],
        }))
        .check()
        .unwrap();

        let joined = json!({ "type": "member_joined", "channel_id": Uuid::nil() });
        definition(json!({
            "trigger": joined.clone(),
            "steps": [{ "type": "send_dm", "to": { "kind": "trigger_user" }, "text": "Welcome {{trigger.user}} to {{trigger.channel}}" }],
        }))
        .check()
        .unwrap();
        assert_eq!(
            error(json!({ "trigger": joined, "steps": [post("{{trigger.message}}")] })),
            "Step 1: {{trigger.message}} isn't available for this trigger"
        );

        let mut no_emoji = reaction.clone();
        no_emoji["emoji"] = json!(" ");
        assert_eq!(
            error(json!({ "trigger": no_emoji, "steps": [post("x")] })),
            "Pick the emoji to listen for"
        );
        let mut long = reaction;
        long["emoji"] = json!("x".repeat(9));
        assert_eq!(
            error(json!({ "trigger": long, "steps": [post("x")] })),
            "Reaction must be at most 8 characters"
        );
    }

    fn form(fields: serde_json::Value) -> serde_json::Value {
        json!({ "type": "form", "to": { "kind": "trigger_user" }, "title": "Time off", "fields": fields })
    }

    fn field(name: &str, kind: &str) -> serde_json::Value {
        json!({ "name": name, "label": name, "kind": kind })
    }

    fn shortcut() -> serde_json::Value {
        json!({ "type": "shortcut", "channel_id": Uuid::nil() })
    }

    #[test]
    fn a_shortcut_form_offers_its_answers_to_later_steps() {
        definition(json!({
            "trigger": shortcut(),
            "steps": [
                form(json!([
                    field("reason", "long_text"),
                    { "name": "kind", "label": "Kind", "kind": "select", "options": ["Sick", "Holiday"], "required": true },
                    field("cover", "person"),
                    field("from", "date"),
                ])),
                { "type": "send_message", "channel_id": Uuid::nil(), "text": "{{trigger.user}} is off from {{form.from}}, {{form.cover}} covers", "summary": true },
            ],
        }))
        .check()
        .unwrap();
        assert_eq!(
            error(json!({
                "trigger": shortcut(),
                "steps": [post("{{form.reason}}"), form(json!([field("reason", "short_text")]))],
            })),
            "Step 1: {{form.reason}} isn't available for this trigger"
        );
        assert_eq!(
            error(json!({
                "trigger": shortcut(),
                "steps": [{ "type": "send_message", "channel_id": Uuid::nil(), "text": "x", "summary": true }],
            })),
            "Step 1: there is no form before it to sum up"
        );
        assert_eq!(
            error(
                json!({ "trigger": schedule(), "steps": [form(json!([field("a", "short_text")]))] })
            ),
            "Step 1: this trigger has no person to send it to"
        );
    }

    #[test]
    fn a_form_needs_a_title_and_well_formed_questions() {
        let check =
            |step: serde_json::Value| error(json!({ "trigger": shortcut(), "steps": [step] }));
        let mut untitled = form(json!([field("a", "short_text")]));
        untitled["title"] = json!(" ");
        assert_eq!(check(untitled), "Step 1: give the form a title");
        assert_eq!(
            check(form(json!([]))),
            "Step 1: a form has 1 to 10 questions"
        );
        assert_eq!(
            check(form(json!((0..11)
                .map(|i| field(&format!("q{i}"), "short_text"))
                .collect::<Vec<_>>()))),
            "Step 1: a form has 1 to 10 questions"
        );
        assert_eq!(
            check(form(json!([{ "name": "a", "label": " ", "kind": "date" }]))),
            "Step 1: every question needs a label"
        );
        assert!(check(form(json!([field("A b", "date")]))).contains("can't be a variable name"));
        assert_eq!(
            check(form(json!([field("a", "date"), field("a", "person")]))),
            "Step 1: \"a\" is asked twice"
        );
        assert_eq!(
            check(form(json!([field("a", "select")]))),
            "Step 1: \u{201c}a\u{201d} needs 1 to 20 choices"
        );
        assert_eq!(
            check(form(
                json!([{ "name": "a", "label": "a", "kind": "select", "options": ["x", " x "] }])
            )),
            "Step 1: \u{201c}a\u{201d} has a blank, repeated or too long choice"
        );
        assert_eq!(
            error(json!({
                "trigger": shortcut(),
                "steps": [form(json!([field("a", "date")])), form(json!([field("a", "date")]))],
            })),
            "Step 2: \"a\" is asked twice"
        );
    }

    fn add_item(cells: serde_json::Value) -> serde_json::Value {
        json!({ "type": "add_list_item", "list_id": Uuid::nil(), "cells": cells })
    }

    fn cell(value: &str) -> serde_json::Value {
        json!({ "column_id": Uuid::new_v4(), "value": value })
    }

    #[test]
    fn list_steps_fill_columns_and_update_the_item_an_earlier_step_added() {
        let update = |from: usize| json!({ "type": "update_list_item", "from_step": from, "cells": [cell("Done")] });
        let definition = definition(json!({
            "trigger": shortcut(),
            "steps": [
                form(json!([field("reason", "short_text"), field("from", "date")])),
                add_item(json!([cell("{{form.reason}}"), cell("{{trigger.user}}")])),
                update(1),
            ],
        }));
        definition.check().unwrap();
        assert_eq!(definition.list_of(2), Some(Uuid::nil()));
        assert_eq!(
            definition.variable_kind(1, "trigger.user"),
            VariableKind::Person
        );
        assert_eq!(definition.variable_kind(1, "form.from"), VariableKind::Date);
        assert_eq!(definition.variable_kind(0, "form.from"), VariableKind::Text);

        let at = |steps: serde_json::Value| error(json!({ "trigger": shortcut(), "steps": steps }));
        assert_eq!(
            at(json!([update(0)])),
            "Step 1: pick an earlier step that adds a list item"
        );
        assert_eq!(
            at(json!([post("x"), update(0)])),
            "Step 2: pick an earlier step that adds a list item"
        );
        assert_eq!(
            at(json!([add_item(json!([]))])),
            "Step 1: fill in at least one column"
        );
        let twice = cell("a");
        assert_eq!(
            at(json!([add_item(json!([twice.clone(), twice]))])),
            "Step 1: a column is filled twice"
        );
        assert_eq!(
            at(json!([add_item(json!([cell(" ")]))])),
            "Step 1: every column it fills needs a value"
        );
        assert_eq!(
            at(json!([add_item(json!([cell("{{form.reason}}")]))])),
            "Step 1: {{form.reason}} isn't available for this trigger"
        );
    }

    #[test]
    fn an_unknown_trigger_or_step_does_not_deserialize() {
        assert!(serde_json::from_value::<Definition>(
            json!({ "trigger": { "type": "cron" }, "steps": [] })
        )
        .is_err());
        assert!(serde_json::from_value::<Definition>(
            json!({ "trigger": schedule(), "steps": [{ "type": "send_email", "text": "x" }] })
        )
        .is_err());
        assert!(serde_json::from_value::<Definition>(
            json!({ "trigger": { "type": "schedule", "days": ["someday"], "time": "09:00", "timezone": "UTC" }, "steps": [] })
        )
        .is_err());
    }

    #[test]
    fn variables_render_in_place_and_tolerate_spacing() {
        let value = |name: &str| match name {
            "webhook.service" => "api".to_string(),
            _ => String::new(),
        };
        assert_eq!(
            render(
                "{{webhook.service}} is {{ webhook.service }}, {{gone}}!",
                value
            ),
            "api is api, !"
        );
        assert_eq!(render("no vars {{ unclosed", value), "no vars {{ unclosed");
        assert_eq!(placeholders("{{ a }} and {{b}} and {{c"), vec!["a", "b"]);
    }
}
