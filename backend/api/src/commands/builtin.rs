use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::authz;
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

use super::routes::CommandResponse;

/// `/away` is deliberately absent. Presence here is derived from whether the
/// gateway holds a socket (CS-027), so there is no flag for a command to set --
/// shipping one would mean inventing a second, manual presence state that
/// nothing else in the product reads.
pub const BUILTIN_COMMANDS: &[(&str, &str)] = &[
    ("dnd", "/dnd [minutes|off] — pause notifications"),
    ("topic", "/topic <text> — set the channel topic"),
    ("invite", "/invite @user — add somebody to this channel"),
    ("shrug", "/shrug [text] — ¯\\_(ツ)_/¯"),
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
            Ok(Some(CommandResponse::ephemeral(match until {
                Some(until) => format!("Notifications paused until {}.", until.format("%H:%M UTC")),
                None => "Notifications are on again.".into(),
            })))
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

        _ => Ok(None),
    }
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
