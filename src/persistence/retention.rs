//! Retention service for time-based data purge.
//!
//! Runs as a background task deleting children first
//! (approval requests, checkpoints, prompts, stall alerts),
//! then terminated sessions older than `retention_days`.

use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use super::db::Database;
use crate::Result;

const PURGE_INTERVAL: Duration = Duration::from_secs(3600);

/// Spawn the retention purge background task.
///
/// The first purge runs after `PURGE_INTERVAL` (1 hour), not immediately
/// on startup.  Subsequent purges repeat at the same interval.
#[must_use]
pub fn spawn_retention_task(
    db: Arc<Database>,
    retention_days: u32,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval_at(tokio::time::Instant::now() + PURGE_INTERVAL, PURGE_INTERVAL);
        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    info!("retention task shutting down");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(err) = purge(&db, retention_days).await {
                        tracing::error!(?err, "retention purge failed");
                    }
                }
            }
        }
    })
}

/// Purge terminated sessions older than `retention_days` and all their child
/// records.
///
/// Deletion order (children before parent): `stall_alert` → `checkpoint` →
/// `continuation_prompt` → `approval_request` → `steering_message` →
/// `task_inbox` (by age) → `session`.
///
/// # Errors
///
/// Returns an error if any of the delete queries fail.
pub async fn purge(db: &Database, retention_days: u32) -> Result<()> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(retention_days));
    let cutoff_str = cutoff.to_rfc3339();

    // Children first — referential integrity.
    sqlx::query(
        "DELETE FROM stall_alert WHERE session_id IN \
         (SELECT id FROM session WHERE terminated_at IS NOT NULL AND terminated_at < ?1)",
    )
    .bind(&cutoff_str)
    .execute(db)
    .await?;

    sqlx::query(
        "DELETE FROM checkpoint WHERE session_id IN \
         (SELECT id FROM session WHERE terminated_at IS NOT NULL AND terminated_at < ?1)",
    )
    .bind(&cutoff_str)
    .execute(db)
    .await?;

    sqlx::query(
        "DELETE FROM continuation_prompt WHERE session_id IN \
         (SELECT id FROM session WHERE terminated_at IS NOT NULL AND terminated_at < ?1)",
    )
    .bind(&cutoff_str)
    .execute(db)
    .await?;

    sqlx::query(
        "DELETE FROM approval_request WHERE session_id IN \
         (SELECT id FROM session WHERE terminated_at IS NOT NULL AND terminated_at < ?1)",
    )
    .bind(&cutoff_str)
    .execute(db)
    .await?;

    // Steering messages are tied to a session_id (T077).
    sqlx::query(
        "DELETE FROM steering_message WHERE session_id IN \
         (SELECT id FROM session WHERE terminated_at IS NOT NULL AND terminated_at < ?1)",
    )
    .bind(&cutoff_str)
    .execute(db)
    .await?;

    // Task inbox items are not session-scoped, so purge by created_at (T077).
    // Purge all items older than the cutoff regardless of consumed status —
    // unconsumed tasks older than the retention window are stale and should
    // not accumulate indefinitely.
    sqlx::query("DELETE FROM task_inbox WHERE created_at < ?1")
        .bind(&cutoff_str)
        .execute(db)
        .await?;

    // Parent last.
    let result =
        sqlx::query("DELETE FROM session WHERE terminated_at IS NOT NULL AND terminated_at < ?1")
            .bind(&cutoff_str)
            .execute(db)
            .await?;

    let purged = result.rows_affected();
    if purged > 0 {
        info!(count = purged, "purged expired sessions and child records");
    }

    Ok(())
}
