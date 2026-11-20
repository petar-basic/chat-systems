use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::integrations::models::Provider;

pub const SYNC_EVERY_SECS: i64 = 300;
pub const WINDOW_HOURS: i64 = 48;
pub const WINDOW_RENEW_HOURS: i64 = 24;
pub const KEEP_PAST_HOURS: i64 = 24;
pub const MAX_TITLE: usize = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Event,
    Meeting,
    OutOfOffice,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::Event => "event",
            EventKind::Meeting => "meeting",
            EventKind::OutOfOffice => "out_of_office",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    At(DateTime<Utc>),
    Day(NaiveDate),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncedEvent {
    pub id: String,
    pub title: Option<String>,
    pub starts: When,
    pub ends: When,
    pub kind: EventKind,
    pub attending: bool,
    pub private: bool,
    pub join_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Upsert(SyncedEvent),
    Remove(String),
}

#[derive(Debug)]
pub struct Fetched {
    pub changes: Vec<Change>,
    pub token: String,
    pub full: bool,
}

#[derive(Debug, Clone)]
pub struct ConnectionKey {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
}

#[derive(Debug, Clone)]
pub struct DueSync {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub sync_token: Option<String>,
    pub window_end: Option<DateTime<Utc>>,
    pub timezone: String,
}

impl DueSync {
    pub fn key(&self) -> ConnectionKey {
        ConnectionKey {
            workspace_id: self.workspace_id,
            user_id: self.user_id,
            provider: self.provider.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CurrentEvent {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub event_id: String,
    pub kind: String,
    pub private: bool,
    pub ends_at: DateTime<Utc>,
    pub status_event_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MemberStatus {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub status_emoji: Option<String>,
    pub status_text: Option<String>,
    pub status_expires_at: Option<DateTime<Utc>>,
    pub status_source: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DueReminder {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub event_id: String,
    pub title: Option<String>,
    pub private: bool,
    pub join_url: Option<String>,
    pub starts_local: NaiveDateTime,
    pub ends_local: NaiveDateTime,
    pub language: Option<String>,
    pub clock: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DueAgenda {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub day: NaiveDate,
    pub timezone: String,
    pub language: Option<String>,
    pub clock: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgendaEvent {
    pub title: Option<String>,
    pub private: bool,
    pub all_day: bool,
    pub join_url: Option<String>,
    pub starts_local: NaiveDateTime,
    pub ends_local: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct SyncSettings {
    pub status_on: bool,
    pub reminders_on: bool,
    pub agenda_on: bool,
    pub agenda_at: NaiveTime,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CalendarSettings {
    pub status: bool,
    pub reminders: bool,
    pub agenda: bool,
    #[schema(example = "09:00")]
    pub agenda_at: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl From<SyncSettings> for CalendarSettings {
    fn from(s: SyncSettings) -> Self {
        Self {
            status: s.status_on,
            reminders: s.reminders_on,
            agenda: s.agenda_on,
            agenda_at: s.agenda_at.format("%H:%M").to_string(),
            last_synced_at: s.last_synced_at,
            last_error: s.last_error,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CalendarState {
    pub provider: Provider,
    pub name: String,
    pub set_up: bool,
    pub connected_account: Option<String>,
    pub settings: Option<CalendarSettings>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CalendarList {
    pub data: Vec<CalendarState>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateCalendarSettings {
    pub status: Option<bool>,
    pub reminders: Option<bool>,
    pub agenda: Option<bool>,
    #[schema(example = "08:30")]
    pub agenda_at: Option<String>,
}

pub fn parse_agenda_at(raw: &str) -> Option<NaiveTime> {
    let (hours, minutes) = raw.trim().split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 {
        return None;
    }
    NaiveTime::from_hms_opt(hours.parse().ok()?, minutes.parse().ok()?, 0)
}

pub fn clip_title(title: &str) -> Option<String> {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        return None;
    }
    Some(match title.char_indices().nth(MAX_TITLE) {
        Some((cut, _)) => format!("{}…", &title[..cut]),
        None => title,
    })
}

pub fn says_out_of_office(title: &str) -> bool {
    title
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.eq_ignore_ascii_case("ooo") || word.eq_ignore_ascii_case("pto"))
}

pub fn join_link(candidates: &[Option<&str>]) -> Option<String> {
    candidates.iter().flatten().find_map(|raw| {
        raw.split_whitespace().find_map(|word| {
            let word = word.trim_matches(|c: char| "<>()[]\"',;".contains(c));
            let url = reqwest::Url::parse(word).ok()?;
            let host = url.host_str()?.to_ascii_lowercase();
            let meeting = host == "meet.google.com"
                || host == "teams.microsoft.com"
                || host == "teams.live.com"
                || host == "zoom.us"
                || host.ends_with(".zoom.us")
                || host == "whereby.com"
                || host.ends_with(".webex.com");
            (url.scheme() == "https" && meeting && word.len() <= 2000).then(|| url.to_string())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agenda_time_is_two_digit_hours_and_minutes() {
        assert_eq!(parse_agenda_at("08:30"), NaiveTime::from_hms_opt(8, 30, 0));
        assert_eq!(parse_agenda_at("23:59"), NaiveTime::from_hms_opt(23, 59, 0));
        for bad in ["8:30", "24:00", "12:60", "noon", "", "12:3"] {
            assert_eq!(parse_agenda_at(bad), None, "{bad}");
        }
    }

    #[test]
    fn ooo_and_pto_are_out_of_office_as_words() {
        assert!(says_out_of_office("OOO"));
        assert!(says_out_of_office("PTO - Lisbon"));
        assert!(says_out_of_office("Ana (ooo)"));
        assert!(!says_out_of_office("Moon landing"));
        assert!(!says_out_of_office("Laptop return"));
    }

    #[test]
    fn a_join_link_is_an_https_meeting_link() {
        assert_eq!(
            join_link(&[None, Some("Room 4, https://acme.zoom.us/j/123?pwd=x")]),
            Some("https://acme.zoom.us/j/123?pwd=x".into())
        );
        assert_eq!(
            join_link(&[Some("https://meet.google.com/abc-defg-hij")]),
            Some("https://meet.google.com/abc-defg-hij".into())
        );
        assert_eq!(join_link(&[Some("http://meet.google.com/abc")]), None);
        assert_eq!(join_link(&[Some("https://evil.test/zoom.us")]), None);
        assert_eq!(join_link(&[Some("Board room")]), None);
    }

    #[test]
    fn a_title_is_trimmed_and_cut() {
        assert_eq!(clip_title("  Weekly   sync "), Some("Weekly sync".into()));
        assert_eq!(clip_title("   "), None);
        let long = "a".repeat(MAX_TITLE + 10);
        assert_eq!(
            clip_title(&long).map(|t| t.chars().count()),
            Some(MAX_TITLE + 1)
        );
    }
}
