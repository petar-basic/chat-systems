//! Channels from `channels.json` and `groups.json`, conversations from `dms.json` and `mpims.json`.

use shared_common::errors::AppResult;
use uuid::Uuid;

use super::super::models::*;
use super::super::source::{read_json, ExportSource};
use super::sanitise_channel_name;
use super::{Import, Target};
use crate::conversations::models::ConversationKind;
use crate::workspace::models::{ChannelRole, ChannelType};

impl Import<'_> {
    /// `channels.json` is public, `groups.json` is private. A missing file is
    /// reported rather than assumed: an export without one is normal, and an
    /// export whose private channels were quietly dropped is not.
    pub(crate) async fn import_channels(
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
                .await?;

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
                        .await?;
                    if !channel.topic.value.is_empty() {
                        self.state
                            .workspace_service
                            .repo
                            .update_channel(created.id, None, Some(&channel.topic.value), None)
                            .await?;
                    }
                    self.report.channels_created += 1;
                    created.id
                }
            };

            if !self.dry_run {
                self.state
                    .slack_import_repo
                    .map_channel(self.workspace_id, &channel.id, channel_id)
                    .await?;
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
    pub(crate) async fn import_conversations(
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
                .await?;

            self.state
                .slack_import_repo
                .map_conversation(self.workspace_id, &conversation.id, created.id)
                .await?;
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
        let listed: Vec<SlackConversation> = read_json(source, listing)?;
        // Present and empty is a different thing from absent, and the difference
        // is what somebody reading the report before a real run wants to know.
        if listed.is_empty() {
            self.report
                .note(format!("{listing} is in the export, and it is empty"));
        }
        Ok(Some(listed))
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
                .await?;
        }

        Ok(())
    }
}
