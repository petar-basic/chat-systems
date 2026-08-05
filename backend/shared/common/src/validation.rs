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
}
