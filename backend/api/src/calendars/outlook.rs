use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use serde_json::Value;

use shared_common::errors::AppError;

use super::models::{
    clip_title, join_link, says_out_of_office, Change, EventKind, Fetched, SyncedEvent, When,
};
use super::SyncError;
use crate::integrations::http;

const MAX_PAGES: usize = 20;
const PREFER: &str = "odata.maxpagesize=100, outlook.timezone=\"UTC\"";

pub struct OutlookCalendar {
    pub graph_base: String,
}

fn failed(what: &str, e: impl std::fmt::Display) -> SyncError {
    SyncError::Failed(http::provider_error("Outlook Calendar", what, e))
}

impl OutlookCalendar {
    fn ours(&self, link: &str) -> bool {
        link.starts_with(&format!("{}/", self.graph_base))
    }

    pub async fn sync(
        &self,
        token: &str,
        delta_link: Option<&str>,
        window: (DateTime<Utc>, DateTime<Utc>),
    ) -> Result<Fetched, SyncError> {
        let client = http::client().map_err(SyncError::Failed)?;
        let incremental = delta_link.filter(|link| self.ours(link));
        let mut url = match incremental {
            Some(link) => link.to_string(),
            None => {
                let mut url =
                    reqwest::Url::parse(&format!("{}/v1.0/me/calendarView/delta", self.graph_base))
                        .map_err(|e| failed("request", e))?;
                url.query_pairs_mut()
                    .append_pair(
                        "startDateTime",
                        &window.0.to_rfc3339_opts(SecondsFormat::Secs, true),
                    )
                    .append_pair(
                        "endDateTime",
                        &window.1.to_rfc3339_opts(SecondsFormat::Secs, true),
                    );
                url.to_string()
            }
        };
        let mut changes = Vec::new();
        for _ in 0..MAX_PAGES {
            let response = client
                .get(&url)
                .bearer_auth(token)
                .header("Prefer", PREFER)
                .send()
                .await
                .map_err(|e| failed("request", e))?;
            let status = response.status();
            if status == reqwest::StatusCode::GONE {
                return Err(SyncError::Gone);
            }
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(SyncError::Failed(AppError::Unauthorized(
                    "Outlook refused the connection; connect again".into(),
                )));
            }
            let body = response
                .json::<Value>()
                .await
                .map_err(|e| failed("response", e))?;
            if !status.is_success() {
                let code = body
                    .pointer("/error/code")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if code.to_ascii_lowercase().contains("syncstate") {
                    return Err(SyncError::Gone);
                }
                return Err(failed("request", status));
            }
            changes.extend(
                body.get("value")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(change_of),
            );
            if let Some(delta) = body.get("@odata.deltaLink").and_then(Value::as_str) {
                if !self.ours(delta) {
                    return Err(failed("response", "a delta link to another host"));
                }
                return Ok(Fetched {
                    changes,
                    token: delta.to_string(),
                    full: incremental.is_none(),
                });
            }
            match body.get("@odata.nextLink").and_then(Value::as_str) {
                Some(next) if self.ours(next) => url = next.to_string(),
                Some(_) => return Err(failed("response", "a next link to another host")),
                None => return Err(failed("response", "no delta link in the last page")),
            }
        }
        Err(failed("response", "too many pages"))
    }
}

fn when(value: &Value, all_day: bool) -> Option<When> {
    let raw = value.get("dateTime").and_then(Value::as_str)?;
    let at = NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f").ok()?;
    Some(if all_day {
        When::Day(at.date())
    } else {
        When::At(at.and_utc())
    })
}

pub fn change_of(item: &Value) -> Option<Change> {
    let text = |pointer: &str| item.pointer(pointer).and_then(Value::as_str);
    let flag = |key: &str| item.get(key).and_then(Value::as_bool).unwrap_or(false);
    let id = text("/id")?.to_string();
    let gone = Change::Remove(id.clone());
    if item.get("@removed").is_some() || flag("isCancelled") {
        return Some(gone);
    }
    let response = text("/responseStatus/response");
    if response == Some("declined") {
        return Some(gone);
    }
    let organizer = flag("isOrganizer");
    let attending = organizer || matches!(response, Some("accepted" | "organizer"));
    let invited = item
        .get("attendees")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    let guests = invited || !organizer;

    let all_day = flag("isAllDay");
    let (Some(starts), Some(ends)) = (
        item.get("start").and_then(|v| when(v, all_day)),
        item.get("end").and_then(|v| when(v, all_day)),
    ) else {
        return Some(gone);
    };
    let ordered = match (starts, ends) {
        (When::At(a), When::At(b)) => b > a,
        (When::Day(a), When::Day(b)) => b > a,
        _ => false,
    };
    if !ordered {
        return Some(gone);
    }
    let show_as = text("/showAs").unwrap_or("busy");
    let busy = !matches!(show_as, "free" | "workingElsewhere");
    let private = matches!(text("/sensitivity"), Some("private" | "confidential"));
    let subject = text("/subject").unwrap_or_default();
    let kind = if show_as == "oof" || says_out_of_office(subject) {
        EventKind::OutOfOffice
    } else if guests && busy && !all_day {
        EventKind::Meeting
    } else {
        EventKind::Event
    };

    Some(Change::Upsert(SyncedEvent {
        id,
        title: if private { None } else { clip_title(subject) },
        starts,
        ends,
        kind,
        attending,
        private,
        join_url: join_link(&[
            text("/onlineMeeting/joinUrl"),
            text("/onlineMeetingUrl"),
            text("/location/displayName"),
        ]),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use serde_json::json;

    fn upserted(item: Value) -> SyncedEvent {
        match change_of(&item) {
            Some(Change::Upsert(event)) => event,
            other => panic!("expected an upsert, got {other:?}"),
        }
    }

    #[test]
    fn an_accepted_teams_meeting() {
        let event = upserted(json!({
            "id": "AAMk1",
            "subject": "Planning",
            "start": { "dateTime": "2026-11-20T09:00:00.0000000", "timeZone": "UTC" },
            "end": { "dateTime": "2026-11-20T09:30:00.0000000", "timeZone": "UTC" },
            "responseStatus": { "response": "accepted" },
            "attendees": [{ "emailAddress": { "address": "ana@acme.test" } }],
            "showAs": "busy",
            "onlineMeeting": { "joinUrl": "https://teams.microsoft.com/l/meetup-join/19%3a1" }
        }));
        assert_eq!(event.kind, EventKind::Meeting);
        assert!(event.attending);
        assert_eq!(
            event.starts,
            When::At("2026-11-20T09:00:00Z".parse().expect("time"))
        );
        assert!(event
            .join_url
            .is_some_and(|url| url.starts_with("https://teams.microsoft.com/")));
    }

    #[test]
    fn an_all_day_out_of_office() {
        let event = upserted(json!({
            "id": "AAMk2",
            "subject": "Away",
            "isAllDay": true,
            "isOrganizer": true,
            "showAs": "oof",
            "start": { "dateTime": "2026-11-20T00:00:00.0000000", "timeZone": "UTC" },
            "end": { "dateTime": "2026-11-21T00:00:00.0000000", "timeZone": "UTC" }
        }));
        assert_eq!(event.kind, EventKind::OutOfOffice);
        assert_eq!(
            event.starts,
            When::Day(NaiveDate::from_ymd_opt(2026, 11, 20).expect("day"))
        );
    }

    #[test]
    fn removed_cancelled_and_declined_are_gone() {
        for item in [
            json!({ "id": "x", "@removed": { "reason": "deleted" } }),
            json!({ "id": "x", "isCancelled": true }),
            json!({ "id": "x", "responseStatus": { "response": "declined" },
                    "start": { "dateTime": "2026-11-20T09:00:00.0000000" },
                    "end": { "dateTime": "2026-11-20T10:00:00.0000000" } }),
        ] {
            assert_eq!(change_of(&item), Some(Change::Remove("x".into())), "{item}");
        }
    }

    #[test]
    fn a_private_event_keeps_no_title() {
        let event = upserted(json!({
            "id": "AAMk3",
            "subject": "Therapy",
            "sensitivity": "private",
            "isOrganizer": true,
            "start": { "dateTime": "2026-11-20T15:00:00.0000000" },
            "end": { "dateTime": "2026-11-20T16:00:00.0000000" }
        }));
        assert!(event.private);
        assert_eq!(event.title, None);
        assert_eq!(event.kind, EventKind::Event);
    }
}
