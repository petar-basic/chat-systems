use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};
use uuid::Uuid;

use crate::audit::{self, AuditAction, AuditEntry};
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
    // Declared up front so the manifest says "0 conversations" rather than
    // staying silent about them — the difference is the whole DM question.
    for file in [
        "channels.jsonl",
        "messages.jsonl",
        "message_edits.jsonl",
        "reactions.jsonl",
        "members.jsonl",
        "files.jsonl",
        "audit_log.jsonl",
        "conversations.jsonl",
        "conversation_messages.jsonl",
    ] {
        archive.declare(file);
    }

    let rows: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT to_jsonb(c) FROM channels c WHERE c.workspace_id = $1")
            .bind(workspace_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("channels.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar(
        r"
        SELECT to_jsonb(m) FROM messages m
          JOIN channels c ON c.id = m.channel_id
         WHERE c.workspace_id = $1
           AND ($2::timestamptz IS NULL OR m.created_at >= $2)
           AND ($3::timestamptz IS NULL OR m.created_at <= $3)
         ORDER BY m.created_at
        ",
    )
    .bind(workspace_id)
    .bind(job.since)
    .bind(job.until)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("messages.jsonl", &row);
    }

    // An export without edit history answers "what does it say now", not "what
    // happened" — which is the question being asked.
    let rows: Vec<serde_json::Value> = sqlx::query_scalar(
        r"
        SELECT to_jsonb(e) FROM message_edits e
          JOIN messages m ON m.id = e.message_id
          JOIN channels c ON c.id = m.channel_id
         WHERE c.workspace_id = $1
         ORDER BY e.edited_at
        ",
    )
    .bind(workspace_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("message_edits.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar(
        r"
        SELECT to_jsonb(r) FROM reactions r
          JOIN messages m ON m.id = r.message_id
          JOIN channels c ON c.id = m.channel_id
         WHERE c.workspace_id = $1
        ",
    )
    .bind(workspace_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("reactions.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT to_jsonb(wm) FROM workspace_members wm WHERE wm.workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("members.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT to_jsonb(f) FROM files f WHERE f.workspace_id = $1")
            .bind(workspace_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("files.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT to_jsonb(a) FROM audit_log a WHERE a.workspace_id = $1")
            .bind(workspace_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("audit_log.jsonl", &row);
    }

    // Silently exporting everyone's private conversations because an owner
    // clicked a button is the kind of default that ends up in a news story.
    if job.include_dms {
        let rows: Vec<serde_json::Value> =
            sqlx::query_scalar("SELECT to_jsonb(c) FROM conversations c WHERE c.workspace_id = $1")
                .bind(workspace_id)
                .fetch_all(&state.pool)
                .await
                .map_err(|e| e.to_string())?;
        for row in rows {
            archive.append("conversations.jsonl", &row);
        }

        let rows: Vec<serde_json::Value> = sqlx::query_scalar(
            r"
            SELECT to_jsonb(cm) FROM conversation_messages cm
              JOIN conversations c ON c.id = cm.conversation_id
             WHERE c.workspace_id = $1
               AND ($2::timestamptz IS NULL OR cm.created_at >= $2)
               AND ($3::timestamptz IS NULL OR cm.created_at <= $3)
             ORDER BY cm.created_at
            ",
        )
        .bind(workspace_id)
        .bind(job.since)
        .bind(job.until)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())?;
        for row in rows {
            archive.append("conversation_messages.jsonl", &row);
        }
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
        "conversation_messages.jsonl",
        "files.jsonl",
    ] {
        archive.declare(file);
    }

    let rows: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT to_jsonb(u) - 'password_hash' FROM users u WHERE u.id = $1")
            .bind(user_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("profile.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT to_jsonb(wm) FROM workspace_members wm WHERE wm.user_id = $1")
            .bind(user_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("memberships.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT to_jsonb(m) FROM messages m WHERE m.user_id = $1 ORDER BY m.created_at",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("messages.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT to_jsonb(cm) FROM conversation_messages cm WHERE cm.user_id = $1 ORDER BY cm.created_at",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("conversation_messages.jsonl", &row);
    }

    let rows: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT to_jsonb(f) FROM files f WHERE f.user_id = $1")
            .bind(user_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
    for row in rows {
        archive.append("files.jsonl", &row);
    }

    Ok(())
}
