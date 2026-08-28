use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use chrono::{Duration, Utc};

use crate::authz;
use crate::hooks::repo::NewReminder;
use crate::state::AppState;
use crate::workspace::models::{ChannelRole, WorkspaceRole};

use super::routes::CommandResponse;

/// `/away` is deliberately absent. Presence here is derived from whether the
/// gateway holds a socket (CS-027), so there is no flag for a command to set.
/// What people actually want from `/away` -- saying what they are doing -- is a
/// custom status, which is set on the profile and is not presence.
pub const BUILTIN_COMMANDS: &[(&str, &str)] = &[
    ("dnd", "/dnd [minutes|off] — pause notifications"),
    ("topic", "/topic <text> — set the channel topic"),
    ("invite", "/invite @user — add somebody to this channel"),
    ("shrug", "/shrug [text] — ¯\\_(ツ)_/¯"),
    (
        "remind",
        "/remind me in 30m to ship — also `at 15:00` or `tomorrow at 9am`",
    ),
];

pub fn is_builtin(command: &str) -> bool {
    BUILTIN_COMMANDS.iter().any(|(name, _)| *name == command)
}

pub async fn run(
    state: &AppState,
    channel_id: Uuid,
    user_id: Uuid,
    command: &str,
    text: &str,
) -> AppResult<Option<CommandResponse>> {
    let text = text.trim();

    match command {
        "shrug" => Ok(Some(CommandResponse::in_channel(
            format!("{text} ¯\\_(ツ)_/¯").trim().to_string(),
        ))),

        "dnd" => {
            let until = parse_dnd(text)?;
            state.notification_repo.set_dnd(user_id, until).await?;
            Ok(Some(match until {
                Some(until) => CommandResponse::ephemeral_at("Notifications paused until", until),
                None => CommandResponse::ephemeral("Notifications are on again."),
            }))
        }

        "topic" => {
            let channel = state
                .workspace_service
                .repo
                .find_channel_by_id(channel_id)
                .await?
                .ok_or_else(|| AppError::NotFound("No such channel".into()))?;
            // The route has already established that this person can see the
            // channel; setting a topic is an ordinary member's job, as it is in
            // the panel.
            if text.is_empty() {
                return Ok(Some(CommandResponse::ephemeral(match channel.topic {
                    Some(topic) => format!("The topic is: {topic}"),
                    None => "This channel has no topic.".into(),
                })));
            }

            shared_common::validation::validate_description(text)?;
            state
                .workspace_service
                .repo
                .update_channel(channel_id, None, Some(text), None)
                .await?;

            Ok(Some(CommandResponse::in_channel(format!(
                "set the channel topic to: {text}"
            ))))
        }

        "invite" => {
            let target = resolve_mentioned_user(text)
                .ok_or_else(|| AppError::Validation("Name somebody: /invite @user".into()))?;
            let channel = state
                .workspace_service
                .repo
                .find_channel_by_id(channel_id)
                .await?
                .ok_or_else(|| AppError::NotFound("No such channel".into()))?;

            authz::require_workspace_member(state, channel.workspace_id, target).await?;
            state
                .workspace_service
                .repo
                .add_channel_member(channel_id, target, &ChannelRole::Member)
                .await?;
            Ok(Some(CommandResponse::in_channel(format!(
                "added <@{target}> to the channel"
            ))))
        }

        "remind" => {
            let parsed = parse_remind(text)?;
            let channel = state
                .workspace_service
                .repo
                .find_channel_by_id(channel_id)
                .await?
                .ok_or_else(|| AppError::NotFound("No such channel".into()))?;

            let target = parsed.target.unwrap_or(user_id);
            if target != user_id {
                let member = authz::require_workspace_role(
                    state,
                    channel.workspace_id,
                    user_id,
                    &WorkspaceRole::Member,
                )
                .await?;
                if !member.role.has_at_least(&WorkspaceRole::Admin) {
                    return Err(AppError::Forbidden(
                        "Only an admin can set a reminder for somebody else".into(),
                    ));
                }
                authz::require_workspace_member(state, channel.workspace_id, target).await?;
            }

            let remind_at = match parsed.when {
                RemindWhen::In(duration) => Utc::now() + duration,
                RemindWhen::AtLocal {
                    day_offset,
                    hour,
                    minute,
                } => {
                    let user = state
                        .auth_service
                        .repo()
                        .find_by_id(user_id)
                        .await?
                        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
                    state
                        .hook_repo
                        .resolve_local_time(&user.timezone, day_offset, hour, minute)
                        .await?
                }
            };

            shared_common::validation::validate_reminder_content(&parsed.content)?;
            state
                .hook_repo
                .create_reminder(NewReminder {
                    workspace_id: channel.workspace_id,
                    created_by: user_id,
                    target_user_id: target,
                    channel_id: Some(channel_id),
                    message_id: None,
                    content: &parsed.content,
                    remind_at,
                })
                .await?;

            let who = if target == user_id {
                "you".to_string()
            } else {
                format!("<@{target}>")
            };
            Ok(Some(CommandResponse::ephemeral_at(
                format!("I will remind {who}"),
                remind_at,
            )))
        }

        _ => Ok(None),
    }
}

enum RemindWhen {
    In(Duration),
    AtLocal {
        day_offset: i32,
        hour: i32,
        minute: i32,
    },
}

struct ParsedRemind {
    target: Option<Uuid>,
    when: RemindWhen,
    content: String,
}

fn remind_usage() -> AppError {
    AppError::Validation("Try /remind me in 30m to stretch".into())
}

fn parse_remind(text: &str) -> AppResult<ParsedRemind> {
    let (target, rest) = if text.starts_with("@[") {
        let label_end = text.find("](").ok_or_else(remind_usage)?;
        let after = &text[label_end + 2..];
        let id_end = after.find(')').ok_or_else(remind_usage)?;
        let id = Uuid::parse_str(after[..id_end].trim()).map_err(|_| remind_usage())?;
        (Some(id), after[id_end + 1..].trim_start())
    } else {
        (None, strip_word(text, "me").ok_or_else(remind_usage)?)
    };

    let (keyword, rest) = next_token(rest);
    let (when, rest) = match keyword.to_ascii_lowercase().as_str() {
        "in" => {
            let (duration, rest) = parse_duration_phrase(rest)?;
            (RemindWhen::In(duration), rest)
        }
        "at" => {
            let (clock, rest) = next_token(rest);
            let (hour, minute) = parse_clock(clock)?;
            (
                RemindWhen::AtLocal {
                    day_offset: 0,
                    hour,
                    minute,
                },
                rest,
            )
        }
        "tomorrow" => {
            let (hour, minute, rest) = match strip_word(rest, "at") {
                Some(after_at) => {
                    let (clock, rest) = next_token(after_at);
                    let (hour, minute) = parse_clock(clock)?;
                    (hour, minute, rest)
                }
                None => (9, 0, rest),
            };
            (
                RemindWhen::AtLocal {
                    day_offset: 1,
                    hour,
                    minute,
                },
                rest,
            )
        }
        _ => return Err(remind_usage()),
    };

    let content = strip_word(rest, "to").unwrap_or(rest).trim().to_string();
    if content.is_empty() {
        return Err(AppError::Validation(
            "Say what to remind about: /remind me in 30m to stretch".into(),
        ));
    }

    Ok(ParsedRemind {
        target,
        when,
        content,
    })
}

fn next_token(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    match text.find(char::is_whitespace) {
        Some(index) => (&text[..index], text[index..].trim_start()),
        None => (text, ""),
    }
}

fn strip_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let (token, rest) = next_token(text);
    token.eq_ignore_ascii_case(word).then_some(rest)
}

fn parse_duration_phrase(text: &str) -> AppResult<(Duration, &str)> {
    let (token, rest) = next_token(text);
    let digits = token.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return Err(remind_usage());
    }
    let amount: i64 = token[..digits].parse().map_err(|_| remind_usage())?;
    let (unit, rest) = if digits < token.len() {
        (&token[digits..], rest)
    } else {
        next_token(rest)
    };
    Ok((duration_for(unit, amount)?, rest))
}

fn duration_for(unit: &str, amount: i64) -> AppResult<Duration> {
    if amount <= 0 {
        return Err(remind_usage());
    }
    let unit = unit.to_ascii_lowercase();
    let duration = match unit.trim_end_matches('s') {
        "m" | "min" | "minute" => Duration::minutes(amount),
        "h" | "hr" | "hour" => Duration::hours(amount),
        "d" | "day" => Duration::days(amount),
        "w" | "week" => Duration::weeks(amount),
        _ => return Err(remind_usage()),
    };
    if duration > Duration::days(365) {
        return Err(AppError::Validation(
            "A reminder can be at most a year out".into(),
        ));
    }
    Ok(duration)
}

fn parse_clock(token: &str) -> AppResult<(i32, i32)> {
    let lower = token.trim().to_ascii_lowercase();
    let (body, meridiem) = match (lower.strip_suffix("pm"), lower.strip_suffix("am")) {
        (Some(body), _) => (body.trim().to_string(), Some(12)),
        (_, Some(body)) => (body.trim().to_string(), Some(0)),
        _ => (lower, None),
    };
    let (hours, minutes) = match body.split_once(':') {
        Some((hours, minutes)) => (hours, minutes),
        None => (body.as_str(), "0"),
    };
    let mut hour: i32 = hours.parse().map_err(|_| remind_usage())?;
    let minute: i32 = minutes.parse().map_err(|_| remind_usage())?;
    if !(0..=59).contains(&minute) {
        return Err(remind_usage());
    }
    match meridiem {
        Some(offset) => {
            if !(1..=12).contains(&hour) {
                return Err(remind_usage());
            }
            if hour == 12 {
                hour = 0;
            }
            hour += offset;
        }
        None => {
            if !(0..=23).contains(&hour) {
                return Err(remind_usage());
            }
        }
    }
    Ok((hour, minute))
}

fn parse_dnd(text: &str) -> AppResult<Option<chrono::DateTime<chrono::Utc>>> {
    if text.is_empty() {
        return Ok(Some(chrono::Utc::now() + chrono::Duration::minutes(60)));
    }
    if text.eq_ignore_ascii_case("off") {
        return Ok(None);
    }
    let minutes: i64 = text
        .parse()
        .map_err(|_| AppError::Validation("Try /dnd 30 or /dnd off".into()))?;
    if !(1..=24 * 60).contains(&minutes) {
        return Err(AppError::Validation(
            "Pause notifications for 1 to 1440 minutes".into(),
        ));
    }
    Ok(Some(
        chrono::Utc::now() + chrono::Duration::minutes(minutes),
    ))
}

/// The composer sends a mention as `@[label](uuid)`, so the id is already there
/// and there is no name to guess at.
fn resolve_mentioned_user(text: &str) -> Option<Uuid> {
    let start = text.find("](")? + 2;
    let end = text[start..].find(')')? + start;
    Uuid::parse_str(text[start..end].trim()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dnd_defaults_to_an_hour_and_understands_off() {
        assert!(parse_dnd("").expect("default").is_some());
        assert!(parse_dnd("off").expect("off").is_none());
        assert!(parse_dnd("OFF").expect("case").is_none());
        assert!(parse_dnd("30").expect("minutes").is_some());
    }

    #[test]
    fn dnd_refuses_what_it_cannot_mean() {
        assert!(parse_dnd("soon").is_err());
        assert!(parse_dnd("0").is_err());
        assert!(parse_dnd("100000").is_err());
    }

    fn remind(text: &str) -> ParsedRemind {
        parse_remind(text).expect("parses")
    }

    #[test]
    fn remind_understands_relative_time() {
        let parsed = remind("me in 30m to stretch");
        assert!(parsed.target.is_none());
        assert_eq!(parsed.content, "stretch");
        match parsed.when {
            RemindWhen::In(duration) => assert_eq!(duration, Duration::minutes(30)),
            RemindWhen::AtLocal { .. } => panic!("a duration is not a clock time"),
        }

        match remind("me in 2 hours check the deploy").when {
            RemindWhen::In(duration) => assert_eq!(duration, Duration::hours(2)),
            RemindWhen::AtLocal { .. } => panic!("a duration is not a clock time"),
        }
    }

    #[test]
    fn remind_understands_clock_time() {
        match remind("me at 15:30 to call back").when {
            RemindWhen::AtLocal {
                day_offset,
                hour,
                minute,
            } => assert_eq!((day_offset, hour, minute), (0, 15, 30)),
            RemindWhen::In(_) => panic!("a clock time is not a duration"),
        }

        match remind("me tomorrow at 9am standup").when {
            RemindWhen::AtLocal {
                day_offset,
                hour,
                minute,
            } => assert_eq!((day_offset, hour, minute), (1, 9, 0)),
            RemindWhen::In(_) => panic!("a clock time is not a duration"),
        }

        match remind("me tomorrow write the report").when {
            RemindWhen::AtLocal {
                day_offset,
                hour,
                minute,
            } => assert_eq!((day_offset, hour, minute), (1, 9, 0)),
            RemindWhen::In(_) => panic!("a clock time is not a duration"),
        }
    }

    #[test]
    fn remind_reads_the_mentioned_target() {
        let id = Uuid::new_v4();
        let parsed = remind(&format!("@[Ana Petrovic]({id}) in 1d to review"));
        assert_eq!(parsed.target, Some(id));
        assert_eq!(parsed.content, "review");
    }

    #[test]
    fn remind_refuses_what_it_cannot_mean() {
        assert!(parse_remind("").is_err(), "nothing to do");
        assert!(parse_remind("me in 30m").is_err(), "no content");
        assert!(parse_remind("me soon to stretch").is_err(), "no when");
        assert!(parse_remind("me in 0m to stretch").is_err(), "not a delay");
        assert!(
            parse_remind("me at 25:00 to stretch").is_err(),
            "not a clock"
        );
        assert!(
            parse_remind("me at 13pm to stretch").is_err(),
            "not a clock"
        );
        assert!(
            parse_remind("me in 400d to stretch").is_err(),
            "too far out"
        );
        assert!(parse_remind("ana in 30m to stretch").is_err(), "no target");
    }

    #[test]
    fn invite_reads_the_id_the_composer_already_sent() {
        let id = Uuid::new_v4();
        assert_eq!(
            resolve_mentioned_user(&format!("@[Ana]({id})")),
            Some(id),
            "a mention carries the id, so there is no name to guess"
        );
        assert_eq!(resolve_mentioned_user("@ana"), None);
        assert_eq!(resolve_mentioned_user(""), None);
    }
}
