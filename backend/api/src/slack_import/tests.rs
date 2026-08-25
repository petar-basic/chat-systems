use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::http_tests::common::*;
use crate::state::AppState;

use super::files::{FileFetcher, SkipFiles};
use super::models::ImportReport;
use super::service::Import;
use super::source::DirectorySource;

/// Stands in for Slack's file host, so an import can be tested without one.
struct StubFiles(Vec<u8>);

#[async_trait]
impl FileFetcher for StubFiles {
    async fn fetch(&self, _url: &str) -> Result<Vec<u8>, String> {
        Ok(self.0.clone())
    }
}

/// A hand-written export covering what actually breaks importers: a thread, a
/// reaction, a pin, a file, a bot, a deleted account with no email, a channel
/// whose name our rules reject, and a join notice nobody wrote.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn write() -> Self {
        let root = std::env::temp_dir().join(format!("slack-export-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("General Chat")).expect("fixture directory");

        let users = json!([
            {
                "id": "U1",
                "name": "ana",
                "profile": { "email": "ana@dev.local", "real_name": "Ana Marić" }
            },
            {
                "id": "U2",
                "name": "ivan",
                "profile": { "email": "ivan@dev.local", "real_name": "Ivan Novak" }
            },
            {
                "id": "U3",
                "name": "ghost",
                "deleted": true,
                "profile": { "real_name": "Nobody" }
            },
            {
                "id": "B1",
                "name": "buildbot",
                "is_bot": true,
                "profile": { "email": "bot@dev.local" }
            }
        ]);
        std::fs::write(
            root.join("users.json"),
            serde_json::to_vec_pretty(&users).expect("users json"),
        )
        .expect("write users");

        let channels = json!([
            {
                "id": "C1",
                "name": "General Chat",
                "members": ["U1", "U2"],
                "topic": { "value": "everything else" },
                "purpose": { "value": "the default channel" }
            }
        ]);
        std::fs::write(
            root.join("channels.json"),
            serde_json::to_vec_pretty(&channels).expect("channels json"),
        )
        .expect("write channels");

        let day = json!([
            {
                "type": "message",
                "subtype": "channel_join",
                "user": "U1",
                "text": "<@U1> has joined the channel",
                "ts": "1700000000.000100"
            },
            {
                "type": "message",
                "user": "U1",
                "text": "*deploy* is out, see <https://example.com|the notes> <@U2>",
                "ts": "1700000100.000100",
                "reactions": [{ "name": "tada", "users": ["U2"] }],
                "pinned_to": ["C1"]
            },
            {
                "type": "message",
                "user": "U2",
                "text": "on it",
                "ts": "1700000200.000100",
                "thread_ts": "1700000100.000100"
            },
            {
                "type": "message",
                "user": "U2",
                "text": "here is the log",
                "ts": "1700000300.000100",
                "files": [{
                    "name": "deploy.log",
                    "mimetype": "text/plain",
                    "url_private_download": "https://files.slack.test/deploy.log"
                }]
            },
            {
                "type": "message",
                "user": "U3",
                "text": "from an account with no address",
                "ts": "1700000400.000100"
            }
        ]);
        std::fs::write(
            root.join("General Chat").join("2023-11-14.json"),
            serde_json::to_vec_pretty(&day).expect("day json"),
        )
        .expect("write day");

        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

async fn import(state: &Arc<AppState>, ws: Uuid, root: &PathBuf, dry_run: bool) -> ImportReport {
    let files = StubFiles(b"deploy log body".to_vec());
    let mut source = DirectorySource::new(root);
    Import::new(state, &files, ws, dry_run)
        .run(&mut source)
        .await
        .expect("import runs")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_export_arrives_as_channels_messages_threads_reactions_and_pins(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;
    // The account that already exists is matched, not duplicated.
    let existing = seed_user(&state, "ana@dev.local", false).await;

    let fixture = Fixture::write();
    let report = import(&state, ws, &fixture.root, false).await;

    assert_eq!(report.users_matched, 1, "ana already had an account");
    assert_eq!(report.users_created, 1, "ivan did not");
    assert_eq!(report.channels_created, 1);
    assert_eq!(
        report.messages_imported, 3,
        "the join notice is not a message"
    );
    assert_eq!(report.threads_resolved, 1);
    assert_eq!(report.reactions, 1);
    assert_eq!(report.pins, 1);
    assert_eq!(report.files_imported, 1);

    let channel_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM channels WHERE workspace_id = $1 AND name = 'general-chat'",
    )
    .bind(ws)
    .fetch_one(&state.pool)
    .await
    .expect("the channel was renamed to something the rules allow");

    let messages = state
        .message_repo
        .list_channel_messages(channel_id, 50, None)
        .await
        .expect("messages");
    let root_message = messages
        .iter()
        .find(|m| m.content.starts_with("**deploy**"))
        .expect("the announcement is there");

    assert!(
        root_message
            .content
            .contains("[the notes](https://example.com)"),
        "the link keeps its label: {}",
        root_message.content
    );
    assert!(
        root_message.content.contains("@[Ivan Novak]("),
        "a mention resolves to the imported account: {}",
        root_message.content
    );
    assert_eq!(root_message.user_id, existing, "authorship is preserved");
    assert!(root_message.is_pinned);
    assert_eq!(root_message.reply_count, 1);
    assert_eq!(
        root_message.created_at.timestamp(),
        1_700_000_100,
        "the message keeps the moment it was written"
    );

    assert!(
        !messages.iter().any(|m| m.content.contains("has joined")),
        "a join notice is Slack's record of an event, not something anybody wrote"
    );

    let reactions = state
        .message_repo
        .list_reactions(root_message.id)
        .await
        .expect("reactions");
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "🎉");

    let attachment: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM files WHERE workspace_id = $1 AND filename = 'deploy.log'",
    )
    .bind(ws)
    .fetch_one(&state.pool)
    .await
    .expect("count files");
    assert_eq!(attachment, 1, "the attachment was fetched and stored");

    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.why.contains("no email in the export")),
        "the deleted account is reported, not silently dropped: {:?}",
        report.skipped
    );
    assert!(
        report.skipped.iter().any(|s| s.why.contains("a bot")),
        "so is the bot"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn running_it_twice_writes_nothing_the_second_time(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-twice", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();
    let first = import(&state, ws, &fixture.root, false).await;

    let count_messages = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM messages m JOIN channels c ON c.id = m.channel_id WHERE c.workspace_id = $1",
        )
        .bind(ws)
        .fetch_one(&state.pool)
        .await
        .expect("count messages")
    };
    let after_first = count_messages().await;

    let second = import(&state, ws, &fixture.root, false).await;

    assert_eq!(
        count_messages().await,
        after_first,
        "a re-run adds nothing: {} then {}",
        first.messages_imported,
        second.messages_imported
    );
    assert_eq!(second.messages_imported, 0);
    assert_eq!(second.messages_already_present, first.messages_imported);
    assert_eq!(
        second.users_created, 0,
        "the mapping survived the first run"
    );
    assert_eq!(second.channels_created, 0);

    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM slack_users WHERE workspace_id = $1")
        .bind(ws)
        .fetch_one(&state.pool)
        .await
        .expect("count mappings");
    assert_eq!(users, 2);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_interrupted_import_picks_up_where_it_stopped(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-resume", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();

    // Stands in for a run killed after the first pass: users and the channel are
    // mapped, no messages are in yet.
    let files = SkipFiles;
    let mut source = DirectorySource::new(&fixture.root);
    let partial = Import::new(&state, &files, ws, false);
    partial.run(&mut source).await.expect("first pass");
    sqlx::query("DELETE FROM messages WHERE channel_id IN (SELECT id FROM channels WHERE workspace_id = $1)")
        .bind(ws)
        .execute(&state.pool)
        .await
        .expect("drop what the interrupted run had written");

    let resumed = import(&state, ws, &fixture.root, false).await;

    assert_eq!(resumed.users_created, 0, "the mapping is reused");
    assert_eq!(resumed.channels_created, 0);
    assert_eq!(resumed.messages_imported, 3, "the messages are written now");
    assert_eq!(resumed.threads_resolved, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_dry_run_reports_and_writes_nothing(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-dry", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();
    let report = import(&state, ws, &fixture.root, true).await;

    assert_eq!(report.messages_imported, 3, "it says what it would do");
    assert_eq!(report.channels_created, 1);

    let imported: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM channels WHERE workspace_id = $1 AND name = 'general-chat'",
    )
    .bind(ws)
    .fetch_one(&state.pool)
    .await
    .expect("count channels");
    assert_eq!(imported, 0, "and writes none of it");

    let runs: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM slack_imports WHERE workspace_id = $1")
            .bind(ws)
            .fetch_one(&state.pool)
            .await
            .expect("count runs");
    assert_eq!(runs, 0, "not even a run record");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_unfetchable_file_is_reported_and_the_message_still_lands(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-nofiles", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();
    let files = SkipFiles;
    let mut source = DirectorySource::new(&fixture.root);
    let report = Import::new(&state, &files, ws, false)
        .run(&mut source)
        .await
        .expect("import runs");

    assert_eq!(report.files_imported, 0);
    assert_eq!(
        report.messages_imported, 3,
        "the message it hung off is kept"
    );
    assert!(
        report.skipped.iter().any(|s| s.what.contains("deploy.log")),
        "the operator is told which file: {:?}",
        report.skipped
    );
}
