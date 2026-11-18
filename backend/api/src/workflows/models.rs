use std::collections::HashSet;

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
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
pub const MAX_BRANCH_DEPTH: usize = 2;
pub const MAX_DELAY_DAYS: i64 = 30;
pub const MAX_TEST_VALUES: usize = 20;
pub const TEST_VALUE_MAX_CHARS: usize = 200;
pub const MAX_INVITEES: usize = 10;
pub const CHANNEL_NAME_MAX_CHARS: usize = 80;
pub const TOPIC_MAX_CHARS: usize = 500;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DelayUnit {
    Minutes,
    Hours,
    Days,
}

impl DelayUnit {
    fn minutes(self) -> i64 {
        match self {
            Self::Minutes => 1,
            Self::Hours => 60,
            Self::Days => 24 * 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Wait {
    For {
        amount: i64,
        unit: DelayUnit,
    },
    Until {
        date: String,
        time: String,
        timezone: String,
    },
}

impl Wait {
    pub fn minutes(&self) -> Option<i64> {
        match self {
            Self::For { amount, unit } => amount.checked_mul(unit.minutes()),
            Self::Until { .. } => None,
        }
    }

    pub fn moment(&self) -> Option<(NaiveDate, NaiveTime, &str)> {
        match self {
            Self::Until {
                date,
                time,
                timezone,
            } => Some((parse_date(date)?, parse_time(time)?, timezone.as_str())),
            Self::For { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Test {
    Equals { value: String },
    Contains { value: String },
    IsEmpty,
    GreaterThan { value: String },
    OneOf { values: Vec<String> },
}

fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

fn number(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', "")
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
}

fn parse_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok()
}

impl Test {
    pub fn holds(&self, subject: &str) -> bool {
        match self {
            Self::Equals { value } => same(subject, value),
            Self::Contains { value } => subject
                .to_lowercase()
                .contains(&value.trim().to_lowercase()),
            Self::IsEmpty => subject.trim().is_empty(),
            Self::GreaterThan { value } => match (number(subject), number(value)) {
                (Some(a), Some(b)) => a > b,
                _ => matches!(
                    (parse_date(subject), parse_date(value)),
                    (Some(a), Some(b)) if a > b
                ),
            },
            Self::OneOf { values } => values.iter().any(|value| same(subject, value)),
        }
    }

    pub fn operands(&self) -> &[String] {
        match self {
            Self::Equals { value } | Self::Contains { value } | Self::GreaterThan { value } => {
                std::slice::from_ref(value)
            }
            Self::OneOf { values } => values,
            Self::IsEmpty => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChannelRef {
    Channel { channel_id: Uuid },
    TriggerChannel,
    FromStep { step: usize },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Invitee {
    TriggerUser,
    Answer { field: String },
    Group { group_id: Uuid },
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
    Branch {
        value: String,
        test: Test,
        #[schema(no_recursion)]
        then: Vec<Step>,
        #[schema(no_recursion)]
        otherwise: Vec<Step>,
    },
    Delay {
        wait: Wait,
    },
    AddReaction {
        emoji: String,
    },
    ReplyInThread {
        text: String,
    },
    CreateChannel {
        name: String,
        #[serde(default)]
        private: bool,
    },
    AddToChannel {
        channel: ChannelRef,
        people: Vec<Invitee>,
    },
    SetTopic {
        channel: ChannelRef,
        topic: String,
    },
    ArchiveChannel {
        channel: ChannelRef,
    },
}

impl Step {
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::SendMessage { text, .. }
            | Self::SendDm { text, .. }
            | Self::ReplyInThread { text } => Some(text),
            _ => None,
        }
    }

    pub fn cells(&self) -> &[CellValue] {
        match self {
            Self::AddListItem { cells, .. } | Self::UpdateListItem { cells, .. } => cells,
            _ => &[],
        }
    }

    pub fn fields(&self) -> &[FormField] {
        match self {
            Self::Form { fields, .. } => fields,
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

    pub fn channel(&self) -> Option<&ChannelRef> {
        match self {
            Self::AddToChannel { channel, .. }
            | Self::SetTopic { channel, .. }
            | Self::ArchiveChannel { channel } => Some(channel),
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
            Self::Branch { .. } => "Branch",
            Self::Delay { .. } => "Wait",
            Self::AddReaction { .. } => "Add a reaction",
            Self::ReplyInThread { .. } => "Reply in the thread",
            Self::CreateChannel { .. } => "Create a channel",
            Self::AddToChannel { .. } => "Add people to a channel",
            Self::SetTopic { .. } => "Set a channel topic",
            Self::ArchiveChannel { .. } => "Archive a channel",
        }
    }

    pub fn outputs(&self, position: usize) -> Vec<String> {
        let number = position + 1;
        match self {
            Self::Form { fields, .. } => fields
                .iter()
                .map(|field| format!("form.{}", field.name))
                .collect(),
            Self::CreateChannel { .. } => vec![
                format!("step.{number}.channel"),
                format!("step.{number}.channel_link"),
            ],
            Self::ReplyInThread { .. } => vec![format!("step.{number}.message_link")],
            _ => Vec::new(),
        }
    }
}

pub struct Node<'a> {
    pub step: &'a Step,
    pub depth: usize,
    pub path: Vec<(usize, bool)>,
    pub before: Vec<usize>,
}

impl Node<'_> {
    pub fn reached(&self, taken: impl Fn(usize) -> Option<bool>) -> bool {
        self.path
            .iter()
            .all(|(branch, lane)| taken(*branch) == Some(*lane))
    }
}

fn flatten<'a>(
    steps: &'a [Step],
    depth: usize,
    path: &[(usize, bool)],
    inherited: &[usize],
    nodes: &mut Vec<Node<'a>>,
) {
    let mut before = inherited.to_vec();
    for step in steps {
        let index = nodes.len();
        nodes.push(Node {
            step,
            depth,
            path: path.to_vec(),
            before: before.clone(),
        });
        before.push(index);
        if let Step::Branch {
            then, otherwise, ..
        } = step
        {
            for (lane, steps) in [(true, then), (false, otherwise)] {
                let mut inner = path.to_vec();
                inner.push((index, lane));
                flatten(steps, depth + 1, &inner, &before, nodes);
            }
        }
    }
}

pub enum Source<'a> {
    Variable(&'a str),
    Profile(Uuid),
    Column { step: usize, column_id: Uuid },
}

pub fn source(value: &str) -> Option<Source<'_>> {
    let value = value.trim();
    if let Some(id) = value.strip_prefix("profile.") {
        return id.parse().ok().map(Source::Profile);
    }
    if let Some((number, column)) = value
        .strip_prefix("step.")
        .and_then(|rest| rest.split_once(".column."))
    {
        return Some(Source::Column {
            step: number.parse::<usize>().ok()?.checked_sub(1)?,
            column_id: column.parse().ok()?,
        });
    }
    (!value.is_empty()).then_some(Source::Variable(value))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Definition {
    pub trigger: Trigger,
    pub steps: Vec<Step>,
}

impl Definition {
    pub fn flat(&self) -> Vec<Node<'_>> {
        let mut nodes = Vec::new();
        flatten(&self.steps, 0, &[], &[], &mut nodes);
        nodes
    }

    pub fn fields(&self) -> Vec<&FormField> {
        self.flat()
            .iter()
            .flat_map(|node| node.step.fields())
            .collect()
    }

    pub fn fields_before(&self, position: usize) -> Vec<&FormField> {
        let flat = self.flat();
        let Some(node) = flat.get(position) else {
            return Vec::new();
        };
        node.before
            .iter()
            .flat_map(|&at| flat[at].step.fields())
            .collect()
    }

    pub fn variables_at(&self, flat: &[Node<'_>], position: usize) -> Vec<String> {
        let mut available = self.trigger.variables();
        if let Some(node) = flat.get(position) {
            for &at in &node.before {
                available.extend(flat[at].step.outputs(at));
            }
        }
        available
    }

    pub fn variable_kind(&self, position: usize, name: &str) -> VariableKind<'_> {
        if name == "trigger.user" {
            return VariableKind::Person;
        }
        let Some(field) = name.strip_prefix("form.").and_then(|field| {
            self.fields_before(position)
                .into_iter()
                .find(|f| f.name == field)
        }) else {
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
        list_in(&self.flat(), position)
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
        let flat = self.flat();
        if flat.len() > MAX_STEPS {
            return invalid(&format!("A workflow has at most {MAX_STEPS} steps"));
        }

        let mut asked = HashSet::new();
        for (index, node) in flat.iter().enumerate() {
            let step = node.step;
            let at = format!("Step {}", index + 1);
            let available = self.variables_at(&flat, index);
            if let Some(text) = step.text() {
                if text.trim().is_empty() {
                    return invalid(&format!("{at} needs some text"));
                }
                if text.chars().count() > TEXT_MAX_CHARS {
                    return invalid(&format!(
                        "{at}: the text can be at most {TEXT_MAX_CHARS} characters"
                    ));
                }
                check_placeholders(&at, text, &available)?;
            }
            if let Step::UpdateListItem { from_step, .. } = step {
                if !matches!(
                    earlier(&flat, node, *from_step),
                    Some(Step::AddListItem { .. })
                ) {
                    return invalid(&format!("{at}: pick an earlier step that adds a list item"));
                }
            }
            if matches!(step, Step::AddListItem { .. } | Step::UpdateListItem { .. }) {
                check_cells(&at, step.cells(), &available)?;
            }
            if step.summary()
                && !node
                    .before
                    .iter()
                    .any(|&b| matches!(flat[b].step, Step::Form { .. }))
            {
                return invalid(&format!("{at}: there is no form before it to sum up"));
            }
            if step.recipient() == Some(&Recipient::TriggerUser) && !self.trigger.has_person() {
                return invalid(&format!("{at}: this trigger has no person to send it to"));
            }
            if let Some(channel) = step.channel() {
                self.check_channel(&at, channel, &flat, node)?;
            }
            match step {
                Step::Form { title, fields, .. } => check_form(&at, title, fields, &mut asked)?,
                Step::Branch {
                    value,
                    test,
                    then,
                    otherwise,
                } => {
                    if node.depth >= MAX_BRANCH_DEPTH {
                        return invalid(&format!(
                            "{at}: branches nest at most {MAX_BRANCH_DEPTH} deep"
                        ));
                    }
                    if then.is_empty() && otherwise.is_empty() {
                        return invalid(&format!("{at}: add a step to one of its branches"));
                    }
                    let kind = match source(value) {
                        Some(Source::Variable(name))
                            if available.iter().any(|known| known == name) =>
                        {
                            self.variable_kind(index, name)
                        }
                        Some(Source::Profile(_)) if self.trigger.has_person() => VariableKind::Text,
                        Some(Source::Column { step, .. })
                            if matches!(
                                earlier(&flat, node, step),
                                Some(Step::AddListItem { .. } | Step::UpdateListItem { .. })
                            ) =>
                        {
                            VariableKind::Text
                        }
                        _ => return invalid(&format!("{at}: pick what the branch tests")),
                    };
                    check_test(&at, test, kind)?;
                }
                Step::Delay { wait } => check_wait(&at, wait)?,
                Step::AddReaction { emoji } => {
                    if !matches!(self.trigger, Trigger::ReactionAdded { .. }) {
                        return invalid(&format!("{at}: this trigger has no message to react to"));
                    }
                    if emoji.trim().is_empty() {
                        return invalid(&format!("{at}: pick an emoji"));
                    }
                    shared_common::validation::validate_reaction_emoji(emoji)
                        .map_err(|e| prefixed(&at, e))?;
                }
                Step::ReplyInThread { .. } => {
                    if !matches!(self.trigger, Trigger::ReactionAdded { .. }) {
                        return invalid(&format!("{at}: this trigger has no message to reply to"));
                    }
                }
                Step::CreateChannel { name, .. } => {
                    if name.trim().is_empty() {
                        return invalid(&format!("{at}: give the channel a name"));
                    }
                    if name.chars().count() > CHANNEL_NAME_MAX_CHARS {
                        return invalid(&format!(
                            "{at}: a channel name can be at most {CHANNEL_NAME_MAX_CHARS} characters"
                        ));
                    }
                    check_placeholders(&at, name, &available)?;
                }
                Step::AddToChannel { people, .. } => {
                    if people.is_empty() {
                        return invalid(&format!("{at}: pick who to add"));
                    }
                    if people.len() > MAX_INVITEES {
                        return invalid(&format!(
                            "{at}: it adds at most {MAX_INVITEES} people or groups"
                        ));
                    }
                    for invitee in people {
                        match invitee {
                            Invitee::TriggerUser if !self.trigger.has_person() => {
                                return invalid(&format!(
                                    "{at}: this trigger has no person to add"
                                ));
                            }
                            Invitee::Answer { field } => {
                                let asked_for =
                                    node.before.iter().any(|&b| {
                                        flat[b].step.fields().iter().any(|f| {
                                            &f.name == field && f.kind == FieldKind::Person
                                        })
                                    });
                                if !asked_for {
                                    return invalid(&format!(
                                        "{at}: pick a person question from an earlier form"
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Step::SetTopic { topic, .. } => {
                    if topic.trim().is_empty() {
                        return invalid(&format!("{at}: write the topic"));
                    }
                    if topic.chars().count() > TOPIC_MAX_CHARS {
                        return invalid(&format!(
                            "{at}: a topic can be at most {TOPIC_MAX_CHARS} characters"
                        ));
                    }
                    check_placeholders(&at, topic, &available)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn check_channel(
        &self,
        at: &str,
        channel: &ChannelRef,
        flat: &[Node<'_>],
        node: &Node<'_>,
    ) -> AppResult<()> {
        match channel {
            ChannelRef::TriggerChannel if self.trigger.channel().is_none() => {
                invalid(&format!("{at}: this trigger has no channel"))
            }
            ChannelRef::FromStep { step }
                if !matches!(earlier(flat, node, *step), Some(Step::CreateChannel { .. })) =>
            {
                invalid(&format!(
                    "{at}: pick an earlier step that creates a channel"
                ))
            }
            _ => Ok(()),
        }
    }
}

fn earlier<'a>(flat: &[Node<'a>], node: &Node<'_>, position: usize) -> Option<&'a Step> {
    node.before.contains(&position).then(|| flat[position].step)
}

fn list_in(flat: &[Node<'_>], position: usize) -> Option<Uuid> {
    let node = flat.get(position)?;
    match node.step {
        Step::AddListItem { list_id, .. } => Some(*list_id),
        Step::UpdateListItem { from_step, .. } if node.before.contains(from_step) => {
            list_in(flat, *from_step)
        }
        _ => None,
    }
}

fn prefixed(at: &str, error: AppError) -> AppError {
    match error {
        AppError::Validation(message) => AppError::Validation(format!("{at}: {message}")),
        other => other,
    }
}

fn check_placeholders(at: &str, text: &str, available: &[String]) -> AppResult<()> {
    for name in placeholders(text) {
        if !available.iter().any(|known| known == name) {
            return invalid(&format!(
                "{at}: {{{{{name}}}}} isn't available for this trigger"
            ));
        }
    }
    Ok(())
}

pub fn check_test(at: &str, test: &Test, kind: VariableKind<'_>) -> AppResult<()> {
    if let Test::OneOf { values } = test {
        if values.is_empty() || values.len() > MAX_TEST_VALUES {
            return invalid(&format!(
                "{at}: list 1 to {MAX_TEST_VALUES} values to match"
            ));
        }
    }
    for operand in test.operands() {
        if operand.trim().is_empty() {
            return invalid(&format!("{at}: fill in the value to compare with"));
        }
        if operand.chars().count() > TEST_VALUE_MAX_CHARS {
            return invalid(&format!(
                "{at}: a value to compare with can be at most {TEST_VALUE_MAX_CHARS} characters"
            ));
        }
    }
    match kind {
        VariableKind::Person => {
            if !matches!(
                test,
                Test::Equals { .. } | Test::OneOf { .. } | Test::IsEmpty
            ) {
                return invalid(&format!(
                    "{at}: a person can only be tested with is, is one of or is empty"
                ));
            }
            if test
                .operands()
                .iter()
                .any(|operand| operand.trim().parse::<Uuid>().is_err())
            {
                return invalid(&format!("{at}: pick the person to compare with"));
            }
        }
        VariableKind::Choice(options)
            if matches!(test, Test::Equals { .. } | Test::OneOf { .. }) =>
        {
            if let Some(missing) = test
                .operands()
                .iter()
                .find(|operand| !options.iter().any(|option| same(option, operand)))
            {
                return invalid(&format!(
                    "{at}: \u{201c}{}\u{201d} isn't one of its choices",
                    missing.trim()
                ));
            }
        }
        _ => {}
    }
    if let Test::GreaterThan { value } = test {
        if number(value).is_none() && parse_date(value).is_none() {
            return invalid(&format!(
                "{at}: \u{201c}greater than\u{201d} compares with a number or a date"
            ));
        }
    }
    Ok(())
}

fn check_wait(at: &str, wait: &Wait) -> AppResult<()> {
    match wait {
        Wait::For { amount, .. } => {
            if *amount < 1 {
                return invalid(&format!("{at}: wait for at least 1 minute"));
            }
            if wait
                .minutes()
                .is_none_or(|minutes| minutes > MAX_DELAY_DAYS * 24 * 60)
            {
                return invalid(&format!(
                    "{at}: a workflow waits at most {MAX_DELAY_DAYS} days"
                ));
            }
        }
        Wait::Until { timezone, .. } => {
            if wait.moment().is_none() {
                return invalid(&format!(
                    "{at}: the date has to look like 2026-12-01 and the time like 09:30"
                ));
            }
            if timezone.trim().is_empty() {
                return invalid(&format!("{at}: pick a timezone"));
            }
        }
    }
    Ok(())
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
        check_placeholders(at, &cell.value, available)?;
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
    pub waiting_at: Option<i32>,
    pub waiting_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunStep {
    pub position: i32,
    pub label: String,
    pub succeeded: bool,
    pub matched: Option<bool>,
    pub message_id: Option<Uuid>,
    pub list_item_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub error: Option<String>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RunWaiting {
    pub position: i32,
    pub label: String,
    pub until: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WorkflowRun {
    pub id: Uuid,
    pub workflow_version: i32,
    pub status: RunStatus,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub waiting: Option<RunWaiting>,
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

    fn urgency_form() -> serde_json::Value {
        form(json!([
            { "name": "urgency", "label": "Urgency", "kind": "select", "options": ["Urgent", "Whenever"], "required": true },
            field("what", "short_text"),
            field("owner", "person"),
        ]))
    }

    fn branch(
        value: &str,
        test: serde_json::Value,
        then: Vec<serde_json::Value>,
        otherwise: Vec<serde_json::Value>,
    ) -> serde_json::Value {
        json!({ "type": "branch", "value": value, "test": test, "then": then, "otherwise": otherwise })
    }

    #[test]
    fn branches_number_their_steps_in_order_and_join_up_after() {
        let definition = definition(json!({
            "trigger": shortcut(),
            "steps": [
                urgency_form(),
                branch("form.urgency", json!({ "op": "equals", "value": "urgent" }), vec![post("page {{form.what}}")], vec![post("filed"), post("thanks")]),
                post("done"),
            ],
        }));
        definition.check().unwrap();
        let flat = definition.flat();
        let labels: Vec<_> = flat.iter().map(|node| node.step.label()).collect();
        assert_eq!(
            labels,
            [
                "Collect a form",
                "Branch",
                "Send a message",
                "Send a message",
                "Send a message",
                "Send a message"
            ]
        );
        assert_eq!(flat[2].path, vec![(1, true)]);
        assert_eq!(flat[3].path, vec![(1, false)]);
        assert_eq!(flat[4].before, vec![0, 1, 3]);
        assert!(flat[5].path.is_empty());
        assert_eq!(flat[5].before, vec![0, 1]);
        assert!(flat[3].reached(|branch| (branch == 1).then_some(false)));
        assert!(!flat[2].reached(|branch| (branch == 1).then_some(false)));
        assert!(!flat[2].reached(|_| None));
    }

    #[test]
    fn what_a_branch_asks_stays_in_its_branch() {
        let asked_inside = json!({
            "trigger": shortcut(),
            "steps": [
                branch("trigger.channel", json!({ "op": "is_empty" }), vec![form(json!([field("why", "short_text")]))], vec![]),
                post("{{form.why}}"),
            ],
        });
        assert_eq!(
            error(asked_inside),
            "Step 3: {{form.why}} isn't available for this trigger"
        );
        let summed = json!({
            "trigger": shortcut(),
            "steps": [
                branch("trigger.user", json!({ "op": "is_empty" }), vec![form(json!([field("why", "short_text")]))], vec![
                    json!({ "type": "send_message", "channel_id": Uuid::nil(), "text": "x", "summary": true }),
                ]),
            ],
        });
        assert_eq!(
            error(summed),
            "Step 3: there is no form before it to sum up"
        );
    }

    #[test]
    fn a_branch_tests_a_value_it_can_see_against_a_condition_that_fits() {
        let with = |step: serde_json::Value| {
            error(json!({ "trigger": shortcut(), "steps": [urgency_form(), step] }))
        };
        assert_eq!(
            with(branch(
                "form.missing",
                json!({ "op": "is_empty" }),
                vec![post("x")],
                vec![]
            )),
            "Step 2: pick what the branch tests"
        );
        assert_eq!(
            with(branch(
                "webhook.id",
                json!({ "op": "is_empty" }),
                vec![post("x")],
                vec![]
            )),
            "Step 2: pick what the branch tests"
        );
        assert_eq!(
            with(branch(
                "form.urgency",
                json!({ "op": "is_empty" }),
                vec![],
                vec![]
            )),
            "Step 2: add a step to one of its branches"
        );
        assert_eq!(
            with(branch(
                "form.urgency",
                json!({ "op": "equals", "value": "Soon" }),
                vec![post("x")],
                vec![]
            )),
            "Step 2: \u{201c}Soon\u{201d} isn't one of its choices"
        );
        assert_eq!(
            with(branch(
                "form.urgency",
                json!({ "op": "one_of", "values": [] }),
                vec![post("x")],
                vec![]
            )),
            "Step 2: list 1 to 20 values to match"
        );
        assert_eq!(
            with(branch(
                "form.what",
                json!({ "op": "contains", "value": " " }),
                vec![post("x")],
                vec![]
            )),
            "Step 2: fill in the value to compare with"
        );
        assert_eq!(
            with(branch(
                "form.what",
                json!({ "op": "greater_than", "value": "lots" }),
                vec![post("x")],
                vec![]
            )),
            "Step 2: \u{201c}greater than\u{201d} compares with a number or a date"
        );
        assert_eq!(
            with(branch(
                "form.owner",
                json!({ "op": "contains", "value": "ana" }),
                vec![post("x")],
                vec![]
            )),
            "Step 2: a person can only be tested with is, is one of or is empty"
        );
        assert_eq!(
            with(branch(
                "trigger.user",
                json!({ "op": "equals", "value": "ana" }),
                vec![post("x")],
                vec![]
            )),
            "Step 2: pick the person to compare with"
        );
        definition(json!({ "trigger": shortcut(), "steps": [urgency_form(),
            branch("form.owner", json!({ "op": "one_of", "values": [Uuid::new_v4()] }), vec![post("x")], vec![]),
            branch(&format!("profile.{}", Uuid::new_v4()), json!({ "op": "contains", "value": "Berlin" }), vec![post("x")], vec![]),
        ] }))
        .check()
        .unwrap();
        assert_eq!(
            error(json!({ "trigger": schedule(), "steps": [
                branch(&format!("profile.{}", Uuid::new_v4()), json!({ "op": "is_empty" }), vec![post("x")], vec![]),
            ] })),
            "Step 1: pick what the branch tests"
        );
    }

    #[test]
    fn branches_nest_two_deep() {
        let test = json!({ "op": "is_empty" });
        let inner = branch("trigger.user", test.clone(), vec![post("x")], vec![]);
        let middle = branch("trigger.user", test.clone(), vec![inner], vec![]);
        definition(json!({ "trigger": shortcut(), "steps": [middle.clone()] }))
            .check()
            .unwrap();
        let outer = branch("trigger.user", test, vec![], vec![middle]);
        assert_eq!(
            error(json!({ "trigger": shortcut(), "steps": [outer] })),
            "Step 3: branches nest at most 2 deep"
        );
    }

    #[test]
    fn a_condition_compares_without_case_and_numbers_and_dates_by_value() {
        let test = |value: serde_json::Value| -> Test { serde_json::from_value(value).unwrap() };
        assert!(test(json!({ "op": "equals", "value": "Urgent" })).holds(" urgent "));
        assert!(!test(json!({ "op": "equals", "value": "Urgent" })).holds("urgently"));
        assert!(test(json!({ "op": "contains", "value": "DOWN" })).holds("api is down"));
        assert!(test(json!({ "op": "is_empty" })).holds("  "));
        assert!(!test(json!({ "op": "is_empty" })).holds("x"));
        assert!(test(json!({ "op": "greater_than", "value": "9" })).holds("10"));
        assert!(test(json!({ "op": "greater_than", "value": "1,000" })).holds("1,500"));
        assert!(!test(json!({ "op": "greater_than", "value": "10" })).holds("9"));
        assert!(test(json!({ "op": "greater_than", "value": "2026-11-18" })).holds("2026-12-01"));
        assert!(!test(json!({ "op": "greater_than", "value": "10" })).holds("lots"));
        assert!(test(json!({ "op": "one_of", "values": ["a", "B"] })).holds("b"));
        assert!(!test(json!({ "op": "one_of", "values": ["a", "B"] })).holds("c"));
    }

    #[test]
    fn a_wait_lasts_at_least_a_minute_and_at_most_thirty_days() {
        let wait = |wait: serde_json::Value| json!({ "trigger": schedule(), "steps": [{ "type": "delay", "wait": wait }, post("x")] });
        definition(wait(json!({ "kind": "for", "amount": 30, "unit": "days" })))
            .check()
            .unwrap();
        definition(wait(json!({ "kind": "until", "date": "2026-12-01", "time": "09:30", "timezone": "Europe/Belgrade" })))
            .check()
            .unwrap();
        assert_eq!(
            error(wait(
                json!({ "kind": "for", "amount": 0, "unit": "minutes" })
            )),
            "Step 1: wait for at least 1 minute"
        );
        assert_eq!(
            error(wait(
                json!({ "kind": "for", "amount": 721, "unit": "hours" })
            )),
            "Step 1: a workflow waits at most 30 days"
        );
        assert_eq!(
            error(wait(
                json!({ "kind": "for", "amount": i64::MAX, "unit": "days" })
            )),
            "Step 1: a workflow waits at most 30 days"
        );
        assert_eq!(
            error(wait(
                json!({ "kind": "until", "date": "1 Dec", "time": "09:30", "timezone": "UTC" })
            )),
            "Step 1: the date has to look like 2026-12-01 and the time like 09:30"
        );
        assert_eq!(
            error(wait(
                json!({ "kind": "until", "date": "2026-12-01", "time": "09:30", "timezone": " " })
            )),
            "Step 1: pick a timezone"
        );
    }

    #[test]
    fn channel_steps_use_a_channel_an_earlier_step_made_and_hand_on_their_outputs() {
        let made =
            json!({ "type": "create_channel", "name": "proj-{{form.what}}", "private": false });
        let invite = json!({ "type": "add_to_channel", "channel": { "kind": "from_step", "step": 1 },
            "people": [{ "kind": "answer", "field": "owner" }, { "kind": "trigger_user" }, { "kind": "group", "group_id": Uuid::nil() }] });
        definition(json!({
            "trigger": shortcut(),
            "steps": [urgency_form(), made.clone(), invite.clone(),
                { "type": "set_topic", "channel": { "kind": "from_step", "step": 1 }, "topic": "About {{form.what}}" },
                dm_back_to("Your channel: {{step.2.channel}} {{step.2.channel_link}}"),
                { "type": "archive_channel", "channel": { "kind": "trigger_channel" } }],
        }))
        .check()
        .unwrap();
        let at = |steps: serde_json::Value| error(json!({ "trigger": shortcut(), "steps": steps }));
        assert_eq!(
            at(json!([dm_back_to("{{step.2.channel}}"), made.clone()])),
            "Step 1: {{step.2.channel}} isn't available for this trigger"
        );
        assert_eq!(
            at(json!([urgency_form(), invite.clone()])),
            "Step 2: pick an earlier step that creates a channel"
        );
        assert_eq!(
            at(
                json!([{ "type": "create_channel", "name": "proj", "private": false }, { "type": "add_to_channel", "channel": { "kind": "from_step", "step": 0 }, "people": [] }])
            ),
            "Step 2: pick who to add"
        );
        assert_eq!(
            at(
                json!([urgency_form(), made.clone(), { "type": "add_to_channel", "channel": { "kind": "from_step", "step": 1 }, "people": [{ "kind": "answer", "field": "what" }] }])
            ),
            "Step 3: pick a person question from an earlier form"
        );
        assert_eq!(
            error(
                json!({ "trigger": schedule(), "steps": [{ "type": "archive_channel", "channel": { "kind": "trigger_channel" } }] })
            ),
            "Step 1: this trigger has no channel"
        );
        assert_eq!(
            at(json!([{ "type": "create_channel", "name": " ", "private": true }])),
            "Step 1: give the channel a name"
        );
        assert_eq!(
            at(
                json!([{ "type": "set_topic", "channel": { "kind": "trigger_channel" }, "topic": "x".repeat(501) }])
            ),
            "Step 1: a topic can be at most 500 characters"
        );
    }

    fn dm_back_to(text: &str) -> serde_json::Value {
        json!({ "type": "send_dm", "to": { "kind": "trigger_user" }, "text": text })
    }

    #[test]
    fn reactions_and_thread_replies_need_a_message_to_act_on() {
        let reaction =
            json!({ "type": "reaction_added", "channel_id": Uuid::nil(), "emoji": "\u{1F6A8}" });
        definition(json!({ "trigger": reaction, "steps": [
            { "type": "add_reaction", "emoji": "\u{1F440}" },
            { "type": "reply_in_thread", "text": "On it, {{trigger.user}}" },
            post("{{step.2.message_link}}"),
        ] }))
        .check()
        .unwrap();
        assert_eq!(
            error(
                json!({ "trigger": shortcut(), "steps": [{ "type": "add_reaction", "emoji": "\u{1F440}" }] })
            ),
            "Step 1: this trigger has no message to react to"
        );
        assert_eq!(
            error(
                json!({ "trigger": shortcut(), "steps": [{ "type": "reply_in_thread", "text": "x" }] })
            ),
            "Step 1: this trigger has no message to reply to"
        );
        assert_eq!(
            error(
                json!({ "trigger": reaction, "steps": [{ "type": "reply_in_thread", "text": " " }] })
            ),
            "Step 1 needs some text"
        );
        assert_eq!(
            error(
                json!({ "trigger": reaction, "steps": [{ "type": "add_reaction", "emoji": "x".repeat(9) }] })
            ),
            "Step 1: Reaction must be at most 8 characters"
        );
    }

    #[test]
    fn a_list_column_or_a_profile_field_names_its_source() {
        let column = Uuid::new_v4();
        assert!(matches!(
            source(&format!("step.3.column.{column}")),
            Some(Source::Column { step: 2, column_id }) if column_id == column
        ));
        assert!(
            matches!(source(&format!("profile.{column}")), Some(Source::Profile(id)) if id == column)
        );
        assert!(matches!(source("form.x"), Some(Source::Variable("form.x"))));
        assert!(source(&format!("step.0.column.{column}")).is_none());
        assert!(source(" ").is_none());
        let definition = definition(json!({
            "trigger": shortcut(),
            "steps": [urgency_form(), add_item(json!([cell("{{form.what}}")])),
                branch(&format!("step.2.column.{column}"), json!({ "op": "is_empty" }), vec![post("x")], vec![])],
        }));
        definition.check().unwrap();
        assert_eq!(definition.list_of(1), Some(Uuid::nil()));
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
