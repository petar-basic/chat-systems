use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::audit::{self, AuditAction, AuditEntry};
use crate::state::AppState;

const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const BETWEEN_BATCHES: Duration = Duration::from_millis(200);
/// A safety rail, not a policy: a run that keeps finding work stops and picks up
/// tomorrow rather than holding the database all night.
const MAX_BATCHES_PER_TABLE: usize = 200;

#[derive(Debug, Default)]
pub struct PurgeCounts {
    pub messages: u64,
    pub files: u64,
    pub notifications: u64,
    pub audit: u64,
    pub reset_tokens: u64,
    pub refresh_tokens: u64,
    pub invites: u64,
    pub hook_executions: u64,
}

impl PurgeCounts {
    fn total(&self) -> u64 {
        self.messages
            + self.files
            + self.notifications
            + self.audit
            + self.reset_tokens
            + self.refresh_tokens
            + self.invites
            + self.hook_executions
    }

    fn record_metrics(&self) {
        for (table, count) in [
            ("messages", self.messages),
            ("files", self.files),
            ("notifications", self.notifications),
            ("audit_log", self.audit),
            ("password_reset_tokens", self.reset_tokens),
            ("refresh_tokens", self.refresh_tokens),
            ("workspace_invites", self.invites),
            ("hook_executions", self.hook_executions),
        ] {
            metrics::counter!("retention_rows_deleted_total", "table" => table).increment(count);
        }
    }
}

pub async fn start_retention_job(state: Arc<AppState>) {
    info!(
        dry_run = state.config.retention_dry_run,
        "Retention job started"
    );

    loop {
        tokio::time::sleep(INTERVAL).await;
        let counts = run_once(&state).await;
        info!(
            dry_run = state.config.retention_dry_run,
            messages = counts.messages,
            files = counts.files,
            notifications = counts.notifications,
            audit = counts.audit,
            reset_tokens = counts.reset_tokens,
            refresh_tokens = counts.refresh_tokens,
            invites = counts.invites,
            hook_executions = counts.hook_executions,
            "Retention pass complete"
        );
    }
}

/// One full pass. Returns what it deleted — or, in dry-run, what it would have.
pub async fn run_once(state: &AppState) -> PurgeCounts {
    let dry_run = state.config.retention_dry_run;
    let mut counts = PurgeCounts::default();

    // Unconditional cleanups first: nothing governs them and nothing is served
    // by keeping a consumed reset token or a refresh token that expired months
    // ago.
    if dry_run {
        match state.retention_repo.count_expired_tokens().await {
            Ok((reset, refresh, invites, executions)) => {
                counts.reset_tokens = reset as u64;
                counts.refresh_tokens = refresh as u64;
                counts.invites = invites as u64;
                counts.hook_executions = executions as u64;
            }
            Err(e) => warn!("Retention: failed to count expired tokens: {}", e),
        }
    } else {
        match state.retention_repo.purge_expired_tokens().await {
            Ok((reset, refresh, invites, executions)) => {
                counts.reset_tokens = reset;
                counts.refresh_tokens = refresh;
                counts.invites = invites;
                counts.hook_executions = executions;
            }
            Err(e) => warn!("Retention: failed to purge expired tokens: {}", e),
        }
    }

    let policies = match state.retention_repo.workspaces_with_policies().await {
        Ok(policies) => policies,
        Err(e) => {
            warn!("Retention: failed to list policies: {}", e);
            counts.record_metrics();
            return counts;
        }
    };

    for policy in policies {
        // Files before messages: an attachment whose message is gone is
        // unreachable but still downloadable by key, which is the mistake
        // CS-020 fixed on the request path.
        if let Some(days) = policy.file_days {
            counts.files += purge_files(state, policy.workspace_id, days, dry_run).await;
        }
        if let Some(days) = policy.message_days {
            counts.messages += purge_loop(dry_run, || async {
                state
                    .retention_repo
                    .purge_messages(policy.workspace_id, days)
                    .await
            })
            .await;
            if dry_run {
                counts.messages += state
                    .retention_repo
                    .count_messages_past(policy.workspace_id, days)
                    .await
                    .unwrap_or(0) as u64;
            }
        }

        counts.notifications += purge_loop(dry_run, || async {
            state
                .retention_repo
                .purge_notifications(policy.workspace_id, policy.notification_days)
                .await
        })
        .await;

        counts.audit += purge_loop(dry_run, || async {
            state
                .retention_repo
                .purge_audit(policy.workspace_id, policy.audit_days)
                .await
        })
        .await;

        if !dry_run && counts.total() > 0 {
            audit::record(
                state,
                AuditEntry::new(AuditAction::RetentionPurge, policy.workspace_id)
                    .workspace(policy.workspace_id)
                    .details(serde_json::json!({
                        "messages": counts.messages,
                        "files": counts.files,
                        "notifications": counts.notifications,
                        "audit": counts.audit,
                    })),
            )
            .await;
        }
    }

    counts.record_metrics();
    counts
}

async fn purge_loop<F, Fut>(dry_run: bool, mut batch: F) -> u64
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = sqlx::Result<u64>>,
{
    if dry_run {
        return 0;
    }
    let mut total = 0;
    for _ in 0..MAX_BATCHES_PER_TABLE {
        match batch().await {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                tokio::time::sleep(BETWEEN_BATCHES).await;
            }
            Err(e) => {
                warn!("Retention: batch failed: {}", e);
                break;
            }
        }
    }
    total
}

async fn purge_files(state: &AppState, workspace_id: uuid::Uuid, days: i32, dry_run: bool) -> u64 {
    let mut total = 0;
    for _ in 0..MAX_BATCHES_PER_TABLE {
        let batch = match state.retention_repo.files_past(workspace_id, days).await {
            Ok(batch) => batch,
            Err(e) => {
                warn!("Retention: failed to list files: {}", e);
                break;
            }
        };
        if batch.is_empty() {
            break;
        }
        if dry_run {
            return batch.len() as u64;
        }

        let mut ids = Vec::with_capacity(batch.len());
        for (id, storage_key) in batch {
            // A previous run may have removed the object and died before the
            // row; a missing object is expected, not a reason to stop.
            if let Err(e) = state.file_storage.delete(&storage_key).await {
                warn!(
                    "Retention: object {} already gone or unreachable: {}",
                    storage_key, e
                );
            }
            ids.push(id);
        }
        match state.retention_repo.delete_file_rows(&ids).await {
            Ok(n) => total += n,
            Err(e) => {
                warn!("Retention: failed to delete file rows: {}", e);
                break;
            }
        }
        tokio::time::sleep(BETWEEN_BATCHES).await;
    }
    total
}
