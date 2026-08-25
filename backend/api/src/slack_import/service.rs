use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tracing::{info, warn};
use uuid::Uuid;

use shared_common::errors::{is_unique_violation, AppError, AppResult};

use crate::files::repo::NewFile;
use crate::messaging::repo::ImportedMessage;
use crate::state::AppState;
use crate::workspace::models::{ChannelRole, ChannelType, WorkspaceRole};

use super::files::FileFetcher;
use super::models::*;
use super::mrkdwn;
use super::source::{read_json, ExportSource};

/// Users are matched by email, and by nothing else. A Slack handle is not an
/// identity: two people can carry the same one across workspaces, and matching
/// on it would attribute somebody's history to a stranger.
pub struct Import<'a> {
    state: &'a Arc<AppState>,
    files: &'a dyn FileFetcher,
    workspace_id: Uuid,
    /// Imported channels are created by the workspace owner: Slack's creator may
    /// have no account here, and `channels.created_by` has to point at somebody.
    owner_id: Uuid,
    dry_run: bool,
    /// Slack user id → (our user id, the name to render in a mention).
    users: HashMap<String, (Uuid, String)>,
    channels: HashMap<String, Uuid>,
    report: ImportReport,
}

impl<'a> Import<'a> {
    pub fn new(
        state: &'a Arc<AppState>,
        files: &'a dyn FileFetcher,
        workspace_id: Uuid,
        dry_run: bool,
    ) -> Self {
        Self {
            state,
            files,
            workspace_id,
            owner_id: Uuid::nil(),
            dry_run,
            users: HashMap::new(),
            channels: HashMap::new(),
            report: ImportReport::default(),
        }
    }

    pub async fn run(mut self, source: &mut dyn ExportSource) -> AppResult<ImportReport> {
        let run_id = if self.dry_run {
            None
        } else {
            Some(
                self.state
                    .slack_import_repo
                    .start_run(self.workspace_id, source.label(), self.dry_run)
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?,
            )
        };

        self.owner_id = self
            .state
            .workspace_service
            .repo
            .find_workspace_by_id(self.workspace_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("No such workspace".into()))?
            .owner_id;

        self.load_existing_mappings().await?;
        self.import_users(source).await?;
        let channels = self.import_channels(source).await?;
        self.import_messages(source, &channels).await?;

        if let Some(run_id) = run_id {
            let report = serde_json::to_value(&self.report).unwrap_or_default();
            self.state
                .slack_import_repo
                .finish_run(run_id, &report)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }

        Ok(self.report)
    }

    /// A resumed run starts from what the last one wrote down, not from zero.
    async fn load_existing_mappings(&mut self) -> AppResult<()> {
        let users = self
            .state
            .slack_import_repo
            .user_mappings(self.workspace_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        for (slack_id, user_id, name) in users {
            self.users.insert(slack_id, (user_id, name));
        }

        let channels = self
            .state
            .slack_import_repo
            .channel_mappings(self.workspace_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        for (slack_id, channel_id) in channels {
            self.channels.insert(slack_id, channel_id);
        }

        Ok(())
    }

    async fn import_users(&mut self, source: &mut dyn ExportSource) -> AppResult<()> {
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

    async fn import_channels(
        &mut self,
        source: &mut dyn ExportSource,
    ) -> AppResult<Vec<SlackChannel>> {
        let channels: Vec<SlackChannel> = read_json(source, "channels.json")?;

        for channel in &channels {
            if self.channels.contains_key(&channel.id) {
                self.report.channels_reused += 1;
                self.add_channel_members(channel).await?;
                continue;
            }

            let name = sanitise_channel_name(&channel.name);
            let existing = self
                .state
                .workspace_service
                .repo
                .find_channel_by_name(self.workspace_id, &name)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

            let channel_id = match existing {
                Some(found) => {
                    self.report.channels_reused += 1;
                    found.id
                }
                None if self.dry_run => {
                    self.report.channels_created += 1;
                    Uuid::nil()
                }
                None => {
                    let created = self
                        .state
                        .workspace_service
                        .repo
                        .create_channel(
                            self.workspace_id,
                            &name,
                            &ChannelType::Public,
                            Some(channel.purpose.value.as_str()).filter(|d| !d.is_empty()),
                            self.owner_id,
                            false,
                        )
                        .await
                        .map_err(|e| AppError::Database(e.to_string()))?;
                    if !channel.topic.value.is_empty() {
                        self.state
                            .workspace_service
                            .repo
                            .update_channel(created.id, None, Some(&channel.topic.value), None)
                            .await
                            .map_err(|e| AppError::Database(e.to_string()))?;
                    }
                    self.report.channels_created += 1;
                    created.id
                }
            };

            if !self.dry_run {
                self.state
                    .slack_import_repo
                    .map_channel(self.workspace_id, &channel.id, channel_id)
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?;
            }
            self.channels.insert(channel.id.clone(), channel_id);
            self.add_channel_members(channel).await?;
        }

        Ok(channels)
    }

    async fn add_channel_members(&mut self, channel: &SlackChannel) -> AppResult<()> {
        let Some(&channel_id) = self.channels.get(&channel.id) else {
            return Ok(());
        };

        for slack_user_id in &channel.members {
            let Some(user_id) = self.users.get(slack_user_id).map(|(id, _)| *id) else {
                continue;
            };
            self.report.memberships += 1;
            if self.dry_run {
                continue;
            }
            self.state
                .workspace_service
                .repo
                .add_channel_member(channel_id, user_id, &ChannelRole::Member)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }

        Ok(())
    }

    /// The second pass. Parents are written before replies within a channel —
    /// Slack's day files are in order — so `thread_ts` resolves against a row
    /// that already exists rather than against a promise.
    async fn import_messages(
        &mut self,
        source: &mut dyn ExportSource,
        channels: &[SlackChannel],
    ) -> AppResult<()> {
        for channel in channels {
            let Some(&channel_id) = self.channels.get(&channel.id) else {
                continue;
            };

            let mut by_slack_ts: HashMap<String, Uuid> = if self.dry_run {
                HashMap::new()
            } else {
                self.state
                    .slack_import_repo
                    .imported_message_ids(channel_id)
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?
                    .into_iter()
                    .collect()
            };

            let days = source.channel_days(&channel.name)?;
            info!(channel = %channel.name, days = days.len(), "importing channel");

            for day in days {
                let messages: Vec<SlackMessage> = read_json(source, &day)?;
                for message in messages {
                    self.import_message(channel_id, &message, &mut by_slack_ts)
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn import_message(
        &mut self,
        channel_id: Uuid,
        message: &SlackMessage,
        by_slack_ts: &mut HashMap<String, Uuid>,
    ) -> AppResult<()> {
        if message.is_system_event() {
            return Ok(());
        }
        if by_slack_ts.contains_key(&message.ts) {
            self.report.messages_already_present += 1;
            return Ok(());
        }

        let Some(slack_user_id) = message.user.as_deref() else {
            self.report
                .skip(format!("message {}", message.ts), "no author in the export");
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

        let stored = self
            .state
            .message_repo
            .insert_imported(ImportedMessage {
                channel_id,
                user_id,
                content: &content,
                thread_parent_id,
                slack_ts: &message.ts,
                created_at,
            })
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        by_slack_ts.insert(message.ts.clone(), stored.id);

        self.import_reactions(stored.id, message).await?;

        if !message.pinned_to.is_empty() {
            self.state
                .message_repo
                .set_pinned(stored.id, true)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
            self.report.pins += 1;
        }

        self.import_files(channel_id, user_id, message, created_at)
            .await;

        Ok(())
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
                match self
                    .state
                    .message_repo
                    .add_reaction(message_id, user_id, &emoji)
                    .await
                {
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
        channel_id: Uuid,
        user_id: Uuid,
        message: &SlackMessage,
        created_at: DateTime<Utc>,
    ) {
        for file in &message.files {
            let Some(url) = file.download_url() else {
                self.report
                    .skip(format!("file on {}", message.ts), "no download url");
                continue;
            };

            let body = match self.files.fetch(url).await {
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
            let storage_key = format!("{}/{}/{}", self.workspace_id, Uuid::new_v4(), filename);
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
            let stored = match self
                .state
                .message_repo
                .insert_imported(ImportedMessage {
                    channel_id,
                    user_id,
                    content: &content,
                    thread_parent_id: None,
                    slack_ts: &format!("{}-file-{}", message.ts, self.report.files_imported),
                    created_at,
                })
                .await
            {
                Ok(stored) => stored,
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
                    message_id: Some(stored.id),
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

/// Slack allows names our channel rules do not; the alternative to rewriting
/// them is refusing to import the channel at all.
pub fn sanitise_channel_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '-' | '_' => c,
            'A'..='Z' => c.to_ascii_lowercase(),
            _ => '-',
        })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "imported".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

/// Slack's `ts` is seconds with a microsecond fraction: `1700000000.001200`.
pub fn parse_ts(ts: &str) -> Option<DateTime<Utc>> {
    let (seconds, fraction) = ts.split_once('.').unwrap_or((ts, "0"));
    let seconds: i64 = seconds.parse().ok()?;
    let micros: u32 = format!("{fraction:0<6}")[..6].parse().ok()?;
    DateTime::from_timestamp(seconds, micros * 1_000)
}

/// `:tada:` is Slack's; the reaction column holds the character.
pub fn emoji_for(name: &str) -> String {
    let base = name.split("::").next().unwrap_or(name);
    match base {
        "+1" | "thumbsup" => "👍".into(),
        "-1" | "thumbsdown" => "👎".into(),
        "tada" => "🎉".into(),
        "heart" => "❤️".into(),
        "eyes" => "👀".into(),
        "white_check_mark" | "heavy_check_mark" => "✅".into(),
        "fire" => "🔥".into(),
        "rocket" => "🚀".into(),
        "smile" | "smiley" => "😄".into(),
        "joy" => "😂".into(),
        "thinking_face" => "🤔".into(),
        "clap" => "👏".into(),
        "pray" => "🙏".into(),
        "wave" => "👋".into(),
        // Anything unmapped keeps Slack's own name. It renders as text rather
        // than as a picture, which is a smaller loss than dropping the reaction.
        other => format!(":{other}:"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slack_timestamp_keeps_its_moment() {
        let at = parse_ts("1700000000.001200").expect("parses");
        assert_eq!(at.timestamp(), 1_700_000_000);
        assert_eq!(at.timestamp_subsec_millis(), 1);
        assert!(parse_ts("not-a-ts").is_none());
        assert!(
            parse_ts("1700000000").is_some(),
            "a whole second is a ts too"
        );
    }

    #[test]
    fn channel_names_are_rewritten_rather_than_refused() {
        assert_eq!(sanitise_channel_name("General Chat"), "general-chat");
        assert_eq!(sanitise_channel_name("#ops"), "ops");
        assert_eq!(sanitise_channel_name("---"), "imported");
        assert_eq!(sanitise_channel_name("naručivanje"), "naru-ivanje");
    }

    #[test]
    fn a_known_shortcode_becomes_the_character_and_the_rest_stays_readable() {
        assert_eq!(emoji_for("tada"), "🎉");
        assert_eq!(emoji_for("+1::skin-tone-3"), "👍");
        assert_eq!(emoji_for("shipit"), ":shipit:");
    }
}
