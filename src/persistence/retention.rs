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

#[allow(clippy::unused_async)] // todo!() stub — Phase 4 will add real queries
async fn purge(_db: &Database, _retention_days: u32) -> Result<()> {
    todo!("rewrite with sqlx in Phase 4 (T042/T043)")
}
