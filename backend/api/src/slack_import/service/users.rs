//! Accounts: matched by email, created pending (or suspended for someone who left), made members or guests.

use shared_common::errors::AppResult;
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
            let email = match user.profile.email.as_deref().filter(|e| e.contains('@')) {
                Some(email) => email.to_lowercase(),
                None if user.deleted => format!("slack-{}@deleted.invalid", user.id.to_lowercase()),
                None => {
                    self.report
                        .skip(format!("user {}", user.id), "no email in the export");
                    continue;
                }
            };

            let existing = self.state.auth_service.repo().find_by_email(&email).await?;

            let (user_id, created) = match existing {
                Some(found) => (found.id, false),
                None if self.dry_run => (Uuid::nil(), true),
                None if user.deleted => {
                    let created = self
                        .state
                        .auth_service
                        .repo()
                        .create_suspended(&email, &user.display_name())
                        .await?;
                    (created.id, true)
                }
                None => {
                    // Pending, with no password: the person activates through the
                    // ordinary invite flow, and until then their history is
                    // attributed to an account only they can claim.
                    let created = self
                        .state
                        .auth_service
                        .repo()
                        .create(&email, None, Some(&user.display_name()), false)
                        .await?;
                    (created.id, true)
                }
            };

            if created && user.deleted {
                self.report.users_suspended += 1;
            } else if created {
                self.report.users_created += 1;
                if email.ends_with("@slack-corp.com") {
                    // Slack replaces a deactivated member's address with one of
                    // its own. The account is still worth creating — their
                    // history needs an owner — but nobody can ever claim it.
                    self.report.note(format!(
                        "{} has Slack's placeholder address, so that account cannot be claimed",
                        user.display_name()
                    ));
                }
            } else {
                self.report.users_matched += 1;
            }

            let role = if user.is_guest() {
                self.report.guests += 1;
                WorkspaceRole::Guest
            } else {
                WorkspaceRole::Member
            };
            if !self.dry_run {
                self.ensure_workspace_member(user_id, &role).await?;
                self.state
                    .slack_import_repo
                    .map_user(self.workspace_id, &user.id, user_id)
                    .await?;
            }
            self.users
                .insert(user.id.clone(), (user_id, user.display_name()));
        }

        Ok(())
    }

    async fn ensure_workspace_member(&self, user_id: Uuid, role: &WorkspaceRole) -> AppResult<()> {
        if self
            .state
            .workspace_service
            .repo
            .get_member(self.workspace_id, user_id)
            .await?
            .is_some()
        {
            return Ok(());
        }

        self.state
            .workspace_service
            .repo
            .add_member(self.workspace_id, user_id, role)
            .await?;
        Ok(())
    }
}
