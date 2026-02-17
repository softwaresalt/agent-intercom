//! Checkpoint repository for `SQLite` persistence.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;

use crate::models::checkpoint::Checkpoint;
use crate::models::progress::ProgressItem;
use crate::{AppError, Result};

use super::db::Database;

/// Repository wrapper around `SQLite` for checkpoint records.
#[derive(Clone)]
pub struct CheckpointRepo {
    db: Arc<Database>,
}

/// Internal row struct for `SQLite` deserialization.
#[derive(sqlx::FromRow)]
struct CheckpointRow {
    id: String,
    session_id: String,
    label: Option<String>,
    session_state: String,
    file_hashes: String,
    workspace_root: String,
    progress_snapshot: Option<String>,
    created_at: String,
}

impl CheckpointRow {
    /// Convert a database row into the domain model.
    fn into_checkpoint(self) -> Result<Checkpoint> {
        let session_state: serde_json::Value = serde_json::from_str(&self.session_state)
            .map_err(|e| AppError::Db(format!("invalid session_state json: {e}")))?;
        let file_hashes: HashMap<String, String> = serde_json::from_str(&self.file_hashes)
            .map_err(|e| AppError::Db(format!("invalid file_hashes json: {e}")))?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|e| AppError::Db(format!("invalid created_at: {e}")))?
            .with_timezone(&Utc);
        let progress_snapshot: Option<Vec<ProgressItem>> = self
            .progress_snapshot
            .as_deref()
            .map(|s| {
                serde_json::from_str(s)
                    .map_err(|e| AppError::Db(format!("invalid progress_snapshot json: {e}")))
            })
            .transpose()?;

        Ok(Checkpoint {
            id: self.id,
            session_id: self.session_id,
            label: self.label,
            session_state,
            file_hashes,
            workspace_root: self.workspace_root,
            progress_snapshot,
            created_at,
        })
    }
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
        let session_state = serde_json::to_string(&checkpoint.session_state)
            .map_err(|e| AppError::Db(format!("failed to serialize session_state: {e}")))?;
        let file_hashes = serde_json::to_string(&checkpoint.file_hashes)
            .map_err(|e| AppError::Db(format!("failed to serialize file_hashes: {e}")))?;
        let created_at = checkpoint.created_at.to_rfc3339();
        let progress_snapshot = checkpoint
            .progress_snapshot
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| AppError::Db(format!("failed to serialize progress_snapshot: {e}")))?;

        sqlx::query(
            "INSERT INTO checkpoint (id, session_id, label, session_state, file_hashes,
             workspace_root, progress_snapshot, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&checkpoint.id)
        .bind(&checkpoint.session_id)
        .bind(&checkpoint.label)
        .bind(&session_state)
        .bind(&file_hashes)
        .bind(&checkpoint.workspace_root)
        .bind(&progress_snapshot)
        .bind(&created_at)
        .execute(self.db.as_ref())
        .await?;

        Ok(checkpoint.clone())
    }

    /// Retrieve a checkpoint by identifier.
    ///
    /// Returns `Ok(None)` if the checkpoint does not exist.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Checkpoint>> {
        let row: Option<CheckpointRow> = sqlx::query_as("SELECT * FROM checkpoint WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.db.as_ref())
            .await?;

        row.map(CheckpointRow::into_checkpoint).transpose()
    }

    /// List all checkpoints for a given session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_for_session(&self, session_id: &str) -> Result<Vec<Checkpoint>> {
        let rows: Vec<CheckpointRow> =
            sqlx::query_as("SELECT * FROM checkpoint WHERE session_id = ?1 ORDER BY created_at")
                .bind(session_id)
                .fetch_all(self.db.as_ref())
                .await?;

        rows.into_iter()
            .map(CheckpointRow::into_checkpoint)
            .collect()
    }

    /// Delete all checkpoints for a given session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the delete fails.
    pub async fn delete_for_session(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM checkpoint WHERE session_id = ?1")
            .bind(session_id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }
}
