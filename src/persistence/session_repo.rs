//! Session repository for `SQLite` persistence.

use std::sync::Arc;

use chrono::Utc;
use sqlx::Row;

use crate::models::progress::ProgressItem;
use crate::models::session::{Session, SessionMode, SessionStatus};
use crate::{AppError, Result};

use super::db::Database;

/// Repository wrapper around `SQLite` for session records.
#[derive(Clone)]
pub struct SessionRepo {
    db: Arc<Database>,
}

/// Internal row struct for `SQLite` deserialization.
#[derive(sqlx::FromRow)]
struct SessionRow {
    id: String,
    owner_user_id: String,
    workspace_root: String,
    status: String,
    prompt: Option<String>,
    mode: String,
    created_at: String,
    updated_at: String,
    terminated_at: Option<String>,
    last_tool: Option<String>,
    nudge_count: i64,
    stall_paused: i64,
    progress_snapshot: Option<String>,
}

impl SessionRow {
    /// Convert a database row into the domain model.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if enum parsing or JSON deserialization fails.
    fn into_session(self) -> Result<Session> {
        let status = parse_status(&self.status)?;
        let mode = parse_mode(&self.mode)?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|e| AppError::Db(format!("invalid created_at: {e}")))?
            .with_timezone(&Utc);
        let updated_at = chrono::DateTime::parse_from_rfc3339(&self.updated_at)
            .map_err(|e| AppError::Db(format!("invalid updated_at: {e}")))?
            .with_timezone(&Utc);
        let terminated_at = self
            .terminated_at
            .as_deref()
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| AppError::Db(format!("invalid terminated_at: {e}")))
            })
            .transpose()?;
        let progress_snapshot: Option<Vec<ProgressItem>> = self
            .progress_snapshot
            .as_deref()
            .map(|s| {
                serde_json::from_str(s)
                    .map_err(|e| AppError::Db(format!("invalid progress_snapshot json: {e}")))
            })
            .transpose()?;

        Ok(Session {
            id: self.id,
            owner_user_id: self.owner_user_id,
            workspace_root: self.workspace_root,
            status,
            prompt: self.prompt,
            mode,
            created_at,
            updated_at,
            last_tool: self.last_tool,
            nudge_count: self.nudge_count,
            stall_paused: self.stall_paused != 0,
            terminated_at,
            progress_snapshot,
        })
    }
}

/// Parse a status string into the domain enum.
fn parse_status(s: &str) -> Result<SessionStatus> {
    match s {
        "created" => Ok(SessionStatus::Created),
        "active" => Ok(SessionStatus::Active),
        "paused" => Ok(SessionStatus::Paused),
        "terminated" => Ok(SessionStatus::Terminated),
        "interrupted" => Ok(SessionStatus::Interrupted),
        other => Err(AppError::Db(format!("invalid session status: {other}"))),
    }
}

/// Serialize a status enum to its database string.
fn status_str(s: SessionStatus) -> &'static str {
    match s {
        SessionStatus::Created => "created",
        SessionStatus::Active => "active",
        SessionStatus::Paused => "paused",
        SessionStatus::Terminated => "terminated",
        SessionStatus::Interrupted => "interrupted",
    }
}

/// Parse a mode string into the domain enum.
fn parse_mode(s: &str) -> Result<SessionMode> {
    match s {
        "remote" => Ok(SessionMode::Remote),
        "local" => Ok(SessionMode::Local),
        "hybrid" => Ok(SessionMode::Hybrid),
        other => Err(AppError::Db(format!("invalid session mode: {other}"))),
    }
}

/// Serialize a mode enum to its database string.
fn mode_str(m: SessionMode) -> &'static str {
    match m {
        SessionMode::Remote => "remote",
        SessionMode::Local => "local",
        SessionMode::Hybrid => "hybrid",
    }
}

/// Valid session status transitions.
fn is_valid_transition(from: SessionStatus, to: SessionStatus) -> bool {
    matches!(
        (from, to),
        (
            SessionStatus::Created | SessionStatus::Paused | SessionStatus::Interrupted,
            SessionStatus::Active
        ) | (
            SessionStatus::Active,
            SessionStatus::Paused | SessionStatus::Interrupted | SessionStatus::Terminated
        ) | (
            SessionStatus::Paused,
            SessionStatus::Terminated | SessionStatus::Interrupted
        )
    )
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
        let status = status_str(session.status);
        let mode = mode_str(session.mode);
        let created_at = session.created_at.to_rfc3339();
        let updated_at = session.updated_at.to_rfc3339();
        let terminated_at = session.terminated_at.map(|dt| dt.to_rfc3339());
        let stall_paused: i64 = i64::from(session.stall_paused);
        let progress_snapshot = session
            .progress_snapshot
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| AppError::Db(format!("failed to serialize progress_snapshot: {e}")))?;

        sqlx::query(
            "INSERT INTO session (id, owner_user_id, workspace_root, status, prompt, mode,
             created_at, updated_at, terminated_at, last_tool, nudge_count, stall_paused,
             progress_snapshot)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )
        .bind(&session.id)
        .bind(&session.owner_user_id)
        .bind(&session.workspace_root)
        .bind(status)
        .bind(&session.prompt)
        .bind(mode)
        .bind(&created_at)
        .bind(&updated_at)
        .bind(&terminated_at)
        .bind(&session.last_tool)
        .bind(session.nudge_count)
        .bind(stall_paused)
        .bind(&progress_snapshot)
        .execute(self.db.as_ref())
        .await?;

        Ok(session.clone())
    }

    /// Retrieve a session by identifier.
    ///
    /// Returns `Ok(None)` if the session does not exist.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Session>> {
        let row: Option<SessionRow> = sqlx::query_as("SELECT * FROM session WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.db.as_ref())
            .await?;

        row.map(SessionRow::into_session).transpose()
    }

    /// Update session status and `updated_at` timestamp.
    ///
    /// Validates state transitions before applying the update.
    /// Returns the updated session entity.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the transition is invalid or the session
    /// is not found.
    pub async fn update_status(&self, id: &str, status: SessionStatus) -> Result<Session> {
        let current = self
            .get_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("session {id} not found")))?;

        if !is_valid_transition(current.status, status) {
            return Err(AppError::Db(format!(
                "invalid status transition: {} -> {}",
                status_str(current.status),
                status_str(status)
            )));
        }

        let now = Utc::now().to_rfc3339();
        let status_s = status_str(status);

        sqlx::query("UPDATE session SET status = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(status_s)
            .bind(&now)
            .bind(id)
            .execute(self.db.as_ref())
            .await?;

        self.get_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("session {id} not found after update")))
    }

    /// Update only the last activity timestamp and optional tool name.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_last_activity(&self, id: &str, last_tool: Option<String>) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query("UPDATE session SET last_tool = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&last_tool)
            .bind(&now)
            .bind(id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }

    /// List active sessions (status == `active`).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_active(&self) -> Result<Vec<Session>> {
        let rows: Vec<SessionRow> = sqlx::query_as(
            "SELECT * FROM session WHERE status = 'active' ORDER BY updated_at DESC",
        )
        .fetch_all(self.db.as_ref())
        .await?;

        rows.into_iter().map(SessionRow::into_session).collect()
    }

    /// Update the progress snapshot on a session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_progress_snapshot(
        &self,
        id: &str,
        snapshot: Option<Vec<ProgressItem>>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let json = snapshot
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| AppError::Db(format!("failed to serialize progress_snapshot: {e}")))?;

        sqlx::query("UPDATE session SET progress_snapshot = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&json)
            .bind(&now)
            .bind(id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }

    /// Terminate a session, setting status and `terminated_at`.
    ///
    /// Returns the updated session entity.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails or the session is not found.
    pub async fn set_terminated(&self, id: &str, status: SessionStatus) -> Result<Session> {
        let now = Utc::now().to_rfc3339();
        let status_s = status_str(status);

        sqlx::query(
            "UPDATE session SET status = ?1, terminated_at = ?2, updated_at = ?2 WHERE id = ?3",
        )
        .bind(status_s)
        .bind(&now)
        .bind(id)
        .execute(self.db.as_ref())
        .await?;

        self.get_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("session {id} not found after terminate")))
    }

    /// Count active sessions (status == `active`).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn count_active(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS cnt FROM session WHERE status = 'active'")
            .fetch_one(self.db.as_ref())
            .await?;

        let count: i64 = row.get("cnt");
        Ok(count)
    }

    /// Retrieve the most recently interrupted session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_most_recent_interrupted(&self) -> Result<Option<Session>> {
        let row: Option<SessionRow> = sqlx::query_as(
            "SELECT * FROM session WHERE status = 'interrupted' \
             ORDER BY updated_at DESC LIMIT 1",
        )
        .fetch_optional(self.db.as_ref())
        .await?;

        row.map(SessionRow::into_session).transpose()
    }

    /// List all sessions with status `interrupted`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_interrupted(&self) -> Result<Vec<Session>> {
        let rows: Vec<SessionRow> =
            sqlx::query_as("SELECT * FROM session WHERE status = 'interrupted'")
                .fetch_all(self.db.as_ref())
                .await?;

        rows.into_iter().map(SessionRow::into_session).collect()
    }

    /// List all sessions with status `active` or `paused`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_active_or_paused(&self) -> Result<Vec<Session>> {
        let rows: Vec<SessionRow> =
            sqlx::query_as("SELECT * FROM session WHERE status IN ('active', 'paused')")
                .fetch_all(self.db.as_ref())
                .await?;

        rows.into_iter().map(SessionRow::into_session).collect()
    }

    /// Update the operational mode for a session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_mode(&self, id: &str, mode: SessionMode) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mode_s = mode_str(mode);

        sqlx::query("UPDATE session SET mode = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(mode_s)
            .bind(&now)
            .bind(id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }
}
