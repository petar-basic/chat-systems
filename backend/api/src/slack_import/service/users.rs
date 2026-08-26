//! Accounts: matched by email, created pending, made members.

use shared_common::errors::{AppError, AppResult};
use uuid::Uuid;

use super::super::models::*;
use super::super::source::{read_json, ExportSource};
use super::Import;
use crate::workspace::models::WorkspaceRole;

impl Import<'_> {
    pub(crate) async fn import_users(&mut self, source: &mut dyn ExportSource) -> AppResult<()> {
        let users: Vec<SlackUser> = read_json(source, "users.json")?;

        for user in users {
            if self.users.contains_key(&user.id) {
                continue;
            }
            if user.is_bot {
                self.report
                    .skip(format!("user {}", user.id), "a bot has no account here");
                continue;
            }
            let Some(email) = user.profile.email.as_deref().filter(|e| e.contains('@')) else {
                // Deactivated Slack accounts often keep no email at all, and an
                // account with no address cannot be matched or invited.
                self.report
                    .skip(format!("user {}", user.id), "no email in the export");
                continue;
            };

            let existing = self
                .state
                .auth_service
                .repo()
                .find_by_email(&email.to_lowercase())
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

            let (user_id, created) = match existing {
                Some(found) => (found.id, false),
                None if self.dry_run => (Uuid::nil(), true),
                None => {
                    // Pending, with no password: the person activates through the
                    // ordinary invite flow, and until then their history is
                    // attributed to an account only they can claim.
                    let created = self
                        .state
                        .auth_service
                        .repo()
                        .create(
                            &email.to_lowercase(),
                            None,
                            Some(&user.display_name()),
                            false,
                        )
                        .await
                        .map_err(|e| AppError::Database(e.to_string()))?;
                    (created.id, true)
                }
            };

            if created {
                self.report.users_created += 1;
            } else {
                self.report.users_matched += 1;
            }

            if !self.dry_run {
                self.ensure_workspace_member(user_id).await?;
                self.state
                    .slack_import_repo
                    .map_user(self.workspace_id, &user.id, user_id)
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?;
            }
            self.users
                .insert(user.id.clone(), (user_id, user.display_name()));
        }

        Ok(())
    }

    async fn ensure_workspace_member(&self, user_id: Uuid) -> AppResult<()> {
        if self
            .state
            .workspace_service
            .repo
            .get_member(self.workspace_id, user_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .is_some()
        {
            return Ok(());
        }

        self.state
            .workspace_service
            .repo
            .add_member(self.workspace_id, user_id, &WorkspaceRole::Member)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }
}
