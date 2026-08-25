use std::time::Duration;

use crate::state::AppState;

/// Small enough that each batch is a short transaction and vacuum can keep up,
/// large enough that a million rows is not a million round trips.
const BATCH: i64 = 5_000;
const PAUSE: Duration = Duration::from_millis(200);

/// Fills in the search vectors migration 27 could not write itself. Runs once at
/// boot and stops as soon as there is nothing left, so on a fresh instance it
/// costs two queries and on a large one it costs a slow background pass rather
/// than a locked table during deployment.
pub async fn backfill(state: &AppState) {
    for table in ["messages", "conversation_messages"] {
        let mut total: i64 = 0;
        loop {
            let sql = format!(
                "UPDATE {table} SET search_vector = search_vector_of(content) \
                 WHERE id IN (SELECT id FROM {table} WHERE search_vector IS NULL LIMIT $1)"
            );
            // The table name is one of two literals a line above, not input.
            let result = sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(BATCH)
                .execute(&state.pool)
                .await;

            let affected = match result {
                Ok(done) => done.rows_affected() as i64,
                Err(e) => {
                    tracing::warn!("search backfill on {} failed: {}", table, e);
                    break;
                }
            };

            if affected == 0 {
                break;
            }

            total += affected;
            metrics::counter!("search_backfill_rows_total", "table" => table)
                .increment(affected as u64);
            tracing::info!("search backfill: {} rows in {}", total, table);

            // The instance is serving traffic while this runs; a backfill that
            // saturates the pool is an outage of its own making.
            tokio::time::sleep(PAUSE).await;
        }

        if total > 0 {
            tracing::info!("search backfill finished: {} rows in {}", total, table);
        }
    }
}
