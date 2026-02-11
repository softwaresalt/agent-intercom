//! Session repository for `SurrealDB` persistence.

use std::sync::Arc;

use chrono::Utc;
use serde::Deserialize;

use crate::models::session::{Session, SessionStatus};
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
            .create(("session", session.id.clone()))
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
            .content(current)
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
            .content(current)
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

    /// Count active sessions (status == active).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn count_active(&self) -> Result<u64> {
        let mut response = self
            .db
            .query("SELECT count() AS count FROM session WHERE status = 'active'")
            .await?;
        let count_row: Option<CountRow> = response.take(0)?;
        count_row
            .map(|row| row.count)
            .ok_or_else(|| AppError::Db("failed to count sessions".into()))
    }
}

#[derive(Debug, Deserialize)]
struct CountRow {
    count: u64,
}
