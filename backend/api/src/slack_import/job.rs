use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::slack_import::files::{HttpSlackClient, OfflineSlack, SlackClient};
use crate::slack_import::repo::ImportRun;
use crate::slack_import::service::Import;
use crate::slack_import::source::ZipSource;
use crate::state::AppState;

const POLL: Duration = Duration::from_secs(5);
const SWEEP_EVERY: Duration = Duration::from_secs(3600);
const HEARTBEAT: Duration = Duration::from_secs(30);
const STALE_AFTER: chrono::Duration = chrono::Duration::minutes(5);
const STALE_ERROR: &str = "The import stopped when the worker did. Start it again to finish it.";

/// Imports queued from the app. The CLI still exists for an export too large to
/// travel through a browser; this is for the ones that are not, where "upload it
/// and watch" is the whole job.
pub async fn start_import_worker(state: Arc<AppState>) {
    info!("Slack import worker started");

    let mut swept_at: Option<tokio::time::Instant> = None;
    loop {
        if swept_at.is_none_or(|at| at.elapsed() >= SWEEP_EVERY) {
            delete_kept_archives(&state).await;
            swept_at = Some(tokio::time::Instant::now());
        }

        match state
            .slack_import_repo
            .fail_stale(STALE_AFTER, STALE_ERROR)
            .await
        {
            Ok(stale) => {
                for id in stale {
                    warn!(import_id = %id, "Slack import had no heartbeat for five minutes; marked failed");
                }
            }
            Err(e) => warn!("Slack import worker: failed to sweep stale runs: {e}"),
        }

        match state.slack_import_repo.claim_next().await {
            Ok(Some(job)) => {
                let id = job.id;
                let beating = tokio::spawn(beat(state.clone(), id));
                let outcome = run(&state, job).await;
                beating.abort();
                if let Err(e) = outcome {
                    warn!(import_id = %id, "Slack import failed: {e}");
                    let _ = state.slack_import_repo.fail_run(id, &e).await;
                }
            }
            Ok(None) => tokio::time::sleep(POLL).await,
            Err(e) => {
                warn!("Slack import worker: failed to claim a job: {e}");
                tokio::time::sleep(POLL).await;
            }
        }
    }
}

pub(crate) async fn delete_kept_archives(state: &AppState) {
    let days = state.config.slack_import_keep_failed_days;
    let due = match state.slack_import_repo.archives_kept_past(days).await {
        Ok(due) => due,
        Err(e) => {
            warn!("Slack import worker: failed to sweep kept archives: {e}");
            return;
        }
    };
    for kept in due {
        match state.file_storage.delete(&kept.storage_key).await {
            Ok(()) => {
                info!(import_id = %kept.id, "deleted the export of a run that ended over {days} days ago")
            }
            Err(e) => warn!(import_id = %kept.id, "could not delete a failed run's export: {e}"),
        }
    }
}

async fn beat(state: Arc<AppState>, id: uuid::Uuid) {
    let mut ticks = tokio::time::interval(HEARTBEAT);
    loop {
        ticks.tick().await;
        if let Err(e) = state.slack_import_repo.beat(id).await {
            warn!(import_id = %id, "Slack import heartbeat failed: {e}");
        }
    }
}

pub(crate) async fn run(state: &Arc<AppState>, job: ImportRun) -> Result<(), String> {
    let storage_key = job
        .storage_key
        .clone()
        .ok_or_else(|| "this run has no uploaded archive".to_string())?;

    // The archive lives wherever uploads live, which may be S3. `ZipSource`
    // reads a file, so the worker puts one on disk for the length of the run and
    // takes it away afterwards.
    let (bytes, _) = state
        .file_storage
        .download(&storage_key)
        .await
        .map_err(|e| format!("could not read the uploaded archive: {e}"))?;

    let scratch = std::env::temp_dir().join(format!("slack-import-{}.zip", job.id));
    tokio::fs::write(&scratch, &bytes)
        .await
        .map_err(|e| format!("could not stage the archive: {e}"))?;

    let outcome = import(state, &job, &scratch).await;

    let _ = tokio::fs::remove_file(&scratch).await;
    if outcome.is_ok() {
        // The archive is a copy of somebody's entire Slack history sitting in
        // this instance's storage. Once it has been read there is no reason to
        // keep it.
        match state.file_storage.delete(&storage_key).await {
            Ok(()) => {
                if let Err(e) = state.slack_import_repo.archive_deleted(job.id).await {
                    warn!(import_id = %job.id, "could not record that the archive is gone: {e}");
                }
            }
            Err(e) => warn!(import_id = %job.id, "could not remove the uploaded archive: {e}"),
        }
    }

    outcome
}

async fn import(
    state: &Arc<AppState>,
    job: &ImportRun,
    archive: &std::path::Path,
) -> Result<(), String> {
    let mut source = ZipSource::open(archive).map_err(|e| e.to_string())?;

    let token = std::env::var("SLACK_TOKEN").ok();
    let slack: Box<dyn SlackClient> = match token {
        Some(token) => Box::new(HttpSlackClient::new(Some(token))),
        // Without a token the messages still import; the files and emoji are
        // named in the report as not fetched.
        None => Box::new(OfflineSlack),
    };

    let report = Import::open(state, slack.as_ref(), job.workspace_id, job.dry_run)
        .await
        .map_err(|e| e.to_string())?
        .reporting_into(job.id)
        .run(&mut source)
        .await
        .map_err(|e| e.to_string())?;

    info!(import_id = %job.id, "Slack import finished: {}", report.summary());
    Ok(())
}
