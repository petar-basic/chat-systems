use chrono::NaiveDateTime;
use serde_json::{json, Value};
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::models::{AgendaEvent, DueAgenda, DueReminder};
use crate::blocks::{self, Input};
use crate::integrations::models::Provider;
use crate::messaging::publisher::EventPublisher;
use crate::notifications::text::{self, Words};
use crate::profile::models::Clock;
use crate::state::AppState;
use crate::workspace_bots::models::WorkspaceBotKind;
use crate::workspace_bots::repo::{BotIdentity, WorkspaceBotRepo};

const AGENDA_EVENTS: usize = 20;

fn kind_of(provider: Provider) -> WorkspaceBotKind {
    match provider {
        Provider::OutlookCalendar => WorkspaceBotKind::OutlookCalendar,
        _ => WorkspaceBotKind::GoogleCalendar,
    }
}

async fn bot(state: &AppState, workspace_id: Uuid, provider: Provider) -> AppResult<Uuid> {
    let identity = BotIdentity {
        name: provider.name(),
        avatar_url: None,
    };
    let mut tx = state.pool.begin().await?;
    let bot =
        WorkspaceBotRepo::ensure_in(&mut tx, workspace_id, kind_of(provider), &identity).await?;
    let joined = if bot.created {
        Some(
            EventPublisher::stage_workspace_member_joined(&mut tx, workspace_id, bot.user_id)
                .await?,
        )
    } else {
        None
    };
    tx.commit().await?;
    if let Some(joined) = joined {
        state.publisher.dispatch(joined).await;
    }
    Ok(bot.user_id)
}

async fn send(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
    provider: Provider,
    text: &str,
    layout: &Value,
) -> AppResult<Uuid> {
    let bot_id = bot(state, workspace_id, provider).await?;
    let channel =
        crate::conversations::routes::open_direct(state, workspace_id, bot_id, user_id).await?;
    let composed = blocks::compose(
        state,
        workspace_id,
        Input {
            text: Some(text),
            blocks: Some(layout),
            attachments: None,
        },
    )
    .await?;
    let metadata = composed.merged_into(json!({}));
    let mut tx = state.pool.begin().await?;
    let message = state
        .message_repo
        .create_bot_reply_in(
            &mut tx,
            channel.id,
            None,
            bot_id,
            &composed.content,
            &metadata,
            &[],
        )
        .await?;
    let staged = state
        .publisher
        .stage_message_created(&mut tx, &message, workspace_id, &[])
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(message.id)
}

fn clock_of(raw: Option<&str>) -> Clock {
    raw.and_then(|raw| serde_json::from_value(Value::String(raw.to_string())).ok())
        .unwrap_or_default()
}

pub fn time(at: NaiveDateTime, clock: Clock) -> String {
    match clock {
        Clock::H24 => at.format("%H:%M").to_string(),
        Clock::H12 => at.format("%-I:%M %p").to_string(),
    }
}

pub fn span(starts: NaiveDateTime, ends: NaiveDateTime, clock: Clock) -> String {
    format!("{} – {}", time(starts, clock), time(ends, clock))
}

fn title_of(words: &Words, title: Option<&str>, private: bool) -> String {
    match (private, title) {
        (true, _) => words.get("calendar.busy", &[]),
        (false, Some(title)) => title.to_string(),
        (false, None) => words.get("calendar.untitled", &[]),
    }
}

fn join_button(words: &Words, url: &str) -> Value {
    json!({
        "type": "button",
        "text": { "type": "plain_text", "text": words.get("calendar.join", &[]) },
        "url": url,
        "style": "primary",
    })
}

pub fn reminder_layout(reminder: &DueReminder) -> (String, Value) {
    let words = text::words(text::language(reminder.language.as_deref()));
    let clock = clock_of(reminder.clock.as_deref());
    let title = title_of(&words, reminder.title.as_deref(), reminder.private);
    let when = span(reminder.starts_local, reminder.ends_local, clock);
    let mut layout = vec![
        json!({ "type": "section", "text": { "type": "plain_text", "text": title } }),
        json!({
            "type": "context",
            "elements": [{
                "type": "plain_text",
                "text": words.get("calendar.startsSoon", &[("time", &when)]),
            }],
        }),
    ];
    if let Some(url) = &reminder.join_url {
        layout.push(json!({ "type": "actions", "elements": [join_button(&words, url)] }));
    }
    (
        words.get("calendar.reminder", &[("title", &title)]),
        Value::Array(layout),
    )
}

pub fn agenda_layout(agenda: &DueAgenda, events: &[AgendaEvent]) -> (String, Value) {
    let words = text::words(text::language(agenda.language.as_deref()));
    let clock = clock_of(agenda.clock.as_deref());
    let mut layout = vec![json!({
        "type": "header",
        "text": { "type": "plain_text", "text": words.get("calendar.agendaTitle", &[]) },
    })];
    for event in events.iter().take(AGENDA_EVENTS) {
        let when = if event.all_day {
            words.get("calendar.allDay", &[])
        } else {
            span(event.starts_local, event.ends_local, clock)
        };
        let mut section = json!({
            "type": "section",
            "fields": [
                { "type": "plain_text", "text": when },
                { "type": "plain_text", "text": title_of(&words, event.title.as_deref(), event.private) },
            ],
        });
        if let Some(url) = &event.join_url {
            section["accessory"] = join_button(&words, url);
        }
        layout.push(section);
    }
    let rest = events.len().saturating_sub(AGENDA_EVENTS);
    if rest > 0 {
        layout.push(json!({
            "type": "context",
            "elements": [{ "type": "plain_text", "text": words.count("calendar.more", rest as i64, &[]) }],
        }));
    }
    (
        words.count("calendar.agenda", events.len() as i64, &[]),
        Value::Array(layout),
    )
}

pub async fn remind(state: &AppState, reminder: &DueReminder) -> AppResult<Uuid> {
    let provider = Provider::parse(&reminder.provider).unwrap_or(Provider::GoogleCalendar);
    let (text, layout) = reminder_layout(reminder);
    send(
        state,
        reminder.workspace_id,
        reminder.user_id,
        provider,
        &text,
        &layout,
    )
    .await
}

pub async fn send_agenda(
    state: &AppState,
    agenda: &DueAgenda,
    events: &[AgendaEvent],
) -> AppResult<Uuid> {
    let provider = Provider::parse(&agenda.provider).unwrap_or(Provider::GoogleCalendar);
    let (text, layout) = agenda_layout(agenda, events);
    send(
        state,
        agenda.workspace_id,
        agenda.user_id,
        provider,
        &text,
        &layout,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 20)
            .and_then(|d| d.and_hms_opt(h, m, 0))
            .expect("time")
    }

    fn reminder(title: Option<&str>, private: bool, join: Option<&str>) -> DueReminder {
        DueReminder {
            workspace_id: Uuid::nil(),
            user_id: Uuid::nil(),
            provider: "google_calendar".into(),
            event_id: "e".into(),
            title: title.map(str::to_string),
            private,
            join_url: join.map(str::to_string),
            starts_local: at(10, 0),
            ends_local: at(10, 30),
            language: None,
            clock: None,
        }
    }

    #[test]
    fn times_follow_the_readers_clock() {
        assert_eq!(span(at(9, 5), at(13, 30), Clock::H24), "09:05 – 13:30");
        assert_eq!(span(at(9, 5), at(13, 30), Clock::H12), "9:05 AM – 1:30 PM");
    }

    #[test]
    fn a_reminder_names_the_event_and_offers_join() {
        let (text, layout) = reminder_layout(&reminder(
            Some("Weekly sync"),
            false,
            Some("https://meet.google.com/abc-defg-hij"),
        ));
        assert_eq!(text, "Weekly sync starts in 1 minute");
        assert_eq!(layout[0]["text"]["text"], "Weekly sync");
        assert_eq!(
            layout[1]["elements"][0]["text"],
            "10:00 – 10:30 · Starts in 1 minute"
        );
        assert_eq!(layout[2]["elements"][0]["text"]["text"], "Join");
        assert_eq!(
            layout[2]["elements"][0]["url"],
            "https://meet.google.com/abc-defg-hij"
        );
    }

    #[test]
    fn a_private_reminder_says_busy_and_has_no_button_without_a_link() {
        let (text, layout) = reminder_layout(&reminder(None, true, None));
        assert_eq!(text, "Busy starts in 1 minute");
        assert_eq!(layout.as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn an_agenda_lists_the_day_and_counts_the_rest() {
        let agenda = DueAgenda {
            workspace_id: Uuid::nil(),
            user_id: Uuid::nil(),
            provider: "outlook_calendar".into(),
            day: NaiveDate::from_ymd_opt(2026, 11, 20).expect("day"),
            timezone: "UTC".into(),
            language: None,
            clock: Some("h12".into()),
        };
        let events: Vec<AgendaEvent> = (0..22)
            .map(|i| AgendaEvent {
                title: Some(format!("Event {i}")),
                private: i == 1,
                all_day: i == 0,
                join_url: None,
                starts_local: at(9, 0),
                ends_local: at(9, 30),
            })
            .collect();
        let (text, layout) = agenda_layout(&agenda, &events);
        assert_eq!(text, "You have 22 events today");
        assert_eq!(layout[0]["text"]["text"], "Your schedule for today");
        assert_eq!(layout[1]["fields"][0]["text"], "All day");
        assert_eq!(layout[2]["fields"][1]["text"], "Busy");
        assert_eq!(layout[3]["fields"][0]["text"], "9:00 AM – 9:30 AM");
        assert_eq!(layout[21]["elements"][0]["text"], "And 2 more");
    }
}
