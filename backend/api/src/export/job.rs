use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};
use uuid::Uuid;

use crate::audit::{self, AuditAction, AuditEntry};
use crate::lists;
use crate::state::AppState;

use super::repo::{ExportJob, ExportScope};
use super::writer::ExportArchive;

const POLL: Duration = Duration::from_secs(10);
/// Long enough to fetch, short enough that a link pasted into a chat is dead
/// before anyone else gets to it. It is single-use besides.
const DOWNLOAD_TTL_HOURS: i64 = 24;

pub async fn start_export_worker(state: Arc<AppState>) {
    info!("Export worker started");

    loop {
        match state.export_repo.claim_next().await {
            Ok(Some(job)) => {
                let id = job.id;
                if let Err(e) = run(&state, job).await {
                    warn!(export_id = %id, "Export failed: {}", e);
                    let _ = state.export_repo.fail(id, &e).await;
                }
            }
            Ok(None) => tokio::time::sleep(POLL).await,
            Err(e) => {
                warn!("Export worker: failed to claim a job: {}", e);
                tokio::time::sleep(POLL).await;
            }
        }
    }
}

#[cfg(test)]
pub(crate) async fn run_for_test(state: &AppState, job: ExportJob) -> Result<(), String> {
    run(state, job).await
}

async fn run(state: &AppState, job: ExportJob) -> Result<(), String> {
    let mut archive = ExportArchive::new();

    match job.scope {
        ExportScope::Workspace => {
            let workspace_id = job
                .workspace_id
                .ok_or("workspace export without a workspace")?;
            export_workspace(state, &job, workspace_id, &mut archive).await?;
        }
        ExportScope::User => {
            let user_id = job.subject_user_id.ok_or("user export without a subject")?;
            export_user(state, user_id, &mut archive).await?;
        }
    }

    let files = archive.manifest_files();
    let manifest = serde_json::json!({
        "scope": job.scope,
        "workspace_id": job.workspace_id,
        "subject_user_id": job.subject_user_id,
        "requested_by": job.requested_by,
        "include_dms": job.include_dms,
        "since": job.since,
        "until": job.until,
        "generated_at": chrono::Utc::now(),
        "files": files,
    });

    let tar = archive.into_tar(&manifest);
    let storage_key = format!("exports/{}.tar", job.id);

    let mut sink = state
        .file_storage
        .begin_upload(&storage_key, "application/x-tar")
        .await
        .map_err(|e| e.to_string())?;
    sink.write_chunk(axum::body::Bytes::from(tar))
        .await
        .map_err(|e| e.to_string())?;
    sink.finish().await.map_err(|e| e.to_string())?;

    let token = super::routes::generate_download_token();
    state
        .export_repo
        .complete(job.id, &storage_key, &manifest, &token, DOWNLOAD_TTL_HOURS)
        .await
        .map_err(|e| e.to_string())?;

    let mut entry = AuditEntry::new(AuditAction::ExportCompleted, job.requested_by)
        .resource(job.id)
        .details(serde_json::json!({
            "scope": job.scope,
            "include_dms": job.include_dms,
            "files": manifest.get("files"),
        }));
    if let Some(ws) = job.workspace_id {
        entry = entry.workspace(ws);
    }
    audit::record(state, entry).await;

    info!(export_id = %job.id, "Export complete");
    Ok(())
}

async fn export_workspace(
    state: &AppState,
    job: &ExportJob,
    workspace_id: Uuid,
    archive: &mut ExportArchive,
) -> Result<(), String> {
    for file in [
        "channels.jsonl",
        "messages.jsonl",
        "message_edits.jsonl",
        "reactions.jsonl",
        "members.jsonl",
        "files.jsonl",
        "polls.jsonl",
        "poll_votes.jsonl",
        "lists.jsonl",
        "audit_log.jsonl",
        "users.jsonl",
    ] {
        archive.declare(file);
    }
    let mut people = BTreeSet::new();

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"
        SELECT to_jsonb(c) AS "row!" FROM channels c
         WHERE c.workspace_id = $1
           AND ($2::bool OR c.channel_type NOT IN ('dm', 'group_dm'))
        "#,
        workspace_id,
        job.include_dms
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        write(archive, &mut people, "channels.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"
        SELECT to_jsonb(m) AS "row!" FROM messages m
          JOIN channels c ON c.id = m.channel_id
         WHERE c.workspace_id = $1
           AND ($2::timestamptz IS NULL OR m.created_at >= $2)
           AND ($3::timestamptz IS NULL OR m.created_at <= $3)
           AND ($4::bool OR c.channel_type NOT IN ('dm', 'group_dm'))
         ORDER BY m.created_at
        "#,
        workspace_id,
        job.since,
        job.until,
        job.include_dms
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        write(archive, &mut people, "messages.jsonl", &row);
    }

    // An export without edit history answers "what does it say now", not "what
    // happened" — which is the question being asked.
    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"
        SELECT to_jsonb(e) AS "row!" FROM message_edits e
          JOIN messages m ON m.id = e.message_id
          JOIN channels c ON c.id = m.channel_id
         WHERE c.workspace_id = $1
           AND ($2::bool OR c.channel_type NOT IN ('dm', 'group_dm'))
           AND ($3::timestamptz IS NULL OR m.created_at >= $3)
           AND ($4::timestamptz IS NULL OR m.created_at <= $4)
         ORDER BY e.edited_at
        "#,
        workspace_id,
        job.include_dms,
        job.since,
        job.until
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        write(archive, &mut people, "message_edits.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"
        SELECT to_jsonb(r) AS "row!" FROM reactions r
          JOIN messages m ON m.id = r.message_id
          JOIN channels c ON c.id = m.channel_id
         WHERE c.workspace_id = $1
           AND ($2::bool OR c.channel_type NOT IN ('dm', 'group_dm'))
           AND ($3::timestamptz IS NULL OR m.created_at >= $3)
           AND ($4::timestamptz IS NULL OR m.created_at <= $4)
        "#,
        workspace_id,
        job.include_dms,
        job.since,
        job.until
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        write(archive, &mut people, "reactions.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"SELECT to_jsonb(wm) AS "row!" FROM workspace_members wm WHERE wm.workspace_id = $1"#,
        workspace_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        write(archive, &mut people, "members.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"
        SELECT to_jsonb(f) AS "row!" FROM files f
          LEFT JOIN messages m ON m.id = f.message_id
          LEFT JOIN channels c ON c.id = m.channel_id
         WHERE f.workspace_id = $1
           AND f.draft_until IS NULL
           AND ($2::timestamptz IS NULL OR f.created_at >= $2)
           AND ($3::timestamptz IS NULL OR f.created_at <= $3)
           AND ($4::bool OR c.channel_type IN ('public', 'private'))
         ORDER BY f.created_at
        "#,
        workspace_id,
        job.since,
        job.until,
        job.include_dms
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        write(archive, &mut people, "files.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"
        SELECT to_jsonb(p) || jsonb_build_object('options', (
                 SELECT jsonb_agg(jsonb_build_object(
                          'id', o.id, 'position', o.position, 'label', o.label,
                          'votes', (SELECT COUNT(*) FROM poll_votes v WHERE v.option_id = o.id))
                        ORDER BY o.position)
                   FROM poll_options o WHERE o.poll_id = p.id)) AS "row!"
          FROM polls p
          JOIN messages m ON m.id = p.message_id
          JOIN channels c ON c.id = m.channel_id
         WHERE c.workspace_id = $1
           AND ($2::bool OR c.channel_type NOT IN ('dm', 'group_dm'))
           AND ($3::timestamptz IS NULL OR m.created_at >= $3)
           AND ($4::timestamptz IS NULL OR m.created_at <= $4)
         ORDER BY p.created_at
        "#,
        workspace_id,
        job.include_dms,
        job.since,
        job.until
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        write(archive, &mut people, "polls.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"
        SELECT to_jsonb(v) - 'single_choice' AS "row!" FROM poll_votes v
          JOIN polls p ON p.id = v.poll_id
          JOIN messages m ON m.id = p.message_id
          JOIN channels c ON c.id = m.channel_id
         WHERE c.workspace_id = $1
           AND NOT p.anonymous
           AND ($2::bool OR c.channel_type NOT IN ('dm', 'group_dm'))
           AND ($3::timestamptz IS NULL OR m.created_at >= $3)
           AND ($4::timestamptz IS NULL OR m.created_at <= $4)
         ORDER BY v.created_at
        "#,
        workspace_id,
        job.include_dms,
        job.since,
        job.until
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        write(archive, &mut people, "poll_votes.jsonl", &row);
    }

    export_lists(state, workspace_id, job.include_dms, archive, &mut people).await?;

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"SELECT to_jsonb(a) AS "row!" FROM audit_log a WHERE a.workspace_id = $1"#,
        workspace_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        write(archive, &mut people, "audit_log.jsonl", &row);
    }

    let ids: Vec<Uuid> = people.into_iter().collect();
    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"
        SELECT jsonb_build_object(
                 'id', u.id,
                 'name', COALESCE(wm.display_name, u.display_name),
                 'email', u.email) AS "row!"
          FROM users u
          LEFT JOIN workspace_members wm ON wm.user_id = u.id AND wm.workspace_id = $1
         WHERE u.id = ANY($2)
         ORDER BY u.id
        "#,
        workspace_id,
        &ids
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("users.jsonl", &row);
    }

    Ok(())
}

const PERSON_FIELDS: [&str; 3] = ["user_id", "created_by", "edited_by"];

fn write(
    archive: &mut ExportArchive,
    people: &mut BTreeSet<Uuid>,
    file: &str,
    row: &serde_json::Value,
) {
    note_people(row, people);
    archive.append(file, row);
}

fn note_people(value: &serde_json::Value, people: &mut BTreeSet<Uuid>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, field) in fields {
                match field.as_str().and_then(|id| id.parse().ok()) {
                    Some(id) if PERSON_FIELDS.contains(&key.as_str()) => {
                        people.insert(id);
                    }
                    _ => note_people(field, people),
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                note_people(item, people);
            }
        }
        _ => {}
    }
}

async fn export_lists(
    state: &AppState,
    workspace_id: Uuid,
    include_private: bool,
    archive: &mut ExportArchive,
    people: &mut BTreeSet<Uuid>,
) -> Result<(), String> {
    let lists = state
        .list_repo
        .exportable(workspace_id, include_private)
        .await
        .map_err(|e| e.to_string())?;
    for list in lists {
        let columns = state
            .list_repo
            .columns(list.id)
            .await
            .map_err(|e| e.to_string())?;
        let items = state
            .list_repo
            .items(list.id)
            .await
            .map_err(|e| e.to_string())?;
        let named = lists::csv::people_in(&columns, &items);
        people.extend(named.iter().copied());
        let names: HashMap<Uuid, String> = state
            .profile_repo
            .names(workspace_id, &named)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect();
        let comments = state
            .list_repo
            .list_comments(list.id)
            .await
            .map_err(|e| e.to_string())?;
        let path = format!("lists/{}.csv", list.id);
        write(
            archive,
            people,
            "lists.jsonl",
            &serde_json::json!({
                "list": list,
                "columns": columns,
                "items": path,
                "comments": comments,
            }),
        );
        archive.put_table(
            &path,
            lists::csv::table(&columns, &items, &names),
            items.len(),
        );
    }
    Ok(())
}

async fn export_user(
    state: &AppState,
    user_id: Uuid,
    archive: &mut ExportArchive,
) -> Result<(), String> {
    for file in [
        "profile.jsonl",
        "memberships.jsonl",
        "messages.jsonl",
        "files.jsonl",
    ] {
        archive.declare(file);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"SELECT to_jsonb(u) - 'password_hash' AS "row!" FROM users u WHERE u.id = $1"#,
        user_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("profile.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"SELECT to_jsonb(wm) AS "row!" FROM workspace_members wm WHERE wm.user_id = $1"#,
        user_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("memberships.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"SELECT to_jsonb(m) AS "row!" FROM messages m WHERE m.user_id = $1 ORDER BY m.created_at"#,
        user_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("messages.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar!(
        r#"SELECT to_jsonb(f) AS "row!" FROM files f WHERE f.user_id = $1"#,
        user_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("files.jsonl", &row);
    }

    Ok(())
}
