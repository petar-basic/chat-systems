use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use serde_json::Value;

use shared_common::errors::AppError;

use super::models::{
    clip_title, join_link, says_out_of_office, Change, EventKind, Fetched, SyncedEvent, When,
};
use super::SyncError;
use crate::integrations::http;

const MAX_PAGES: usize = 20;
const FIELDS: &str = "nextPageToken,nextSyncToken,items(id,status,summary,start,end,visibility,eventType,transparency,attendees(self,responseStatus),hangoutLink,conferenceData/entryPoints(entryPointType,uri),location)";

pub struct GoogleCalendar {
    pub api_base: String,
}

fn failed(what: &str, e: impl std::fmt::Display) -> SyncError {
    SyncError::Failed(http::provider_error("Google Calendar", what, e))
}

impl GoogleCalendar {
    pub async fn sync(
        &self,
        token: &str,
        sync_token: Option<&str>,
        window: (DateTime<Utc>, DateTime<Utc>),
    ) -> Result<Fetched, SyncError> {
        let client = http::client().map_err(SyncError::Failed)?;
        let mut changes = Vec::new();
        let mut page: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let mut query: Vec<(&str, String)> = vec![
                ("singleEvents", "true".into()),
                ("maxResults", "250".into()),
                ("fields", FIELDS.into()),
            ];
            match sync_token {
                Some(sync_token) => query.push(("syncToken", sync_token.into())),
                None => {
                    query.push((
                        "timeMin",
                        window.0.to_rfc3339_opts(SecondsFormat::Secs, true),
                    ));
                    query.push((
                        "timeMax",
                        window.1.to_rfc3339_opts(SecondsFormat::Secs, true),
                    ));
                }
            }
            if let Some(page) = &page {
                query.push(("pageToken", page.clone()));
            }
            let response = client
                .get(format!(
                    "{}/calendar/v3/calendars/primary/events",
                    self.api_base
                ))
                .bearer_auth(token)
                .query(&query)
                .send()
                .await
                .map_err(|e| failed("request", e))?;
            match response.status() {
                reqwest::StatusCode::GONE => return Err(SyncError::Gone),
                reqwest::StatusCode::UNAUTHORIZED => {
                    return Err(SyncError::Failed(AppError::Unauthorized(
                        "Google Calendar refused the connection; connect again".into(),
                    )))
                }
                status if !status.is_success() => return Err(failed("request", status)),
                _ => {}
            }
            let body = response
                .json::<Value>()
                .await
                .map_err(|e| failed("response", e))?;
            changes.extend(
                body.get("items")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(change_of),
            );
            if let Some(next) = body.get("nextSyncToken").and_then(Value::as_str) {
                return Ok(Fetched {
                    changes,
                    token: next.to_string(),
                    full: sync_token.is_none(),
                });
            }
            page = body
                .get("nextPageToken")
                .and_then(Value::as_str)
                .map(str::to_string);
            if page.is_none() {
                return Err(failed("response", "no sync token in the last page"));
            }
        }
        Err(failed("response", "too many pages"))
    }
}

fn when(value: &Value) -> Option<When> {
    if let Some(at) = value.get("dateTime").and_then(Value::as_str) {
        return DateTime::parse_from_rfc3339(at)
            .ok()
            .map(|at| When::At(at.with_timezone(&Utc)));
    }
    value
        .get("date")
        .and_then(Value::as_str)
        .and_then(|day| NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        .map(When::Day)
}

fn ordered(starts: When, ends: When) -> bool {
    match (starts, ends) {
        (When::At(a), When::At(b)) => b > a,
        (When::Day(a), When::Day(b)) => b > a,
        _ => false,
    }
}

pub fn change_of(item: &Value) -> Option<Change> {
    let text = |key: &str| item.get(key).and_then(Value::as_str);
    let id = text("id")?.to_string();
    let gone = Change::Remove(id.clone());
    if text("status") == Some("cancelled")
        || matches!(text("eventType"), Some("workingLocation" | "birthday"))
    {
        return Some(gone);
    }
    let attendees = item
        .get("attendees")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let me = attendees
        .iter()
        .find(|a| a.get("self").and_then(Value::as_bool) == Some(true));
    let response = me
        .and_then(|a| a.get("responseStatus"))
        .and_then(Value::as_str);
    if response == Some("declined") {
        return Some(gone);
    }
    let attending = me.is_none() || response == Some("accepted");
    let guests = attendees.len() > usize::from(me.is_some());

    let (Some(starts), Some(ends)) = (
        item.get("start").and_then(when),
        item.get("end").and_then(when),
    ) else {
        return Some(gone);
    };
    if !ordered(starts, ends) {
        return Some(gone);
    }
    let timed = matches!(starts, When::At(_));
    let busy = text("transparency") != Some("transparent");
    let private = matches!(text("visibility"), Some("private" | "confidential"));
    let summary = text("summary").unwrap_or_default();
    let kind = if text("eventType") == Some("outOfOffice") || says_out_of_office(summary) {
        EventKind::OutOfOffice
    } else if guests && busy && timed {
        EventKind::Meeting
    } else {
        EventKind::Event
    };
    let video = item
        .pointer("/conferenceData/entryPoints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|e| e.get("entryPointType").and_then(Value::as_str) == Some("video"))
        .and_then(|e| e.get("uri"))
        .and_then(Value::as_str);

    Some(Change::Upsert(SyncedEvent {
        id,
        title: if private { None } else { clip_title(summary) },
        starts,
        ends,
        kind,
        attending,
        private,
        join_url: join_link(&[text("hangoutLink"), video, text("location")]),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn upserted(item: Value) -> SyncedEvent {
        match change_of(&item) {
            Some(Change::Upsert(event)) => event,
            other => panic!("expected an upsert, got {other:?}"),
        }
    }

    #[test]
    fn an_accepted_meeting_with_a_meet_link() {
        let event = upserted(json!({
            "id": "m1",
            "summary": "Weekly sync",
            "start": { "dateTime": "2026-11-20T10:00:00+01:00" },
            "end": { "dateTime": "2026-11-20T10:30:00+01:00" },
            "attendees": [
                { "self": true, "responseStatus": "accepted" },
                { "responseStatus": "needsAction" }
            ],
            "hangoutLink": "https://meet.google.com/abc-defg-hij"
        }));
        assert_eq!(event.kind, EventKind::Meeting);
        assert!(event.attending);
        assert_eq!(event.title.as_deref(), Some("Weekly sync"));
        assert_eq!(
            event.starts,
            When::At("2026-11-20T09:00:00Z".parse().expect("time"))
        );
        assert_eq!(
            event.join_url.as_deref(),
            Some("https://meet.google.com/abc-defg-hij")
        );
    }

    #[test]
    fn a_private_event_keeps_no_title() {
        let event = upserted(json!({
            "id": "p1",
            "summary": "Doctor",
            "visibility": "private",
            "start": { "dateTime": "2026-11-20T12:00:00Z" },
            "end": { "dateTime": "2026-11-20T13:00:00Z" }
        }));
        assert!(event.private);
        assert_eq!(event.title, None);
        assert_eq!(event.kind, EventKind::Event);
    }

    #[test]
    fn declined_cancelled_and_working_location_are_removed() {
        for item in [
            json!({ "id": "x", "status": "cancelled" }),
            json!({ "id": "x", "eventType": "workingLocation",
                    "start": { "date": "2026-11-20" }, "end": { "date": "2026-11-21" } }),
            json!({ "id": "x", "start": { "dateTime": "2026-11-20T12:00:00Z" },
                    "end": { "dateTime": "2026-11-20T13:00:00Z" },
                    "attendees": [{ "self": true, "responseStatus": "declined" }, {}] }),
        ] {
            assert_eq!(change_of(&item), Some(Change::Remove("x".into())), "{item}");
        }
    }

    #[test]
    fn out_of_office_by_type_or_title() {
        let typed = upserted(json!({
            "id": "o1", "eventType": "outOfOffice",
            "start": { "dateTime": "2026-11-20T00:00:00Z" },
            "end": { "dateTime": "2026-11-21T00:00:00Z" }
        }));
        assert_eq!(typed.kind, EventKind::OutOfOffice);
        let titled = upserted(json!({
            "id": "o2", "summary": "PTO",
            "start": { "date": "2026-11-20" }, "end": { "date": "2026-11-21" }
        }));
        assert_eq!(titled.kind, EventKind::OutOfOffice);
        assert_eq!(
            titled.starts,
            When::Day(NaiveDate::from_ymd_opt(2026, 11, 20).expect("day"))
        );
    }

    #[test]
    fn a_tentative_invite_is_not_attended_and_a_free_one_sets_no_status() {
        let tentative = upserted(json!({
            "id": "t1", "summary": "Maybe",
            "start": { "dateTime": "2026-11-20T12:00:00Z" },
            "end": { "dateTime": "2026-11-20T13:00:00Z" },
            "attendees": [{ "self": true, "responseStatus": "tentative" }, {}]
        }));
        assert!(!tentative.attending);
        let free = upserted(json!({
            "id": "f1", "summary": "Lunch talk", "transparency": "transparent",
            "start": { "dateTime": "2026-11-20T12:00:00Z" },
            "end": { "dateTime": "2026-11-20T13:00:00Z" },
            "attendees": [{ "self": true, "responseStatus": "accepted" }, {}]
        }));
        assert_eq!(free.kind, EventKind::Event);
    }
}
