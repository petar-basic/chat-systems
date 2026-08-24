pub mod builtin;
pub mod routes;

use shared_common::errors::{AppError, AppResult};

/// The name as it is typed, without the slash. Kept to the same shape as a
/// channel name so that what somebody reads in a message is what they can type.
pub fn validate_command_name(command: &str) -> AppResult<String> {
    let command = command.trim().trim_start_matches('/').to_lowercase();

    if command.len() < 2 || command.len() > 32 {
        return Err(AppError::Validation(
            "A command name is 2 to 32 characters".into(),
        ));
    }
    if !command
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(AppError::Validation(
            "A command name uses lowercase letters, digits, dashes and underscores".into(),
        ));
    }
    if builtin::is_builtin(&command) {
        return Err(AppError::Validation(format!(
            "/{command} is built in on this instance"
        )));
    }

    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_name_is_normalized_the_way_people_type_it() {
        assert_eq!(validate_command_name("/Deploy").expect("valid"), "deploy");
        assert_eq!(
            validate_command_name(" roll-dice ").expect("valid"),
            "roll-dice"
        );
    }

    #[test]
    fn a_registered_command_cannot_shadow_a_built_in_one() {
        assert!(validate_command_name("dnd").is_err());
        assert!(validate_command_name("/topic").is_err());
        assert!(validate_command_name("deploy").is_ok());
    }

    #[test]
    fn a_name_that_would_not_parse_after_a_slash_is_refused() {
        assert!(validate_command_name("d").is_err());
        assert!(validate_command_name("two words").is_err());
        assert!(validate_command_name("deploy!").is_err());
    }
}
