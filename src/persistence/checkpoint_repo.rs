//! Checkpoint repository for `SQLite` persistence.

use std::sync::Arc;

use crate::models::checkpoint::Checkpoint;
use crate::Result;

use super::db::Database;

/// Repository wrapper around `SQLite` for checkpoint records.
#[derive(Clone)]
pub struct CheckpointRepo {
    db: Arc<Database>,
}

#[allow(clippy::unused_async)] // todo!() stubs lack .await — Phase 3 will add real queries
impl CheckpointRepo {
    /// Create a new repository instance.
    #[must_use]
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Insert a new checkpoint record.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the database insert fails.
    pub async fn create(&self, _checkpoint: &Checkpoint) -> Result<Checkpoint> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T025)")
    }

    /// Retrieve a checkpoint by identifier.
    ///
    /// # Errors
    ///
    /// Returns `AppError::NotFound` if the checkpoint does not exist.
    pub async fn get_by_id(&self, _id: &str) -> Result<Checkpoint> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T025)")
    }

    /// List all checkpoints for a given session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_for_session(&self, _session_id: &str) -> Result<Vec<Checkpoint>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T025)")
    }

    /// Delete all checkpoints for a given session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the delete fails.
    pub async fn delete_for_session(&self, _session_id: &str) -> Result<()> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T025)")
    }
}
