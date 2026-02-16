//! Session repository for `SurrealDB` persistence.

use std::sync::Arc;

use chrono::Utc;
use serde::Deserialize;

use crate::models::session::{Session, SessionMode, SessionStatus};
use crate::{AppError, Result};

use super::db::Database;

/// Repository wrapper around `SurrealDB` for session records.
#[derive(Clone)]
pub struct SessionRepo {
    db: Arc<Database>,
}

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
    pub async fn create(&self, session: &Session) -> Result<Session> {
        self.db
            .create(("session", session.id.as_str()))
            .content(session)
            .await?
            .ok_or_else(|| AppError::Db("failed to create session".into()))
    }

    /// Retrieve a session by identifier.
    ///
    /// # Errors
    ///
    /// Returns `AppError::NotFound` if the session does not exist.
    pub async fn get_by_id(&self, id: &str) -> Result<Session> {
        let session: Option<Session> = self.db.select(("session", id)).await?;
        session.ok_or_else(|| AppError::NotFound("session not found".into()))
    }

    /// Update session status and `updated_at` timestamp, respecting state machine.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the transition is invalid or persistence fails.
    pub async fn update_status(&self, id: &str, status: SessionStatus) -> Result<Session> {
        let mut current = self.get_by_id(id).await?;
        if !current.can_transition_to(status) {
            return Err(AppError::Db("invalid session status transition".into()));
        }

        current.status = status;
        current.updated_at = Utc::now();

        self.db
            .update(("session", id))
            .content(&current)
            .await?
            .ok_or_else(|| AppError::Db("failed to update session status".into()))
    }

    /// Update only the last activity timestamp and optional tool name.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_last_activity(
        &self,
        id: &str,
        last_tool: Option<String>,
    ) -> Result<Session> {
        let mut current = self.get_by_id(id).await?;
        current.updated_at = Utc::now();
        current.last_tool = last_tool;

        self.db
            .update(("session", id))
            .content(&current)
            .await?
            .ok_or_else(|| AppError::Db("failed to update session activity".into()))
    }

    /// List active sessions (status == active).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_active(&self) -> Result<Vec<Session>> {
        let mut response = self
            .db
            .query("SELECT * FROM session WHERE status = 'active'")
            .await?;
        response.take::<Vec<Session>>(0).map_err(AppError::from)
    }

    /// Update the progress snapshot on a session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_progress_snapshot(
        &self,
        id: &str,
        snapshot: Option<Vec<crate::models::progress::ProgressItem>>,
    ) -> Result<Session> {
        let mut current = self.get_by_id(id).await?;
        current.progress_snapshot = snapshot;
        current.updated_at = Utc::now();
        self.db
            .update(("session", id))
            .content(&current)
            .await?
            .ok_or_else(|| AppError::Db("failed to update progress snapshot".into()))
    }

    /// Terminate a session, setting status and `terminated_at`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the transition is invalid or persistence fails.
    pub async fn set_terminated(&self, id: &str, status: SessionStatus) -> Result<Session> {
        let mut current = self.get_by_id(id).await?;
        if !current.can_transition_to(status) {
            return Err(AppError::Db("invalid terminal status transition".into()));
        }
        current.status = status;
        current.terminated_at = Some(Utc::now());
        current.updated_at = Utc::now();
        self.db
            .update(("session", id))
            .content(&current)
            .await?
            .ok_or_else(|| AppError::Db("failed to set session terminated".into()))
    }

    /// Count active sessions (status == active).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn count_active(&self) -> Result<u64> {
        let mut response = self
            .db
            .query("SELECT count() AS count FROM session WHERE status = 'active' GROUP ALL")
            .await?;
        let count_row: Option<CountRow> = response.take(0)?;
        Ok(count_row.map_or(0, |row| row.count))
    }

    /// Retrieve the most recently interrupted session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_most_recent_interrupted(&self) -> Result<Option<Session>> {
        let mut response = self
            .db
            .query(
                "SELECT * FROM session WHERE status = 'interrupted' \
                 ORDER BY updated_at DESC LIMIT 1",
            )
            .await?;
        let sessions: Vec<Session> = response.take(0)?;
        Ok(sessions.into_iter().next())
    }

    /// List all sessions with status `interrupted`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_interrupted(&self) -> Result<Vec<Session>> {
        let mut response = self
            .db
            .query("SELECT * FROM session WHERE status = 'interrupted'")
            .await?;
        response.take::<Vec<Session>>(0).map_err(AppError::from)
    }

    /// List all sessions with status `active` or `paused`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_active_or_paused(&self) -> Result<Vec<Session>> {
        let mut response = self
            .db
            .query("SELECT * FROM session WHERE status = 'active' OR status = 'paused'")
            .await?;
        response.take::<Vec<Session>>(0).map_err(AppError::from)
    }

    /// Update the operational mode for a session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_mode(&self, id: &str, mode: SessionMode) -> Result<Session> {
        let mut current = self.get_by_id(id).await?;
        current.mode = mode;
        current.updated_at = Utc::now();
        self.db
            .update(("session", id))
            .content(&current)
            .await?
            .ok_or_else(|| AppError::Db("failed to update session mode".into()))
    }
}

#[derive(Debug, Deserialize)]
struct CountRow {
    count: u64,
}
