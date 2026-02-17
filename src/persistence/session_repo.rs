//! Session repository for `SQLite` persistence.

use std::sync::Arc;

use crate::models::session::{Session, SessionMode, SessionStatus};
use crate::Result;

use super::db::Database;

/// Repository wrapper around `SQLite` for session records.
#[derive(Clone)]
pub struct SessionRepo {
    db: Arc<Database>,
}

#[allow(clippy::unused_async)] // todo!() stubs lack .await — Phase 3 will add real queries
impl SessionRepo {
    /// Create a new repository instance.
    #[must_use]
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Insert a new session record.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the database insert fails.
    pub async fn create(&self, _session: &Session) -> Result<Session> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// Retrieve a session by identifier.
    ///
    /// # Errors
    ///
    /// Returns `AppError::NotFound` if the session does not exist.
    pub async fn get_by_id(&self, _id: &str) -> Result<Session> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// Update session status and `updated_at` timestamp, respecting state machine.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the transition is invalid or persistence fails.
    pub async fn update_status(&self, _id: &str, _status: SessionStatus) -> Result<Session> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// Update only the last activity timestamp and optional tool name.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_last_activity(
        &self,
        _id: &str,
        _last_tool: Option<String>,
    ) -> Result<Session> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// List active sessions (status == active).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_active(&self) -> Result<Vec<Session>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// Update the progress snapshot on a session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_progress_snapshot(
        &self,
        _id: &str,
        _snapshot: Option<Vec<crate::models::progress::ProgressItem>>,
    ) -> Result<Session> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// Terminate a session, setting status and `terminated_at`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the transition is invalid or persistence fails.
    pub async fn set_terminated(&self, _id: &str, _status: SessionStatus) -> Result<Session> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// Count active sessions (status == active).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn count_active(&self) -> Result<u64> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// Retrieve the most recently interrupted session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_most_recent_interrupted(&self) -> Result<Option<Session>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// List all sessions with status `interrupted`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_interrupted(&self) -> Result<Vec<Session>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// List all sessions with status `active` or `paused`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_active_or_paused(&self) -> Result<Vec<Session>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }

    /// Update the operational mode for a session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_mode(&self, _id: &str, _mode: SessionMode) -> Result<Session> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T023)")
    }
}
