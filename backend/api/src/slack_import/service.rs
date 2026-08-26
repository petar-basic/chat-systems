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

use crate::conversations::models::ConversationKind;

use super::files::SlackClient;
use super::models::*;
use super::mrkdwn;
use super::source::{read_json, ExportSource};

/// Where a Slack conversation landed. Public and private channels are channels;
/// DMs and group DMs are conversations, which is the same split this product
/// makes natively.
#[derive(Debug, Clone, Copy)]
enum Target {
    Channel(Uuid),
    Conversation(Uuid),
}

/// Users are matched by email, and by nothing else. A Slack handle is not an
/// identity: two people can carry the same one across workspaces, and matching
/// on it would attribute somebody's history to a stranger.
pub struct Import<'a> {
    state: &'a Arc<AppState>,
    slack: &'a dyn SlackClient,
    workspace_id: Uuid,
    /// Imported channels are created by the workspace owner: Slack's creator may
    /// have no account here, and `channels.created_by` has to point at somebody.
    owner_id: Uuid,
    dry_run: bool,
    /// Slack user id → (our user id, the name to render in a mention).
    users: HashMap<String, (Uuid, String)>,
    channels: HashMap<String, Uuid>,
    conversations: HashMap<String, Uuid>,
    report: ImportReport,
}

impl<'a> Import<'a> {
    pub fn new(
        state: &'a Arc<AppState>,
        slack: &'a dyn SlackClient,
        workspace_id: Uuid,
        dry_run: bool,
    ) -> Self {
        Self {
            state,
            slack,
            workspace_id,
            owner_id: Uuid::nil(),
            dry_run,
            users: HashMap::new(),
            channels: HashMap::new(),
            conversations: HashMap::new(),
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

        self.import_custom_emoji(source).await?;

        let mut targets = Vec::new();
        targets.extend(
            self.import_channels(source, "channels.json", &ChannelType::Public)
                .await?,
        );
        targets.extend(
            self.import_channels(source, "groups.json", &ChannelType::Private)
                .await?,
        );
        targets.extend(self.import_conversations(source, "dms.json").await?);
        targets.extend(self.import_conversations(source, "mpims.json").await?);

        self.import_messages(source, &targets).await?;

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

        let conversations = self
            .state
            .slack_import_repo
            .conversation_mappings(self.workspace_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        for (slack_id, conversation_id) in conversations {
            self.conversations.insert(slack_id, conversation_id);
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

    /// Custom emoji are not in the export — Slack keeps them behind `emoji.list`.
    /// An export carrying `emoji.json` (some tools write one) is used when it is
    /// there, so an import can bring them across without a token.
    async fn import_custom_emoji(&mut self, source: &mut dyn ExportSource) -> AppResult<()> {
        let listed = if source.has("emoji.json") {
            read_json::<HashMap<String, String>>(source, "emoji.json")?
        } else {
            match self.slack.custom_emoji().await {
                Ok(listed) => listed,
                Err(why) => {
                    self.report.skip("custom emoji", why);
                    return Ok(());
                }
            }
        };

        // Direct emoji first: an alias is only meaningful once the image it
        // points at has somewhere to point.
        let mut aliases: Vec<(String, String)> = Vec::new();
        let mut stored: HashMap<String, String> = HashMap::new();

        for (name, url) in &listed {
            if let Some(target) = url.strip_prefix("alias:") {
                aliases.push((name.clone(), target.to_string()));
                continue;
            }
            if let Some(key) = self.import_one_emoji(name, url).await? {
                stored.insert(name.clone(), key);
            }
        }

        for (name, target) in aliases {
            let key = match stored.get(&target) {
                Some(key) => key.clone(),
                None => match self.existing_emoji_key(&target).await? {
                    Some(key) => key,
                    None => {
                        self.report.skip(
                            format!("emoji :{name}:"),
                            format!("an alias of :{target}:, which was not imported"),
                        );
                        continue;
                    }
                },
            };
            // The alias reuses the image rather than downloading it twice.
            self.record_emoji(&name, &key).await?;
        }

        Ok(())
    }

    async fn import_one_emoji(&mut self, name: &str, url: &str) -> AppResult<Option<String>> {
        let name = match crate::emoji::routes::validate_name(name) {
            Ok(name) => name,
            Err(e) => {
                self.report.skip(format!("emoji :{name}:"), e.to_string());
                return Ok(None);
            }
        };

        if let Some(key) = self.existing_emoji_key(&name).await? {
            self.report.emoji_already_present += 1;
            return Ok(Some(key));
        }

        if self.dry_run {
            self.report.emoji_imported += 1;
            return Ok(None);
        }

        let bytes = match self.slack.fetch(url).await {
            Ok(bytes) => bytes,
            Err(why) => {
                self.report.skip(format!("emoji :{name}:"), why);
                return Ok(None);
            }
        };
        if bytes.len() as u64 > crate::emoji::routes::MAX_EMOJI_BYTES {
            self.report.skip(
                format!("emoji :{name}:"),
                format!("{} bytes is over the emoji limit", bytes.len()),
            );
            return Ok(None);
        }

        let content_type = content_type_for(url);
        let storage_key = format!("emoji/{}/{}", self.workspace_id, Uuid::new_v4());
        if let Err(e) = self
            .state
            .file_storage
            .upload(&storage_key, bytes, content_type)
            .await
        {
            self.report.skip(format!("emoji :{name}:"), e.to_string());
            return Ok(None);
        }

        self.record_emoji(&name, &storage_key).await?;
        Ok(Some(storage_key))
    }

    async fn record_emoji(&mut self, name: &str, storage_key: &str) -> AppResult<()> {
        if self.dry_run {
            self.report.emoji_imported += 1;
            return Ok(());
        }
        match self
            .state
            .emoji_repo
            .create(self.workspace_id, name, storage_key, self.owner_id)
            .await
        {
            Ok(_) => self.report.emoji_imported += 1,
            Err(ref e) if is_unique_violation(e) => self.report.emoji_already_present += 1,
            Err(e) => return Err(AppError::Database(e.to_string())),
        }
        Ok(())
    }

    async fn existing_emoji_key(&self, name: &str) -> AppResult<Option<String>> {
        Ok(self
            .state
            .emoji_repo
            .find_by_name(self.workspace_id, name)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .map(|emoji| emoji.storage_key))
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

    /// `channels.json` is public, `groups.json` is private. A missing file is
    /// reported rather than assumed: an export without one is normal, and an
    /// export whose private channels were quietly dropped is not.
    async fn import_channels(
        &mut self,
        source: &mut dyn ExportSource,
        listing: &str,
        channel_type: &ChannelType,
    ) -> AppResult<Vec<(SlackConversation, Target)>> {
        let Some(channels) = self.read_listing(source, listing)? else {
            return Ok(Vec::new());
        };
        let mut targets = Vec::new();

        for channel in channels {
            let slack_name = channel.folder().to_string();
            if let Some(&channel_id) = self.channels.get(&channel.id) {
                self.report.channels_reused += 1;
                self.add_channel_members(&channel, channel_id).await?;
                targets.push((channel, Target::Channel(channel_id)));
                continue;
            }

            let name = sanitise_channel_name(&slack_name);
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
                            channel_type,
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
            self.add_channel_members(&channel, channel_id).await?;
            targets.push((channel, Target::Channel(channel_id)));
        }

        Ok(targets)
    }

    /// `dms.json` and `mpims.json`. These are conversations here rather than
    /// channels, which is the same distinction the product already makes, and it
    /// keeps a two-person history out of everybody's channel list.
    async fn import_conversations(
        &mut self,
        source: &mut dyn ExportSource,
        listing: &str,
    ) -> AppResult<Vec<(SlackConversation, Target)>> {
        let Some(conversations) = self.read_listing(source, listing)? else {
            return Ok(Vec::new());
        };
        let mut targets = Vec::new();

        for conversation in conversations {
            if let Some(&conversation_id) = self.conversations.get(&conversation.id) {
                self.report.conversations_reused += 1;
                targets.push((conversation, Target::Conversation(conversation_id)));
                continue;
            }

            let participants: Vec<Uuid> = conversation
                .members
                .iter()
                .filter_map(|slack_id| self.users.get(slack_id).map(|(id, _)| *id))
                .collect();
            if participants.len() < 2 {
                // One side of the conversation has no account here, so there is
                // nobody to open it between.
                self.report.skip(
                    format!("conversation {}", conversation.id),
                    "fewer than two of its members were imported",
                );
                continue;
            }

            self.report.conversations_created += 1;
            if self.dry_run {
                self.conversations
                    .insert(conversation.id.clone(), Uuid::nil());
                targets.push((conversation, Target::Conversation(Uuid::nil())));
                continue;
            }

            let kind = if participants.len() == 2 {
                ConversationKind::Direct
            } else {
                ConversationKind::Group
            };
            let created = self
                .state
                .conversation_repo
                .create(self.workspace_id, kind, participants[0], &participants)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

            self.state
                .slack_import_repo
                .map_conversation(self.workspace_id, &conversation.id, created.id)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
            self.conversations
                .insert(conversation.id.clone(), created.id);
            targets.push((conversation, Target::Conversation(created.id)));
        }

        Ok(targets)
    }

    /// An export need not carry every listing — a workspace with no private
    /// channels has no `groups.json`, and only some plans export DMs at all.
    fn read_listing(
        &mut self,
        source: &mut dyn ExportSource,
        listing: &str,
    ) -> AppResult<Option<Vec<SlackConversation>>> {
        if !source.has(listing) {
            self.report.skip(listing.to_string(), "not in this export");
            return Ok(None);
        }
        read_json(source, listing).map(Some)
    }

    async fn add_channel_members(
        &mut self,
        channel: &SlackConversation,
        channel_id: Uuid,
    ) -> AppResult<()> {
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
            }
        }

        Ok(())
    }

    async fn already_imported(&self, target: Target) -> AppResult<HashMap<String, Uuid>> {
        if self.dry_run {
            return Ok(HashMap::new());
        }
        let rows = match target {
            Target::Channel(id) => self.state.slack_import_repo.imported_message_ids(id).await,
            Target::Conversation(id) => {
                self.state
                    .slack_import_repo
                    .imported_conversation_message_ids(id)
                    .await
            }
        };
        rows.map(|rows| rows.into_iter().collect())
            .map_err(|e| AppError::Database(e.to_string()))
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

        self.import_reactions(target, stored_id, message).await?;

        if !message.pinned_to.is_empty() {
            if let Target::Channel(_) = target {
                self.state
                    .message_repo
                    .set_pinned(stored_id, true)
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?;
                self.report.pins += 1;
            }
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
        let id = match target {
            Target::Channel(channel_id) => {
                self.state
                    .message_repo
                    .insert_imported(ImportedMessage {
                        channel_id,
                        user_id,
                        content,
                        thread_parent_id,
                        slack_ts,
                        created_at,
                    })
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?
                    .id
            }
            Target::Conversation(conversation_id) => {
                self.state
                    .conversation_repo
                    .insert_imported(
                        conversation_id,
                        user_id,
                        content,
                        thread_parent_id,
                        slack_ts,
                        created_at,
                    )
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?
                    .id
            }
        };
        Ok(id)
    }

    async fn import_reactions(
        &mut self,
        target: Target,
        message_id: Uuid,
        message: &SlackMessage,
    ) -> AppResult<()> {
        for reaction in &message.reactions {
            let emoji = emoji_for(&reaction.name);
            for slack_user_id in &reaction.users {
                let Some(user_id) = self.users.get(slack_user_id).map(|(id, _)| *id) else {
                    continue;
                };
                let added = match target {
                    Target::Channel(_) => self
                        .state
                        .message_repo
                        .add_reaction(message_id, user_id, &emoji)
                        .await
                        .map(|_| ()),
                    Target::Conversation(_) => self
                        .state
                        .conversation_repo
                        .add_reaction(message_id, user_id, &emoji)
                        .await
                        .map(|_| ()),
                };
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

            let body = match self.slack.fetch(url).await {
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
            let attachment_ts = format!("{}-file-{}", message.ts, self.report.files_imported);
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

/// Slack serves emoji as images and names the file; the URL is the only hint at
/// what kind, and png is what the overwhelming majority are.
fn content_type_for(url: &str) -> &'static str {
    let path = url.split('?').next().unwrap_or(url).to_lowercase();
    if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".webp") {
        "image/webp"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "image/png"
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
