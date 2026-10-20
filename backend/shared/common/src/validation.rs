use crate::errors::{AppError, AppResult};

pub fn validate_email(email: &str) -> AppResult<()> {
    if email.len() > 255 {
        return Err(AppError::Validation("Invalid email address".into()));
    }
    let at = email
        .find('@')
        .ok_or_else(|| AppError::Validation("Invalid email address".into()))?;
    let local = &email[..at];
    let domain = &email[at + 1..];
    if local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return Err(AppError::Validation("Invalid email address".into()));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> AppResult<()> {
    if password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters".into(),
        ));
    }
    if password.len() > 128 {
        return Err(AppError::Validation(
            "Password must be at most 128 characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_display_name(name: &str) -> AppResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("Display name cannot be empty".into()));
    }
    if trimmed.len() > 100 {
        return Err(AppError::Validation(
            "Display name must be at most 100 characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_workspace_name(name: &str) -> AppResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "Workspace name cannot be empty".into(),
        ));
    }
    if trimmed.len() > 100 {
        return Err(AppError::Validation(
            "Workspace name must be at most 100 characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_channel_name(name: &str) -> AppResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("Channel name cannot be empty".into()));
    }
    if trimmed.len() > 80 {
        return Err(AppError::Validation(
            "Channel name must be at most 80 characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_message_content(content: &str) -> AppResult<()> {
    if content.trim().is_empty() {
        return Err(AppError::Validation(
            "Message content cannot be empty".into(),
        ));
    }
    if content.len() > 4000 {
        return Err(AppError::Validation(
            "Message content must be at most 4000 characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_avatar_url(url: &str) -> AppResult<()> {
    if url.len() > 500 {
        return Err(AppError::Validation(
            "Avatar URL must be at most 500 characters".into(),
        ));
    }
    let is_absolute = url.starts_with("http://") || url.starts_with("https://");
    let is_site_relative = url.starts_with('/') && !url.starts_with("//");
    if !is_absolute && !is_site_relative {
        return Err(AppError::Validation(
            "Avatar URL must be an http(s) or site-relative URL".into(),
        ));
    }
    Ok(())
}

/// Matches the rule the realtime gateway already enforces on the WebSocket path.
/// Two paths reach the same column; one limit.
pub fn validate_reaction_emoji(emoji: &str) -> AppResult<()> {
    if emoji.is_empty() {
        return Err(AppError::Validation("Reaction cannot be empty".into()));
    }
    if emoji.chars().count() > 8 {
        return Err(AppError::Validation(
            "Reaction must be at most 8 characters".into(),
        ));
    }
    if emoji.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "Reaction cannot contain control characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_reminder_content(content: &str) -> AppResult<()> {
    if content.trim().is_empty() {
        return Err(AppError::Validation(
            "Reminder content cannot be empty".into(),
        ));
    }
    if content.len() > 4000 {
        return Err(AppError::Validation(
            "Reminder content must be at most 4000 characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_channel_topic(topic: &str) -> AppResult<()> {
    if topic.len() > 500 {
        return Err(AppError::Validation(
            "Channel topic must be at most 500 characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_description(description: &str) -> AppResult<()> {
    if description.len() > 4000 {
        return Err(AppError::Validation(
            "Description must be at most 4000 characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_hook_name(name: &str) -> AppResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "Integration name cannot be empty".into(),
        ));
    }
    if trimmed.len() > 100 {
        return Err(AppError::Validation(
            "Integration name must be at most 100 characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_bio(bio: &str) -> AppResult<()> {
    if bio.len() > 500 {
        return Err(AppError::Validation(
            "Bio must be at most 500 characters".into(),
        ));
    }
    Ok(())
}

/// An IANA name, not free text — it is fed to a date formatter, and the column
/// is 50 characters wide.
pub fn validate_timezone(timezone: &str) -> AppResult<()> {
    if timezone.is_empty() || timezone.len() > 50 {
        return Err(AppError::Validation(
            "Timezone must be between 1 and 50 characters".into(),
        ));
    }
    if !timezone
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'))
    {
        return Err(AppError::Validation(
            "Timezone must be an IANA name, e.g. Europe/Belgrade".into(),
        ));
    }
    Ok(())
}

pub fn validate_icon_url(url: &str) -> AppResult<()> {
    validate_avatar_url(url)
}

/// The sender picks this one, so it has to be a real random id. A nil or
/// non-random value is a client that will collide with itself.
pub fn validate_client_message_id(id: uuid::Uuid) -> AppResult<()> {
    if id.is_nil() || id.get_version_num() != 4 {
        return Err(AppError::Validation(
            "client_message_id must be a version 4 UUID".into(),
        ));
    }
    Ok(())
}

pub fn validate_bookmark_label(label: &str) -> AppResult<()> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("Bookmark needs a label".into()));
    }
    if trimmed.chars().count() > 80 {
        return Err(AppError::Validation(
            "Bookmark label must be at most 80 characters".into(),
        ));
    }
    Ok(())
}

/// A bookmark is rendered as a link someone else clicks, so `javascript:` and
/// `data:` are not merely unusual here — they are the attack.
pub fn validate_bookmark_url(url: &str) -> AppResult<()> {
    if url.len() > 2000 {
        return Err(AppError::Validation(
            "Bookmark URL must be at most 2000 characters".into(),
        ));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AppError::Validation(
            "Bookmark URL must start with http:// or https://".into(),
        ));
    }
    if url.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "Bookmark URL cannot contain control characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_status_text(text: &str) -> AppResult<()> {
    if text.chars().count() > 100 {
        return Err(AppError::Validation(
            "Status must be at most 100 characters".into(),
        ));
    }
    if text.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "Status cannot contain control characters".into(),
        ));
    }
    Ok(())
}

pub fn validate_status_emoji(emoji: &str) -> AppResult<()> {
    if emoji.chars().count() > 16 {
        return Err(AppError::Validation(
            "Status emoji must be at most 16 characters".into(),
        ));
    }
    if emoji.chars().any(char::is_control) || emoji.contains(char::is_whitespace) {
        return Err(AppError::Validation("Status emoji is not an emoji".into()));
    }
    Ok(())
}

pub mod rules {
    use super::*;

    fn adapt(result: AppResult<()>) -> garde::Result {
        result.map_err(|e| match e {
            AppError::Validation(message) => garde::Error::new(message),
            other => garde::Error::new(other.to_string()),
        })
    }

    macro_rules! rule {
        ($name:ident, $validate:ident) => {
            pub fn $name(value: &str, _: &()) -> garde::Result {
                adapt($validate(value))
            }
        };
    }

    rule!(email, validate_email);
    rule!(password, validate_password);
    rule!(display_name, validate_display_name);
    rule!(workspace_name, validate_workspace_name);
    rule!(channel_name, validate_channel_name);
    rule!(message_content, validate_message_content);
    rule!(reaction_emoji, validate_reaction_emoji);
    rule!(reminder_content, validate_reminder_content);
    rule!(channel_topic, validate_channel_topic);
    rule!(description, validate_description);
    rule!(hook_name, validate_hook_name);
    rule!(bio, validate_bio);
    rule!(timezone, validate_timezone);
    rule!(bookmark_label, validate_bookmark_label);
    rule!(bookmark_url, validate_bookmark_url);

    pub fn avatar_url_or_empty(value: &str, _: &()) -> garde::Result {
        if value.is_empty() {
            return Ok(());
        }
        adapt(validate_avatar_url(value))
    }

    pub fn icon_url_or_empty(value: &str, _: &()) -> garde::Result {
        if value.is_empty() {
            return Ok(());
        }
        adapt(validate_icon_url(value))
    }

    pub fn status_text_or_blank(value: &str, _: &()) -> garde::Result {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        adapt(validate_status_text(trimmed))
    }

    pub fn status_emoji_or_blank(value: &str, _: &()) -> garde::Result {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        adapt(validate_status_emoji(trimmed))
    }

    pub fn client_message_id(value: &uuid::Uuid, _: &()) -> garde::Result {
        adapt(validate_client_message_id(*value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_validation_err(result: AppResult<()>) {
        match result {
            Err(AppError::Validation(_)) => {}
            Err(other) => panic!("expected AppError::Validation, got {other:?}"),
            Ok(()) => panic!("expected a validation error, got Ok(())"),
        }
    }

    #[test]
    fn validate_email_accepts_a_normal_address() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("first.last@mail.example.co").is_ok());
    }

    #[test]
    fn validate_email_rejects_empty() {
        assert_validation_err(validate_email(""));
    }

    #[test]
    fn validate_email_rejects_missing_at() {
        assert_validation_err(validate_email("userexample.com"));
    }

    #[test]
    fn validate_email_rejects_empty_local_or_domain() {
        assert_validation_err(validate_email("@example.com"));
        assert_validation_err(validate_email("user@"));
    }

    #[test]
    fn validate_email_rejects_domain_without_dot() {
        assert_validation_err(validate_email("user@localhost"));
        assert_validation_err(validate_email("user@.com"));
        assert_validation_err(validate_email("user@example."));
    }

    #[test]
    fn validate_email_rejects_too_long() {
        let too_long = format!("{}@example.com", "a".repeat(256));
        assert!(too_long.len() > 255);
        assert_validation_err(validate_email(&too_long));
    }

    #[test]
    fn validate_password_enforces_lower_bound() {
        assert_validation_err(validate_password("1234567"));
        assert!(validate_password("12345678").is_ok());
    }

    #[test]
    fn validate_password_enforces_upper_bound() {
        let max = "a".repeat(128);
        assert!(validate_password(&max).is_ok());
        let over = "a".repeat(129);
        assert_validation_err(validate_password(&over));
    }

    #[test]
    fn validate_message_content_accepts_normal() {
        assert!(validate_message_content("hello world").is_ok());
    }

    #[test]
    fn validate_message_content_rejects_empty_and_whitespace() {
        assert_validation_err(validate_message_content(""));
        assert_validation_err(validate_message_content("   \n\t  "));
    }

    #[test]
    fn validate_message_content_rejects_over_limit() {
        let max = "x".repeat(4000);
        assert!(validate_message_content(&max).is_ok());
        let over = "x".repeat(4001);
        assert_validation_err(validate_message_content(&over));
    }

    #[test]
    fn validate_reaction_emoji_matches_the_websocket_rule() {
        assert!(validate_reaction_emoji("\u{1F680}").is_ok());
        assert!(validate_reaction_emoji(&"a".repeat(8)).is_ok());
        assert_validation_err(validate_reaction_emoji(&"a".repeat(9)));
        assert_validation_err(validate_reaction_emoji(""));
        assert_validation_err(validate_reaction_emoji("a\u{0007}"));
    }

    #[test]
    fn validate_reminder_content_matches_the_message_limit() {
        assert!(validate_reminder_content(&"x".repeat(4000)).is_ok());
        assert_validation_err(validate_reminder_content(&"x".repeat(4001)));
        assert_validation_err(validate_reminder_content("   "));
    }

    #[test]
    fn validate_channel_topic_stops_at_the_column_width() {
        assert!(validate_channel_topic("").is_ok());
        assert!(validate_channel_topic(&"x".repeat(500)).is_ok());
        assert_validation_err(validate_channel_topic(&"x".repeat(501)));
    }

    #[test]
    fn validate_description_stops_at_the_message_limit() {
        assert!(validate_description(&"x".repeat(4000)).is_ok());
        assert_validation_err(validate_description(&"x".repeat(4001)));
    }

    #[test]
    fn validate_hook_name_rejects_blank_and_over_long() {
        assert!(validate_hook_name("Deploy bot").is_ok());
        assert_validation_err(validate_hook_name("   "));
        assert_validation_err(validate_hook_name(&"x".repeat(101)));
    }

    #[test]
    fn validate_bio_stops_at_five_hundred() {
        assert!(validate_bio(&"x".repeat(500)).is_ok());
        assert_validation_err(validate_bio(&"x".repeat(501)));
    }

    #[test]
    fn validate_timezone_accepts_iana_names_only() {
        assert!(validate_timezone("Europe/Belgrade").is_ok());
        assert!(validate_timezone("UTC").is_ok());
        assert!(validate_timezone("Etc/GMT+2").is_ok());
        assert_validation_err(validate_timezone(""));
        assert_validation_err(validate_timezone("Europe/Belgrade; DROP"));
        assert_validation_err(validate_timezone(&"a".repeat(51)));
    }

    #[test]
    fn validate_client_message_id_requires_a_random_uuid() {
        assert!(validate_client_message_id(uuid::Uuid::new_v4()).is_ok());
        assert_validation_err(validate_client_message_id(uuid::Uuid::nil()));
        assert_validation_err(validate_client_message_id(
            uuid::Uuid::parse_str("00000000-0000-1000-8000-000000000000").unwrap(),
        ));
    }

    #[test]
    fn validate_avatar_url_accepts_http_and_site_relative() {
        assert!(validate_avatar_url("https://cdn.example.com/a.png").is_ok());
        assert!(validate_avatar_url("http://localhost:8080/api/files/download/ws/a.png").is_ok());
        assert!(validate_avatar_url("/api/files/download/ws/id/a.png").is_ok());
    }

    #[test]
    fn validate_avatar_url_rejects_other_schemes() {
        assert_validation_err(validate_avatar_url("javascript:alert(1)"));
        assert_validation_err(validate_avatar_url(
            "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=",
        ));
        assert_validation_err(validate_avatar_url("//evil.example.com/a.png"));
        assert_validation_err(validate_avatar_url("a.png"));
    }

    #[test]
    fn validate_avatar_url_rejects_over_limit() {
        let over = format!("https://example.com/{}", "a".repeat(500));
        assert_validation_err(validate_avatar_url(&over));
    }

    #[test]
    fn validate_bookmark_url_only_accepts_a_link_a_reader_can_click() {
        assert!(validate_bookmark_url("https://example.com/runbook").is_ok());
        assert!(validate_bookmark_url("http://intranet.local/wiki").is_ok());
        assert_validation_err(validate_bookmark_url("javascript:alert(1)"));
        assert_validation_err(validate_bookmark_url("data:text/html,<script></script>"));
        assert_validation_err(validate_bookmark_url("/etc/passwd"));
        assert_validation_err(validate_bookmark_url("https://example.com/\u{7}"));
    }

    #[test]
    fn validate_bookmark_label_needs_something_to_show() {
        assert!(validate_bookmark_label("Runbook").is_ok());
        assert_validation_err(validate_bookmark_label("   "));
        assert_validation_err(validate_bookmark_label(&"a".repeat(81)));
    }

    #[test]
    fn validate_status_is_one_short_line() {
        assert!(validate_status_text("out for lunch").is_ok());
        assert!(validate_status_emoji("\u{1F355}").is_ok());
        assert_validation_err(validate_status_text(&"a".repeat(101)));
        assert_validation_err(validate_status_text("two\nlines"));
        assert_validation_err(validate_status_emoji("a b"));
    }
}
