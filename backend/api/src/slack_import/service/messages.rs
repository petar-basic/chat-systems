//! The second pass: messages, threads, reactions, pins and attachments.

use std::collections::HashMap;

use shared_common::errors::{AppError, AppResult};
use uuid::Uuid;

use super::super::models::*;
use super::super::mrkdwn;
use super::super::source::{read_json, ExportSource};
use super::{
    attachment_slack_ts, emoji_for, parse_ts, storage_safe_filename, MAX_ATTACHMENT_BYTES,
};
use super::{Import, Target};
use crate::files::repo::NewFile;
use crate::messaging::repo::ImportedMessage;
use chrono::{DateTime, Utc};
use shared_common::errors::is_unique_violation;
use tracing::{info, warn};

/// Often enough that a watched import looks alive, rarely enough that it is not
/// one write per message.
const PROGRESS_EVERY: usize = 500;

impl Import<'_> {
    /// The second pass. Parents are written before replies within a channel —
    /// Slack's day files are in order — so `thread_ts` resolves against a row
    /// that already exists rather than against a promise.
    pub(crate) async fn import_messages(
        &mut self,
        source: &mut dyn ExportSource,
        targets: &[(SlackConversation, Target)],
    ) -> AppResult<()> {
        for (conversation, target) in targets {
            let mut by_slack_ts = self.already_imported(*target).await?;

            let days = source.channel_days(conversation.folder())?;
            info!(
                conversation = conversation.folder(),
                days = days.len(),
                "importing"
            );

            for day in days {
                let messages: Vec<SlackMessage> = read_json(source, &day)?;
                for message in messages {
                    self.import_message(*target, &message, &mut by_slack_ts)
                        .await?;
                }
                // Messages are the pass that takes hours, so the counter moves
                // while it runs rather than at the end of it.
                if self.report.messages_imported.is_multiple_of(PROGRESS_EVERY) {
                    self.publish_progress().await;
                }
            }
        }

        Ok(())
    }

    async fn already_imported(&self, target: Target) -> AppResult<HashMap<String, Uuid>> {
        if self.dry_run {
            return Ok(HashMap::new());
        }
        let rows = self
            .state
            .slack_import_repo
            .imported_message_ids(target)
            .await?;
        Ok(rows.into_iter().collect())
    }

    async fn import_message(
        &mut self,
        target: Target,
        message: &SlackMessage,
        by_slack_ts: &mut HashMap<String, Uuid>,
    ) -> AppResult<()> {
        if message.is_system_event() {
            return Ok(());
        }
        if by_slack_ts.contains_key(&message.ts) {
            self.report.messages_already_present += 1;
            // A run without a token imports the message and skips its files. The
            // remedy this runbook gives for a bad run is "run it again", so the
            // second run has to be able to pick the files up.
            if !message.files.is_empty()
                && !by_slack_ts.contains_key(&attachment_slack_ts(&message.ts, 0))
            {
                if let Some(user_id) = message
                    .user
                    .as_deref()
                    .and_then(|slack_id| self.users.get(slack_id))
                    .map(|(id, _)| *id)
                {
                    if let Some(created_at) = parse_ts(&message.ts) {
                        self.import_files(target, user_id, message, created_at)
                            .await;
                    }
                }
            }
            return Ok(());
        }

        let Some(slack_user_id) = message.user.as_deref() else {
            // An integration posted it. There is no account here to attribute it
            // to, and inventing one would put a stranger in the member list.
            let why =
                if message.bot_id.is_some() || message.subtype.as_deref() == Some("bot_message") {
                    "posted by an integration, which has no account here"
                } else {
                    "no author in the export"
                };
            self.report.skip(format!("message {}", message.ts), why);
            return Ok(());
        };
        let Some(user_id) = self.users.get(slack_user_id).map(|(id, _)| *id) else {
            self.report.skip(
                format!("message {}", message.ts),
                format!("author {slack_user_id} was not imported"),
            );
            return Ok(());
        };

        let content = mrkdwn::convert(&message.text, &self.users);
        let created_at = parse_ts(&message.ts).ok_or_else(|| {
            AppError::BadRequest(format!("message {} has an unreadable ts", message.ts))
        })?;

        let thread_parent_id = match message.parent_ts() {
            Some(parent_ts) => match by_slack_ts.get(parent_ts) {
                Some(&parent_id) => {
                    self.report.threads_resolved += 1;
                    // A dry run has no row to point at, but it has seen the
                    // parent go past, which is what the report is claiming.
                    (!parent_id.is_nil()).then_some(parent_id)
                }
                None => {
                    // The parent is missing from the export — a reply to a
                    // message someone deleted. Keeping the reply in the channel
                    // is better than dropping what was said.
                    self.report.skip(
                        format!("thread reply {}", message.ts),
                        format!("parent {parent_ts} is not in the export"),
                    );
                    None
                }
            },
            None => None,
        };

        self.report.messages_imported += 1;
        if self.dry_run {
            by_slack_ts.insert(message.ts.clone(), Uuid::nil());
            return Ok(());
        }

        let stored_id = self
            .write_message(
                target,
                user_id,
                &content,
                thread_parent_id,
                &message.ts,
                created_at,
            )
            .await?;
        by_slack_ts.insert(message.ts.clone(), stored_id);

        self.import_reactions(stored_id, message).await?;

        if !message.pinned_to.is_empty() {
            self.state.message_repo.set_pinned(stored_id, true).await?;
            self.report.pins += 1;
        }

        self.import_files(target, user_id, message, created_at)
            .await;

        Ok(())
    }

    async fn write_message(
        &self,
        target: Target,
        user_id: Uuid,
        content: &str,
        thread_parent_id: Option<Uuid>,
        slack_ts: &str,
        created_at: DateTime<Utc>,
    ) -> AppResult<Uuid> {
        let message = self
            .state
            .message_repo
            .insert_imported(ImportedMessage {
                channel_id: target,
                user_id,
                content,
                thread_parent_id,
                slack_ts,
                created_at,
            })
            .await?;
        Ok(message.id)
    }

    async fn import_reactions(
        &mut self,
        message_id: Uuid,
        message: &SlackMessage,
    ) -> AppResult<()> {
        for reaction in &message.reactions {
            let emoji = emoji_for(&reaction.name);
            for slack_user_id in &reaction.users {
                let Some(user_id) = self.users.get(slack_user_id).map(|(id, _)| *id) else {
                    continue;
                };
                let added = self
                    .state
                    .message_repo
                    .add_reaction(message_id, user_id, &emoji)
                    .await
                    .map(|_| ());
                match added {
                    Ok(_) => self.report.reactions += 1,
                    // The same person reacting twice is the same reaction; a
                    // re-run finds it already there.
                    Err(ref e) if is_unique_violation(e) => {}
                    Err(e) => return Err(AppError::Database(e.to_string())),
                }
            }
        }
        Ok(())
    }

    /// Slack's file URLs expire, so the bytes are fetched now or not at all. A
    /// file that cannot be fetched is reported rather than failing the import:
    /// the message it belonged to is still worth keeping.
    async fn import_files(
        &mut self,
        target: Target,
        user_id: Uuid,
        message: &SlackMessage,
        created_at: DateTime<Utc>,
    ) {
        for file in &message.files {
            if let Some(why) = file.unavailable() {
                self.report
                    .skip(format!("file {} on {}", file.filename(), message.ts), why);
                continue;
            }
            let Some(url) = file.download_url() else {
                continue;
            };

            let body = match self.slack.fetch(url, MAX_ATTACHMENT_BYTES).await {
                Ok(body) => body,
                Err(e) => {
                    self.report
                        .skip(format!("file {} on {}", file.filename(), message.ts), e);
                    continue;
                }
            };

            let filename = file.filename().to_string();
            let content_type = file
                .mimetype
                .clone()
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let storage_key = format!(
                "{}/{}/{}",
                self.workspace_id,
                Uuid::new_v4(),
                storage_safe_filename(&filename)
            );
            let size = body.len() as i64;

            if let Err(e) = self
                .state
                .file_storage
                .upload(&storage_key, body, &content_type)
                .await
            {
                self.report
                    .skip(format!("file {filename} on {}", message.ts), e.to_string());
                continue;
            }

            // The attachment reads as one the composer produced, which is what
            // makes it render and what makes CS-009's access rules apply to it.
            let url = self.state.file_storage.public_url(&storage_key);
            let content = format!("[file: {filename}]({url})");
            let attachment_ts = attachment_slack_ts(&message.ts, self.report.files_imported);
            let stored_id = match self
                .write_message(target, user_id, &content, None, &attachment_ts, created_at)
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    warn!("import: failed to record attachment {filename}: {e}");
                    continue;
                }
            };

            if let Err(e) = self
                .state
                .file_repo
                .create(NewFile {
                    user_id,
                    workspace_id: self.workspace_id,
                    message_id: Some(stored_id),
                    filename: &filename,
                    storage_key: &storage_key,
                    mime_type: &content_type,
                    size_bytes: size,
                })
                .await
            {
                warn!("import: failed to record file row for {filename}: {e}");
                continue;
            }

            self.report.files_imported += 1;
        }
    }
}
