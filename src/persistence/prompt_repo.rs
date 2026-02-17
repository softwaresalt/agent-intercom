//! Continuation prompt repository for `SQLite` persistence.

use std::sync::Arc;

use crate::models::prompt::{ContinuationPrompt, PromptDecision};
use crate::Result;

use super::db::Database;

/// Repository wrapper around `SQLite` for continuation prompt records.
#[derive(Clone)]
pub struct PromptRepo {
    db: Arc<Database>,
}

#[allow(clippy::unused_async)] // todo!() stubs lack .await — Phase 3 will add real queries
impl PromptRepo {
    /// Create a new repository instance.
    #[must_use]
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Insert a new continuation prompt record.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the database insert fails.
    pub async fn create(&self, _prompt: &ContinuationPrompt) -> Result<ContinuationPrompt> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T026)")
    }

    /// Retrieve a prompt by identifier.
    ///
    /// # Errors
    ///
    /// Returns `AppError::NotFound` if the prompt does not exist.
    pub async fn get_by_id(&self, _id: &str) -> Result<ContinuationPrompt> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T026)")
    }

    /// Retrieve the pending prompt for a session, if any.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_pending_for_session(
        &self,
        _session_id: &str,
    ) -> Result<Option<ContinuationPrompt>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T026)")
    }

    /// Update the decision and optional instruction on a prompt.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_decision(
        &self,
        _id: &str,
        _decision: PromptDecision,
        _instruction: Option<String>,
    ) -> Result<ContinuationPrompt> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T026)")
    }

    /// List all pending prompts (no decision yet) across sessions.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_pending(&self) -> Result<Vec<ContinuationPrompt>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T026)")
    }
}
