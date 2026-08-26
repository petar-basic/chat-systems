use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::http_tests::common::*;
use crate::state::AppState;

use super::files::{OfflineSlack, SlackClient};
use super::models::ImportReport;
use super::service::Import;
use super::source::{DirectorySource, ZipSource};

/// Stands in for Slack, so an import can be tested without one.
struct StubSlack {
    body: Vec<u8>,
    emoji: HashMap<String, String>,
}

impl StubSlack {
    fn new() -> Self {
        Self {
            body: b"deploy log body".to_vec(),
            emoji: HashMap::new(),
        }
    }

    fn with_emoji(emoji: &[(&str, &str)]) -> Self {
        Self {
            emoji: emoji
                .iter()
                .map(|(name, url)| ((*name).to_string(), (*url).to_string()))
                .collect(),
            ..Self::new()
        }
    }
}

#[async_trait]
impl SlackClient for StubSlack {
    async fn fetch(&self, _url: &str) -> Result<Vec<u8>, String> {
        Ok(self.body.clone())
    }

    async fn custom_emoji(&self) -> Result<HashMap<String, String>, String> {
        Ok(self.emoji.clone())
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
        // Slack's own folder names: the channel name for a channel, the
        // conversation id for a DM.
        std::fs::create_dir_all(root.join("general-chat")).expect("fixture directory");
        std::fs::create_dir_all(root.join("incident-2023")).expect("private directory");
        std::fs::create_dir_all(root.join("D01ANAIVAN")).expect("dm directory");

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
                "name": "general-chat",
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

        // Private channels live in their own listing, and an export that has
        // them and an export that does not are both normal.
        let groups = json!([
            {
                "id": "G1",
                "name": "incident-2023",
                "members": ["U1", "U2"],
                "topic": { "value": "" },
                "purpose": { "value": "the bad week" }
            }
        ]);
        std::fs::write(
            root.join("groups.json"),
            serde_json::to_vec_pretty(&groups).expect("groups json"),
        )
        .expect("write groups");

        let dms = json!([{ "id": "D01ANAIVAN", "members": ["U1", "U2"] }]);
        std::fs::write(
            root.join("dms.json"),
            serde_json::to_vec_pretty(&dms).expect("dms json"),
        )
        .expect("write dms");

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
            },
            {
                "type": "message",
                "subtype": "bot_message",
                "bot_id": "B1",
                "username": "buildbot",
                "text": "build 412 passed",
                "ts": "1700000500.000100"
            },
            {
                "type": "message",
                "user": "U1",
                "text": "the screenshot is gone",
                "ts": "1700000600.000100",
                "files": [{
                    "id": "F9",
                    "name": "screenshot.png",
                    "mimetype": "image/png",
                    "mode": "tombstone",
                    "is_tombstoned": true
                }]
            }
        ]);
        std::fs::write(
            root.join("general-chat").join("2023-11-14.json"),
            serde_json::to_vec_pretty(&day).expect("day json"),
        )
        .expect("write day");

        let private_day = json!([
            {
                "type": "message",
                "user": "U2",
                "text": "postmortem is drafted",
                "ts": "1700100000.000100"
            }
        ]);
        std::fs::write(
            root.join("incident-2023").join("2023-11-15.json"),
            serde_json::to_vec_pretty(&private_day).expect("private day json"),
        )
        .expect("write private day");

        let dm_day = json!([
            {
                "type": "message",
                "user": "U1",
                "text": "can you take the on-call swap?",
                "ts": "1700200000.000100"
            },
            {
                "type": "message",
                "user": "U2",
                "text": "yes",
                "ts": "1700200100.000100"
            }
        ]);
        std::fs::write(
            root.join("D01ANAIVAN").join("2023-11-16.json"),
            serde_json::to_vec_pretty(&dm_day).expect("dm day json"),
        )
        .expect("write dm day");

        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

async fn import(state: &Arc<AppState>, ws: Uuid, root: &PathBuf, dry_run: bool) -> ImportReport {
    let slack = StubSlack::new();
    let mut source = DirectorySource::new(root);
    Import::new(state, &slack, ws, dry_run)
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
    assert_eq!(
        report.channels_created, 2,
        "the private channel comes from groups.json"
    );
    assert_eq!(report.conversations_created, 1, "the DM is a conversation");
    assert_eq!(
        report.messages_imported, 7,
        "four in the channel, one private, two in the DM — the join notice is not a message"
    );
    assert_eq!(report.threads_resolved, 1);
    assert_eq!(report.reactions, 1);
    assert_eq!(report.pins, 1);
    assert_eq!(report.files_imported, 1);

    let private_type: String = sqlx::query_scalar(
        "SELECT channel_type::text FROM channels WHERE workspace_id = $1 AND name = 'incident-2023'",
    )
    .bind(ws)
    .fetch_one(&state.pool)
    .await
    .expect("the private channel was imported");
    assert_eq!(private_type, "private", "and stayed private");

    let dm_messages: Vec<String> = sqlx::query_scalar(
        r"SELECT cm.content FROM conversation_messages cm
          JOIN conversations c ON c.id = cm.conversation_id
          WHERE c.workspace_id = $1 ORDER BY cm.created_at",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await
    .expect("dm messages");
    assert_eq!(
        dm_messages,
        vec!["can you take the on-call swap?", "yes"],
        "the DM landed as a conversation, not as a channel everybody can see"
    );

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

    // An imported account is one nobody can sign into until they claim it
    // through the invite flow.
    let imported_status: String = sqlx::query_scalar(
        "SELECT status::text FROM users WHERE email = 'ivan@dev.local' AND password_hash IS NULL",
    )
    .fetch_one(&state.pool)
    .await
    .expect("the imported account exists");
    assert_eq!(imported_status, "pending");

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
    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.why.contains("posted by an integration")),
        "and the message that bot posted, with the reason it has no author: {:?}",
        report.skipped
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.what.contains("screenshot.png") && s.why.contains("deleted in Slack")),
        "a tombstoned file says what happened to it rather than failing to download: {:?}",
        report.skipped
    );
}

/// The documented input is a ZIP, so the ZIP path is what the acceptance test
/// should exercise; a directory is the convenience.
fn zip_of(root: &PathBuf) -> PathBuf {
    let target = root.with_extension("zip");
    let file = std::fs::File::create(&target).expect("create the zip");
    let mut writer = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read the fixture") {
            let entry = entry.expect("entry");
            let path = entry.path();
            let name = path
                .strip_prefix(root)
                .expect("inside the fixture")
                .to_string_lossy()
                .to_string();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            writer.start_file(name, options).expect("start the entry");
            let bytes = std::fs::read(&path).expect("read the entry");
            std::io::Write::write_all(&mut writer, &bytes).expect("write the entry");
        }
    }
    writer.finish().expect("finish the zip");
    target
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_zip_slack_hands_you_imports_the_same_as_a_directory(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-zip", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();
    let archive = zip_of(&fixture.root);

    let slack = StubSlack::new();
    let mut source = ZipSource::open(&archive).expect("open the zip");
    let report = Import::new(&state, &slack, ws, false)
        .run(&mut source)
        .await
        .expect("import runs");
    let _ = std::fs::remove_file(&archive);

    assert_eq!(report.channels_created, 2);
    assert_eq!(report.conversations_created, 1);
    assert_eq!(report.messages_imported, 7);
    assert_eq!(report.threads_resolved, 1);
    assert_eq!(report.files_imported, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_export_without_private_channels_or_dms_says_so(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-partial", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();
    for listing in ["groups.json", "dms.json"] {
        std::fs::remove_file(fixture.root.join(listing)).expect("remove the listing");
    }

    let report = import(&state, ws, &fixture.root, false).await;

    assert_eq!(report.channels_created, 1, "only the public channel");
    assert_eq!(report.conversations_created, 0);
    for listing in ["groups.json", "dms.json", "mpims.json"] {
        assert!(
            report
                .skipped
                .iter()
                .any(|s| s.what == listing && s.why.contains("not in this export")),
            "{listing} is reported as absent rather than assumed empty: {:?}",
            report.skipped
        );
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_imported_attachment_is_as_private_as_a_native_one(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "import-acl", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;
    let (outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "import-outsider", false).await;

    let fixture = Fixture::write();
    import(&state, ws, &fixture.root, false).await;

    let key: String = sqlx::query_scalar(
        "SELECT storage_key FROM files WHERE workspace_id = $1 AND filename = 'deploy.log'",
    )
    .bind(ws)
    .fetch_one(&state.pool)
    .await
    .expect("the imported attachment");

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/files/download/{key}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a member of the channel can read it"
    );

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/files/download/{key}"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_ne!(
        status,
        StatusCode::OK,
        "somebody outside the workspace cannot, exactly as for a native upload"
    );
    let _ = outsider_id;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn custom_emoji_come_from_the_api_because_the_export_has_none(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-emoji", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();
    let slack = StubSlack::with_emoji(&[
        ("shipit", "https://emoji.slack.test/shipit/1.png"),
        ("deployed", "alias:shipit"),
        ("orphan", "alias:nothing-here"),
        // Slack allows names ours does not, and one of ours is already taken by
        // a standard emoji.
        ("Party Parrot", "https://emoji.slack.test/parrot.gif"),
        ("tada", "https://emoji.slack.test/tada.png"),
    ]);
    let mut source = DirectorySource::new(&fixture.root);
    let report = Import::new(&state, &slack, ws, false)
        .run(&mut source)
        .await
        .expect("import runs");

    let emoji = state.emoji_repo.list(ws).await.expect("emoji");
    let names: Vec<&str> = emoji.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["deployed", "shipit"],
        "the alias came across, the invalid name and the standard one did not"
    );
    assert_eq!(report.emoji_imported, 2);

    let shipit = emoji.iter().find(|e| e.name == "shipit").expect("shipit");
    let deployed = emoji.iter().find(|e| e.name == "deployed").expect("alias");
    assert_eq!(
        shipit.storage_key, deployed.storage_key,
        "an alias points at the same image rather than downloading it twice"
    );

    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.what.contains("orphan") && s.why.contains("was not imported")),
        "an alias of an emoji nobody exported says so: {:?}",
        report.skipped
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.what.contains("Party Parrot")),
        "and so does a name our rules reject: {:?}",
        report.skipped
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_emoji_json_in_the_export_is_used_instead_of_the_api(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-emoji-file", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();
    std::fs::write(
        fixture.root.join("emoji.json"),
        serde_json::to_vec(&json!({ "from-the-export": "https://emoji.slack.test/x.png" }))
            .expect("emoji json"),
    )
    .expect("write emoji.json");

    // The client would answer with something else; the export wins.
    let slack = StubSlack::with_emoji(&[("from-the-api", "https://emoji.slack.test/y.png")]);
    let mut source = DirectorySource::new(&fixture.root);
    Import::new(&state, &slack, ws, false)
        .run(&mut source)
        .await
        .expect("import runs");

    let names: Vec<String> = state
        .emoji_repo
        .list(ws)
        .await
        .expect("emoji")
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, vec!["from-the-export"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn without_a_token_the_run_says_the_emoji_were_left_behind(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-emoji-none", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();
    let slack = OfflineSlack;
    let mut source = DirectorySource::new(&fixture.root);
    let report = Import::new(&state, &slack, ws, false)
        .run(&mut source)
        .await
        .expect("import runs");

    assert_eq!(report.emoji_imported, 0);
    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.what == "custom emoji" && s.why.contains("not fetched")),
        "the operator is told, rather than finding out from a message full of :shortcodes:: {:?}",
        report.skipped
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

    let conversations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE workspace_id = $1")
            .bind(ws)
            .fetch_one(&state.pool)
            .await
            .expect("count conversations");
    assert_eq!(conversations, 1, "the DM was not opened a second time");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_interrupted_import_picks_up_where_it_stopped(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-resume", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();

    // Stands in for a run killed after the first pass: users and the channel are
    // mapped, no messages are in yet.
    let slack = OfflineSlack;
    let mut source = DirectorySource::new(&fixture.root);
    let partial = Import::new(&state, &slack, ws, false);
    partial.run(&mut source).await.expect("first pass");
    sqlx::query("DELETE FROM messages WHERE channel_id IN (SELECT id FROM channels WHERE workspace_id = $1)")
        .bind(ws)
        .execute(&state.pool)
        .await
        .expect("drop what the interrupted run had written");
    sqlx::query(
        "DELETE FROM conversation_messages WHERE conversation_id IN (SELECT id FROM conversations WHERE workspace_id = $1)",
    )
    .bind(ws)
    .execute(&state.pool)
    .await
    .expect("and what it had written in conversations");

    let resumed = import(&state, ws, &fixture.root, false).await;

    assert_eq!(resumed.users_created, 0, "the mapping is reused");
    assert_eq!(resumed.channels_created, 0);
    assert_eq!(resumed.messages_imported, 7, "the messages are written now");
    assert_eq!(resumed.threads_resolved, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_dry_run_reports_and_writes_nothing(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "import-dry", false).await;
    let ws = seed_workspace(&state, owner_id, "Imported").await;

    let fixture = Fixture::write();
    let report = import(&state, ws, &fixture.root, true).await;

    assert_eq!(report.messages_imported, 7, "it says what it would do");
    assert_eq!(report.channels_created, 2);
    assert_eq!(report.conversations_created, 1);

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
    let slack = OfflineSlack;
    let mut source = DirectorySource::new(&fixture.root);
    let report = Import::new(&state, &slack, ws, false)
        .run(&mut source)
        .await
        .expect("import runs");

    assert_eq!(report.files_imported, 0);
    assert_eq!(
        report.messages_imported, 7,
        "the message it hung off is kept"
    );
    assert!(
        report.skipped.iter().any(|s| s.what.contains("deploy.log")),
        "the operator is told which file: {:?}",
        report.skipped
    );
}
