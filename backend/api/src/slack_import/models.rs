use serde::{Deserialize, Serialize};

/// The subset of a Slack export this import reads. Everything else in those
/// files is ignored on purpose: an export carries a great deal that has no home
/// here, and silently dropping it is better than half-translating it.
#[derive(Debug, Clone, Deserialize)]
pub struct SlackUser {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub is_bot: bool,
    #[serde(default)]
    pub profile: SlackProfile,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SlackProfile {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

impl SlackUser {
    pub fn display_name(&self) -> String {
        self.profile
            .real_name
            .as_deref()
            .or(self.profile.display_name.as_deref())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(&self.name)
            .to_string()
    }
}

/// `channels.json`, `groups.json`, `mpims.json` and `dms.json` all carry the
/// same shape; only DMs have no name, and their folder is the conversation id.
#[derive(Debug, Clone, Deserialize)]
pub struct SlackConversation {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub is_archived: bool,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub topic: SlackText,
    #[serde(default)]
    pub purpose: SlackText,
}

impl SlackConversation {
    /// The directory its day files are in: the channel name where there is one,
    /// the conversation id for a DM.
    pub fn folder(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SlackText {
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackMessage {
    #[serde(default)]
    pub subtype: Option<String>,
    /// Present on a message an integration posted, where `user` is absent.
    #[serde(default)]
    pub bot_id: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub text: String,
    pub ts: String,
    #[serde(default)]
    pub thread_ts: Option<String>,
    #[serde(default)]
    pub reactions: Vec<SlackReaction>,
    #[serde(default)]
    pub files: Vec<SlackFile>,
    #[serde(default)]
    pub pinned_to: Vec<String>,
}

impl SlackMessage {
    /// A reply carries the parent's `ts` in `thread_ts`; the parent carries its
    /// own, which is not a thread relationship.
    pub fn parent_ts(&self) -> Option<&str> {
        self.thread_ts
            .as_deref()
            .filter(|parent| *parent != self.ts)
    }

    /// Joins, leaves, channel renames and the rest are Slack's own record of
    /// what happened to the channel, not something anybody wrote.
    pub fn is_system_event(&self) -> bool {
        self.subtype.as_deref().is_some_and(|subtype| {
            subtype.starts_with("channel_")
                || subtype.starts_with("group_")
                || subtype == "bot_add"
                || subtype == "bot_remove"
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackReaction {
    pub name: String,
    #[serde(default)]
    pub users: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackFile {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mimetype: Option<String>,
    #[serde(default)]
    pub url_private_download: Option<String>,
    #[serde(default)]
    pub url_private: Option<String>,
    /// "hosted", "external", "snippet", "post" — an external file's bytes were
    /// never Slack's to give.
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub is_deleted: bool,
    #[serde(default)]
    pub is_tombstoned: bool,
}

impl SlackFile {
    /// Why there are no bytes to fetch, when there are none.
    pub fn unavailable(&self) -> Option<&'static str> {
        if self.is_deleted || self.is_tombstoned {
            return Some("the file was deleted in Slack");
        }
        if self.mode.as_deref() == Some("external") {
            return Some("the file is hosted outside Slack");
        }
        self.download_url()
            .is_none()
            .then_some("the export carries no download url")
    }

    pub fn download_url(&self) -> Option<&str> {
        self.url_private_download
            .as_deref()
            .or(self.url_private.as_deref())
    }

    pub fn filename(&self) -> &str {
        self.name.as_deref().unwrap_or("attachment")
    }
}

/// What the run did, and what it could not do. The second half is the point:
/// an import that silently drops a tenth of its input looks identical to one
/// that worked.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub users_matched: usize,
    pub users_created: usize,
    pub channels_created: usize,
    pub channels_reused: usize,
    pub memberships: usize,
    pub conversations_created: usize,
    pub conversations_reused: usize,
    pub messages_imported: usize,
    pub messages_already_present: usize,
    pub threads_resolved: usize,
    pub reactions: usize,
    pub pins: usize,
    pub files_imported: usize,
    pub emoji_imported: usize,
    pub emoji_already_present: usize,
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skipped {
    pub what: String,
    pub why: String,
}

impl ImportReport {
    pub fn skip(&mut self, what: impl Into<String>, why: impl Into<String>) {
        self.skipped.push(Skipped {
            what: what.into(),
            why: why.into(),
        });
    }

    pub fn summary(&self) -> String {
        format!(
            "users: {} matched, {} created | channels: {} created, {} reused | \
             conversations: {} created, {} reused | memberships: {} | \
             messages: {} imported, {} already present, {} in threads | \
             reactions: {} | pins: {} | files: {} | emoji: {} imported, {} already present | \
             skipped: {}",
            self.users_matched,
            self.users_created,
            self.channels_created,
            self.channels_reused,
            self.conversations_created,
            self.conversations_reused,
            self.memberships,
            self.messages_imported,
            self.messages_already_present,
            self.threads_resolved,
            self.reactions,
            self.pins,
            self.files_imported,
            self.emoji_imported,
            self.emoji_already_present,
            self.skipped.len(),
        )
    }
}
