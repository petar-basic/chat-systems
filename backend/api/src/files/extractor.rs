use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::models::{FileTextStatus, TextJob};
use super::text::{self, Extraction};
use crate::state::AppState;

pub const BATCH: i64 = 20;
const IDLE: Duration = Duration::from_secs(15);
const PAUSE: Duration = Duration::from_millis(100);
const TIMEOUT: Duration = Duration::from_secs(30);
const STALE_AFTER_SECS: f64 = 600.0;
const MAX_ATTEMPTS: i32 = 3;
const BACKLOG_EVERY: Duration = Duration::from_secs(60);
const MIN_STORED_BYTES: usize = 16 * 1024;
const TSVECTOR_TOO_LONG: &str = "54000";

pub async fn start_text_extractor(state: Arc<AppState>) {
    let mut backlog_at: Option<Instant> = None;
    loop {
        if backlog_at.is_none_or(|at| at.elapsed() >= BACKLOG_EVERY) {
            report_backlog(&state).await;
            backlog_at = Some(Instant::now());
        }
        let jobs = match claim(&state).await {
            Ok(jobs) => jobs,
            Err(e) => {
                tracing::warn!("file text claim failed: {}", e);
                tokio::time::sleep(IDLE).await;
                continue;
            }
        };
        if jobs.is_empty() {
            tokio::time::sleep(IDLE).await;
            continue;
        }
        for job in jobs {
            process(&state, job).await;
        }
        tokio::time::sleep(PAUSE).await;
    }
}

pub async fn claim(state: &AppState) -> sqlx::Result<Vec<TextJob>> {
    let abandoned = state
        .file_repo
        .give_up_stale_texts(STALE_AFTER_SECS, MAX_ATTEMPTS)
        .await?;
    if abandoned > 0 {
        metrics::counter!("file_text_extraction_total", "status" => "abandoned")
            .increment(abandoned);
    }
    let mut jobs = state
        .file_repo
        .reclaim_stale_texts(BATCH, STALE_AFTER_SECS, MAX_ATTEMPTS)
        .await?;
    let room = BATCH - jobs.len() as i64;
    if room > 0 {
        jobs.extend(state.file_repo.claim_new_texts(room).await?);
    }
    Ok(jobs)
}

pub async fn process(state: &AppState, job: TextJob) {
    let extraction = extract(state, &job).await;
    if let Extraction::Failed(reason) = &extraction {
        tracing::info!(file_id = %job.file_id, "file text extraction failed: {}", reason);
    }
    let status = extraction.status();
    let stored = match extraction {
        Extraction::Text(content) => store(state, &job, content).await,
        _ => state
            .file_repo
            .finish_text(job.file_id, status, None)
            .await
            .map(|_| status),
    };
    match stored {
        Ok(status) => {
            metrics::counter!("file_text_extraction_total", "status" => status.as_str())
                .increment(1);
        }
        Err(e) => tracing::warn!(file_id = %job.file_id, "file text not stored: {}", e),
    }
}

async fn extract(state: &AppState, job: &TextJob) -> Extraction {
    if !text::readable(job.kind, &job.mime_type, &job.filename) {
        return Extraction::Unsupported;
    }
    if job.size_bytes > text::MAX_SOURCE_BYTES {
        return Extraction::TooLarge;
    }
    let bytes = match state.file_storage.download(&job.storage_key).await {
        Ok((bytes, _)) => bytes,
        Err(e) => return Extraction::Failed(format!("download: {e}")),
    };
    if bytes.len() as i64 > text::MAX_SOURCE_BYTES {
        return Extraction::TooLarge;
    }
    let kind = job.kind;
    let mime = job.mime_type.clone();
    let filename = job.filename.clone();
    let work = tokio::task::spawn_blocking(move || {
        catch_unwind(AssertUnwindSafe(|| {
            text::extract(kind, &mime, &filename, &bytes)
        }))
    });
    match tokio::time::timeout(TIMEOUT, work).await {
        Ok(Ok(Ok(extraction))) => extraction,
        Ok(Ok(Err(_))) => Extraction::Failed("extractor panicked".into()),
        Ok(Err(e)) => Extraction::Failed(format!("extractor task: {e}")),
        Err(_) => Extraction::Failed(format!("timed out after {}s", TIMEOUT.as_secs())),
    }
}

async fn store(state: &AppState, job: &TextJob, content: String) -> sqlx::Result<FileTextStatus> {
    let mut end = content.len();
    loop {
        let attempt = state
            .file_repo
            .finish_text(job.file_id, FileTextStatus::Done, Some(&content[..end]))
            .await;
        match attempt {
            Ok(()) => return Ok(FileTextStatus::Done),
            Err(e) if is_tsvector_too_long(&e) && end > MIN_STORED_BYTES => {
                end = content.floor_char_boundary(end / 2);
            }
            Err(e) if is_tsvector_too_long(&e) => {
                state
                    .file_repo
                    .finish_text(job.file_id, FileTextStatus::TooLarge, None)
                    .await?;
                return Ok(FileTextStatus::TooLarge);
            }
            Err(e) => return Err(e),
        }
    }
}

fn is_tsvector_too_long(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|e| e.code())
        .is_some_and(|code| code == TSVECTOR_TOO_LONG)
}

async fn report_backlog(state: &AppState) {
    match state.file_repo.text_backlog().await {
        Ok(backlog) => metrics::gauge!("file_text_backlog").set(backlog as f64),
        Err(e) => tracing::warn!("file text backlog query failed: {}", e),
    }
}
