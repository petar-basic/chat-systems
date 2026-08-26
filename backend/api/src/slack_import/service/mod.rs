use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::state::AppState;
use crate::workspace::models::ChannelType;

use super::files::SlackClient;
use super::models::*;
use super::source::ExportSource;

mod channels;
mod emoji;
mod messages;
mod users;

/// Where a Slack conversation landed. Public and private channels are channels;
/// DMs and group DMs are conversations, which is the same split this product
/// makes natively.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Target {
    Channel(Uuid),
    Conversation(Uuid),
}

/// Users are matched by email, and by nothing else. A Slack handle is not an
/// identity: two people can carry the same one across workspaces, and matching
/// on it would attribute somebody's history to a stranger.
pub struct Import<'a> {
    pub(crate) state: &'a Arc<AppState>,
    pub(crate) slack: &'a dyn SlackClient,
    pub(crate) workspace_id: Uuid,
    /// Imported channels are created by the workspace owner: Slack's creator may
    /// have no account here, and `channels.created_by` has to point at somebody.
    pub(crate) owner_id: Uuid,
    pub(crate) dry_run: bool,
    /// Slack user id → (our user id, the name to render in a mention).
    pub(crate) users: HashMap<String, (Uuid, String)>,
    pub(crate) channels: HashMap<String, Uuid>,
    pub(crate) conversations: HashMap<String, Uuid>,
    pub(crate) report: ImportReport,
}

impl<'a> Import<'a> {
    /// Fallible and async because the workspace has to exist before there is an
    /// import to speak of: the alternative is a struct that is invalid between
    /// construction and the first line of `run`.
    pub async fn open(
        state: &'a Arc<AppState>,
        slack: &'a dyn SlackClient,
        workspace_id: Uuid,
        dry_run: bool,
    ) -> AppResult<Self> {
        let owner_id = state
            .workspace_service
            .repo
            .find_workspace_by_id(workspace_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("No such workspace".into()))?
            .owner_id;

        Ok(Self {
            state,
            slack,
            workspace_id,
            owner_id,
            dry_run,
            users: HashMap::new(),
            channels: HashMap::new(),
            conversations: HashMap::new(),
            report: ImportReport::default(),
        })
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
}

/// `slack_ts` is a `VARCHAR(32)`, and an attachment's key is the message's plus
/// a suffix. Building it to fit is cheaper than discovering the limit from a
/// failed insert halfway through somebody's migration.
pub fn attachment_slack_ts(message_ts: &str, index: usize) -> String {
    const LIMIT: usize = 32;
    let suffix = format!("-f{index}");
    let room = LIMIT.saturating_sub(suffix.len());
    let base: String = message_ts.chars().take(room).collect();
    format!("{base}{suffix}")
}

/// A filename from an export is somebody else's string. The storage key is built
/// from it, and while `LocalStorage` refuses traversal on its own, a key should
/// not depend on another module's guard to be a single path segment.
pub fn storage_safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .filter(|c| !c.is_whitespace() || *c == ' ')
        .collect();
    let trimmed = cleaned.trim();
    // `.` and `..` are the two names that mean something to a filesystem rather
    // than to a person; every other arrangement of dots is somebody's filename.
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return "attachment".to_string();
    }
    trimmed.chars().take(120).collect()
}

/// Stands in for a storage key during a dry run, where nothing is uploaded but
/// the alias pass still has to know which names were seen.
const DRY_RUN_KEY: &str = "";

/// What a chat attachment ever is, and what an import is allowed to pull into
/// this instance's storage in one go.
const MAX_ATTACHMENT_BYTES: usize = 100 * 1024 * 1024;

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
    if !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // Padded to microseconds by character, not by byte: a tampered export should
    // come back as "unreadable", not as a panic on a character boundary.
    let micros: u32 = fraction
        .chars()
        .chain(std::iter::repeat('0'))
        .take(6)
        .collect::<String>()
        .parse()
        .ok()?;
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
    fn an_attachment_key_fits_the_column_it_goes_in() {
        assert_eq!(
            attachment_slack_ts("1700000100.000100", 3),
            "1700000100.000100-f3"
        );
        for index in [0, 9, 99_999] {
            let key = attachment_slack_ts("1700000100.000100", index);
            assert!(key.len() <= 32, "{key} is {} characters", key.len());
        }
        let long = attachment_slack_ts(&"9".repeat(40), 7);
        assert!(long.len() <= 32, "even a ts nobody should send fits");
        assert!(long.ends_with("-f7"), "and keeps what makes it unique");
    }

    #[test]
    fn a_filename_becomes_one_path_segment() {
        assert_eq!(storage_safe_filename("deploy.log"), "deploy.log");
        assert_eq!(
            storage_safe_filename("../../etc/passwd"),
            "..-..-etc-passwd"
        );
        assert_eq!(storage_safe_filename("a\\b.png"), "a-b.png");
        assert_eq!(storage_safe_filename("   "), "attachment");
        assert_eq!(storage_safe_filename(".."), "attachment");
        assert_eq!(
            storage_safe_filename(".hidden.png"),
            ".hidden.png",
            "a leading dot is a filename, not an escape"
        );
    }

    #[test]
    fn an_unreadable_timestamp_is_none_rather_than_a_panic() {
        assert!(parse_ts("1700000000.00Đ100").is_none());
        assert!(parse_ts("1700000000.").is_some());
    }

    #[test]
    fn a_known_shortcode_becomes_the_character_and_the_rest_stays_readable() {
        assert_eq!(emoji_for("tada"), "🎉");
        assert_eq!(emoji_for("+1::skin-tone-3"), "👍");
        assert_eq!(emoji_for("shipit"), ":shipit:");
    }
}
