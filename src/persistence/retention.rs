//! Retention service for time-based data purge.
//!
//! Runs as a background task deleting children first
//! (approval requests, checkpoints, prompts, stall alerts),
//! then terminated sessions older than `retention_days`.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use super::db::Database;
use crate::Result;

const PURGE_INTERVAL: Duration = Duration::from_secs(3600);

/// Spawn the retention purge background task.
///
/// The task runs hourly. On each tick it deletes all associated records
/// for sessions that have been terminated for longer than `retention_days`.
#[must_use]
pub fn spawn_retention_task(
    db: Arc<Database>,
    retention_days: u32,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(PURGE_INTERVAL);
        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    info!("retention task shutting down");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(err) = purge(&db, retention_days).await {
                        error!(?err, "retention purge failed");
                    }
                }
            }
        }
    })
}

async fn purge(db: &Database, retention_days: u32) -> Result<()> {
    let cutoff = Utc::now() - chrono::Duration::days(i64::from(retention_days));
    let cutoff_str = cutoff.to_rfc3339();

    // Delete children first to maintain referential integrity.
    let child_tables = [
        "approval_request",
        "checkpoint",
        "continuation_prompt",
        "stall_alert",
    ];
    for table in child_tables {
        // SAFETY: `table` values are compile-time string literals defined above,
        // not user input, so interpolation here is not a SQL injection vector.
        let query = format!(
            "DELETE FROM {table} WHERE session_id IN \
             (SELECT VALUE id FROM session \
              WHERE status = 'terminated' AND terminated_at < $cutoff)"
        );
        db.query(&query).bind(("cutoff", &cutoff_str)).await?;
    }

    // Delete expired sessions.
    db.query(
        "DELETE FROM session \
         WHERE status = 'terminated' AND terminated_at < $cutoff",
    )
    .bind(("cutoff", &cutoff_str))
    .await?;

    info!(retention_days, "retention purge completed");
    Ok(())
}
