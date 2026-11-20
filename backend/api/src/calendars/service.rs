use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use tracing::warn;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::google::GoogleCalendar;
use super::models::{
    parse_agenda_at, CalendarState, ConnectionKey, CurrentEvent, DueSync, Fetched,
    UpdateCalendarSettings, WINDOW_HOURS, WINDOW_RENEW_HOURS,
};
use super::outlook::OutlookCalendar;
use super::repo::CalendarStatus;
use super::{messages, SyncError};
use crate::integrations::models::Provider;
use crate::integrations::service::{access_token, enabled_app};
use crate::notifications::text;
use crate::state::AppState;

const SYNCS_PER_TICK: i64 = 25;
const MEETING_EMOJI: &str = "🗓️";
const AWAY_EMOJI: &str = "⛔";

fn calendar(raw: &str) -> AppResult<Provider> {
    Provider::parse(raw)
        .filter(|p| p.is_calendar())
        .ok_or_else(|| AppError::NotFound("No such calendar".into()))
}

pub async fn list(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<Vec<CalendarState>> {
    let apps = state.integration_repo.apps(workspace_id).await?;
    let connected = state
        .integration_repo
        .connected_accounts(workspace_id, user_id)
        .await?;
    let mut out = Vec::new();
    for provider in Provider::CALENDARS {
        let account = connected
            .iter()
            .find(|(p, _)| p == provider.as_str())
            .map(|(_, account)| account.clone());
        let settings = match account {
            Some(_) => state
                .calendar_repo
                .settings(workspace_id, user_id, provider.as_str())
                .await?
                .map(Into::into),
            None => None,
        };
        out.push(CalendarState {
            provider,
            name: provider.name().into(),
            set_up: apps
                .iter()
                .any(|a| a.provider == provider.as_str() && a.enabled),
            connected_account: account,
            settings,
        });
    }
    Ok(out)
}

pub async fn update(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
    raw: &str,
    req: &UpdateCalendarSettings,
) -> AppResult<CalendarState> {
    let provider = calendar(raw)?;
    let agenda_at = match req.agenda_at.as_deref() {
        Some(raw) => Some(
            parse_agenda_at(raw)
                .ok_or_else(|| AppError::Validation("agenda_at: use a time like 08:30".into()))?,
        ),
        None => None,
    };
    let key = ConnectionKey {
        workspace_id,
        user_id,
        provider: provider.as_str().into(),
    };
    if !state
        .calendar_repo
        .update_settings(&key, req.status, req.reminders, req.agenda, agenda_at)
        .await?
    {
        return Err(AppError::NotFound(format!(
            "Connect {} first",
            provider.name()
        )));
    }
    if req.status == Some(false) {
        forget(state, workspace_id, user_id, provider).await?;
    }
    list(state, workspace_id, user_id)
        .await?
        .into_iter()
        .find(|c| c.provider == provider)
        .ok_or_else(|| AppError::Internal("The calendar went missing".into()))
}

pub async fn forget(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
    provider: Provider,
) -> AppResult<()> {
    state
        .calendar_repo
        .clear_status(workspace_id, user_id, Some(provider.as_str()))
        .await?;
    if state.calendar_repo.clear_away(user_id).await? {
        recompute_presence(state, user_id).await;
    }
    Ok(())
}

pub async fn forget_app(state: &AppState, workspace_id: Uuid, provider: Provider) -> AppResult<()> {
    if provider.is_calendar() {
        state
            .calendar_repo
            .clear_provider_statuses(workspace_id, provider.as_str())
            .await?;
    }
    Ok(())
}

async fn recompute_presence(state: &AppState, user_id: Uuid) {
    if let Err(e) = state
        .publisher
        .publish(
            "presence.recompute",
            serde_json::json!({ "user_id": user_id }),
        )
        .await
    {
        warn!("calendars: failed to recompute presence: {e}");
    }
}

fn trimmed(base: &str) -> String {
    base.trim_end_matches('/').to_string()
}

async fn fetch_once(
    state: &AppState,
    provider: Provider,
    token: &str,
    sync_token: Option<&str>,
    window: (DateTime<Utc>, DateTime<Utc>),
) -> Result<Fetched, SyncError> {
    match provider {
        Provider::OutlookCalendar => {
            OutlookCalendar {
                graph_base: trimmed(&state.config.microsoft_graph_url),
            }
            .sync(token, sync_token, window)
            .await
        }
        _ => {
            GoogleCalendar {
                api_base: trimmed(&state.config.google_api_url),
            }
            .sync(token, sync_token, window)
            .await
        }
    }
}

async fn fetch(
    state: &AppState,
    sync: &DueSync,
    now: DateTime<Utc>,
) -> AppResult<(Fetched, (DateTime<Utc>, DateTime<Utc>))> {
    let provider = calendar(&sync.provider)?;
    let app = enabled_app(state, sync.workspace_id, provider)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("{} is turned off", provider.name())))?;
    let connection = state
        .integration_repo
        .connection(sync.workspace_id, sync.user_id, provider.as_str())
        .await?
        .ok_or_else(|| AppError::NotFound("Not connected".into()))?;
    let token = access_token(state, &app, sync.user_id, &connection).await?;

    let fresh = (now, now + Duration::hours(WINDOW_HOURS));
    let incremental = match (&sync.sync_token, sync.window_end) {
        (Some(sync_token), Some(end)) if end - now >= Duration::hours(WINDOW_RENEW_HOURS) => {
            Some((sync_token.as_str(), (now, end)))
        }
        _ => None,
    };
    if let Some((sync_token, window)) = incremental {
        match fetch_once(state, provider, &token, Some(sync_token), window).await {
            Ok(fetched) => return Ok((fetched, window)),
            Err(SyncError::Gone) => {}
            Err(SyncError::Failed(e)) => return Err(e),
        }
    }
    match fetch_once(state, provider, &token, None, fresh).await {
        Ok(fetched) => Ok((fetched, fresh)),
        Err(SyncError::Gone) => Err(AppError::BadRequest(format!(
            "{} refused a full sync",
            provider.name()
        ))),
        Err(SyncError::Failed(e)) => Err(e),
    }
}

fn reason(error: &AppError) -> String {
    match error {
        AppError::Unauthorized(_) => "The connection has expired; connect again".into(),
        AppError::BadRequest(message) | AppError::NotFound(message) => message.clone(),
        _ => "The calendar could not be read; it is tried again in five minutes".into(),
    }
}

pub async fn sync_one(state: &AppState, sync: &DueSync, now: DateTime<Utc>) -> AppResult<()> {
    match fetch(state, sync, now).await {
        Ok((fetched, window)) => {
            state
                .calendar_repo
                .apply(
                    sync,
                    &fetched.changes,
                    fetched.full,
                    &fetched.token,
                    window,
                    now,
                )
                .await?;
        }
        Err(error) => {
            warn!(provider = %sync.provider, "calendars: sync failed: {error}");
            state
                .calendar_repo
                .sync_failed(&sync.key(), &reason(&error), now, false)
                .await?;
        }
    }
    Ok(())
}

pub async fn sync_due(state: &AppState, now: DateTime<Utc>) -> AppResult<usize> {
    let due = state.calendar_repo.claim_due(now, SYNCS_PER_TICK).await?;
    for sync in &due {
        if let Err(e) = sync_one(state, sync, now).await {
            warn!("calendars: could not store a sync: {e}");
        }
    }
    Ok(due.len())
}

fn chosen<'a>(events: &[&'a CurrentEvent]) -> Option<&'a CurrentEvent> {
    let latest = |kind: &str| {
        events
            .iter()
            .filter(|e| e.kind == kind)
            .max_by_key(|e| e.ends_at)
            .copied()
    };
    latest("out_of_office").or_else(|| latest("meeting"))
}

pub async fn statuses(state: &AppState, now: DateTime<Utc>) -> AppResult<usize> {
    let current = state.calendar_repo.current_events(now).await?;
    let mut by_member: HashMap<(Uuid, Uuid), Vec<&CurrentEvent>> = HashMap::new();
    for event in &current {
        by_member
            .entry((event.workspace_id, event.user_id))
            .or_default()
            .push(event);
    }
    let desired: HashMap<(Uuid, Uuid), &CurrentEvent> = by_member
        .iter()
        .filter_map(|(member, events)| chosen(events).map(|event| (*member, event)))
        .collect();
    let (workspace_ids, user_ids): (Vec<Uuid>, Vec<Uuid>) = desired.keys().copied().unzip();
    let members = state
        .calendar_repo
        .member_statuses(&workspace_ids, &user_ids, now)
        .await?;

    let mut changed = 0;
    for member in members {
        let shown = member.status_emoji.is_some() || member.status_text.is_some();
        let Some(event) = desired.get(&(member.workspace_id, member.user_id)) else {
            if member.status_source.is_some()
                && shown
                && state
                    .calendar_repo
                    .clear_status(member.workspace_id, member.user_id, None)
                    .await?
            {
                changed += 1;
            }
            continue;
        };
        if member.status_source.is_none()
            && (shown || event.status_event_id.as_deref() == Some(event.event_id.as_str()))
        {
            continue;
        }
        let words = text::words(text::language(member.language.as_deref()));
        let (emoji, key) = match (event.kind.as_str(), event.private) {
            ("out_of_office", _) => (AWAY_EMOJI, "calendar.outOfOffice"),
            (_, true) => (MEETING_EMOJI, "calendar.busy"),
            _ => (MEETING_EMOJI, "calendar.inMeeting"),
        };
        let status_text = words.get(key, &[]);
        if member.status_source.as_deref() == Some(event.provider.as_str())
            && member.status_emoji.as_deref() == Some(emoji)
            && member.status_text.as_deref() == Some(status_text.as_str())
            && member.status_expires_at == Some(event.ends_at)
        {
            continue;
        }
        if state
            .calendar_repo
            .set_status(
                member.workspace_id,
                member.user_id,
                &CalendarStatus {
                    provider: &event.provider,
                    emoji,
                    text: &status_text,
                    until: event.ends_at,
                },
                &event.event_id,
                now,
            )
            .await?
        {
            changed += 1;
        }
    }
    Ok(changed)
}

pub async fn presence(state: &AppState, now: DateTime<Utc>) -> AppResult<usize> {
    let changed = state.calendar_repo.mark_away(now).await?;
    for user_id in &changed {
        recompute_presence(state, *user_id).await;
    }
    Ok(changed.len())
}

pub async fn reminders(state: &AppState, now: DateTime<Utc>) -> AppResult<usize> {
    let due = state.calendar_repo.claim_reminders(now).await?;
    let mut sent = 0;
    for reminder in &due {
        match messages::remind(state, reminder).await {
            Ok(_) => sent += 1,
            Err(e) => {
                warn!("calendars: a reminder could not be sent: {e}");
                state.calendar_repo.release_reminder(reminder).await?;
            }
        }
    }
    Ok(sent)
}

pub async fn agendas(state: &AppState, now: DateTime<Utc>) -> AppResult<usize> {
    let due = state.calendar_repo.claim_agendas(now).await?;
    let mut sent = 0;
    for agenda in &due {
        let events = state.calendar_repo.day_events(agenda).await?;
        if events.is_empty() {
            continue;
        }
        match messages::send_agenda(state, agenda, &events).await {
            Ok(_) => sent += 1,
            Err(e) => warn!("calendars: an agenda could not be sent: {e}"),
        }
    }
    Ok(sent)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tick {
    pub synced: usize,
    pub statuses: usize,
    pub presence: usize,
    pub reminders: usize,
    pub agendas: usize,
}

pub async fn tick(state: &AppState, now: DateTime<Utc>) -> AppResult<Tick> {
    let synced = sync_due(state, now).await?;
    state.calendar_repo.purge_past(now).await?;
    Ok(Tick {
        synced,
        statuses: statuses(state, now).await?,
        presence: presence(state, now).await?,
        reminders: reminders(state, now).await?,
        agendas: agendas(state, now).await?,
    })
}
