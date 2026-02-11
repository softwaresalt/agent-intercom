//! Checkpoint repository for `SurrealDB` persistence.

use std::sync::Arc;

use crate::models::checkpoint::Checkpoint;
use crate::{AppError, Result};

use super::db::Database;

/// Repository wrapper around `SurrealDB` for checkpoint records.
#[derive(Clone)]
pub struct CheckpointRepo {
    db: Arc<Database>,
}

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
    pub async fn create(&self, checkpoint: &Checkpoint) -> Result<Checkpoint> {
        self.db
            .create(("checkpoint", checkpoint.id.clone()))
            .content(checkpoint)
            .await?
            .ok_or_else(|| AppError::Db("failed to create checkpoint".into()))
    }

    /// Retrieve a checkpoint by identifier.
    ///
    /// # Errors
    ///
    /// Returns `AppError::NotFound` if the checkpoint does not exist.
    pub async fn get_by_id(&self, id: &str) -> Result<Checkpoint> {
        let checkpoint: Option<Checkpoint> =
            self.db.select(("checkpoint", id)).await?;
        checkpoint.ok_or_else(|| AppError::NotFound("checkpoint not found".into()))
    }

    /// List all checkpoints for a given session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_for_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<Checkpoint>> {
        let mut response = self
            .db
            .query(
                "SELECT * FROM checkpoint \
                 WHERE session_id = $sid \
                 ORDER BY created_at DESC",
            )
            .bind(("sid", session_id))
            .await?;
        response.take::<Vec<Checkpoint>>(0).map_err(AppError::from)
    }

    /// Delete all checkpoints for a given session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the delete fails.
    pub async fn delete_for_session(&self, session_id: &str) -> Result<()> {
        self.db
            .query("DELETE FROM checkpoint WHERE session_id = $sid")
            .bind(("sid", session_id))
            .await?;
        Ok(())
    }
}
