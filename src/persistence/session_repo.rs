//! Session repository for `SQLite` persistence.

use std::sync::Arc;

use chrono::Utc;
use sqlx::Row;

use crate::models::progress::ProgressItem;
use crate::models::session::{
    ConnectivityStatus, ProtocolMode, Session, SessionMode, SessionStatus,
};
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
    protocol_mode: String,
    channel_id: Option<String>,
    thread_ts: Option<String>,
    connectivity_status: String,
    last_activity_at: Option<String>,
    restart_of: Option<String>,
    agent_session_id: Option<String>,
    title: Option<String>,
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

        let protocol_mode = parse_protocol_mode(&self.protocol_mode)?;
        let connectivity_status = parse_connectivity_status(&self.connectivity_status)?;
        let last_activity_at = self
            .last_activity_at
            .as_deref()
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| AppError::Db(format!("invalid last_activity_at: {e}")))
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
            protocol_mode,
            channel_id: self.channel_id,
            thread_ts: self.thread_ts,
            connectivity_status,
            last_activity_at,
            restart_of: self.restart_of,
            agent_session_id: self.agent_session_id,
            title: self.title,
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

/// Parse a protocol mode string from the database.
fn parse_protocol_mode(s: &str) -> Result<ProtocolMode> {
    match s {
        "mcp" => Ok(ProtocolMode::Mcp),
        "acp" => Ok(ProtocolMode::Acp),
        other => Err(AppError::Db(format!("invalid protocol_mode: {other}"))),
    }
}

/// Serialize a protocol mode enum to its database string.
fn protocol_mode_str(m: ProtocolMode) -> &'static str {
    match m {
        ProtocolMode::Mcp => "mcp",
        ProtocolMode::Acp => "acp",
    }
}

/// Parse a connectivity status string from the database.
fn parse_connectivity_status(s: &str) -> Result<ConnectivityStatus> {
    match s {
        "online" => Ok(ConnectivityStatus::Online),
        "offline" => Ok(ConnectivityStatus::Offline),
        "stalled" => Ok(ConnectivityStatus::Stalled),
        other => Err(AppError::Db(format!(
            "invalid connectivity_status: {other}"
        ))),
    }
}

/// Serialize a connectivity status enum to its database string.
fn connectivity_status_str(c: ConnectivityStatus) -> &'static str {
    match c {
        ConnectivityStatus::Online => "online",
        ConnectivityStatus::Offline => "offline",
        ConnectivityStatus::Stalled => "stalled",
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
        let protocol_mode = protocol_mode_str(session.protocol_mode);
        let connectivity_status = connectivity_status_str(session.connectivity_status);
        let last_activity_at = session.last_activity_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            "INSERT INTO session (id, owner_user_id, workspace_root, status, prompt, mode,
             created_at, updated_at, terminated_at, last_tool, nudge_count, stall_paused,
             progress_snapshot, protocol_mode, channel_id, thread_ts, connectivity_status,
             last_activity_at, restart_of, agent_session_id, title)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
             ?17, ?18, ?19, ?20, ?21)",
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
        .bind(protocol_mode)
        .bind(&session.channel_id)
        .bind(&session.thread_ts)
        .bind(connectivity_status)
        .bind(&last_activity_at)
        .bind(&session.restart_of)
        .bind(&session.agent_session_id)
        .bind(&session.title)
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

    /// Retrieve a session by ID prefix.
    ///
    /// Matches sessions whose ID starts with the given prefix. Returns
    /// `Ok(None)` if no match exists, or `Err` if multiple sessions match
    /// (ambiguous prefix).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` on query failure or `AppError::Config` if the
    /// prefix matches more than one session.
    pub async fn get_by_prefix(&self, prefix: &str) -> Result<Option<Session>> {
        let pattern = format!("{prefix}%");
        let rows: Vec<SessionRow> =
            sqlx::query_as("SELECT * FROM session WHERE id LIKE ?1 ORDER BY updated_at DESC")
                .bind(&pattern)
                .fetch_all(self.db.as_ref())
                .await?;

        match rows.len() {
            0 => Ok(None),
            1 => rows
                .into_iter()
                .next()
                .map(SessionRow::into_session)
                .transpose(),
            n => Err(AppError::Config(format!(
                "ambiguous session prefix '{prefix}' matches {n} sessions"
            ))),
        }
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

        let result = sqlx::query(
            "UPDATE session SET status = ?1, terminated_at = ?2, updated_at = ?2 WHERE id = ?3",
        )
        .bind(status_s)
        .bind(&now)
        .bind(id)
        .execute(self.db.as_ref())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "set_terminated: no rows updated for session {id}"
            )));
        }

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

    /// Return `(session_id, last_activity_at)` for all active sessions (T148, FR-045).
    ///
    /// Used by the server startup path to seed stall-detector timers from
    /// persisted `last_activity_at` timestamps so stall detection resumes
    /// correctly after a restart.  `last_activity_at` is `None` when the
    /// session has not yet recorded any activity.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn load_active_session_timestamps(&self) -> Result<Vec<(String, Option<String>)>> {
        let rows: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT id, last_activity_at FROM session WHERE status = 'active'")
                .fetch_all(self.db.as_ref())
                .await?;

        Ok(rows)
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

    /// Return all active sessions associated with a Slack channel.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn find_active_by_channel(&self, channel_id: &str) -> Result<Vec<Session>> {
        let rows: Vec<SessionRow> = sqlx::query_as(
            "SELECT * FROM session WHERE channel_id = ?1 AND status IN ('created', 'active',
             'paused')",
        )
        .bind(channel_id)
        .fetch_all(self.db.as_ref())
        .await?;

        rows.into_iter().map(SessionRow::into_session).collect()
    }

    /// Return all interrupted sessions associated with a Slack channel (HITL-006).
    ///
    /// Used as a fallback when `find_active_by_channel` returns no results,
    /// allowing operators to manage sessions that became `Interrupted` after a
    /// server restart (e.g., via `session-stop`, `session-cleanup`).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn find_interrupted_by_channel(&self, channel_id: &str) -> Result<Vec<Session>> {
        let rows: Vec<SessionRow> = sqlx::query_as(
            "SELECT * FROM session WHERE channel_id = ?1 AND status = 'interrupted' \
             ORDER BY updated_at DESC",
        )
        .bind(channel_id)
        .fetch_all(self.db.as_ref())
        .await?;

        rows.into_iter().map(SessionRow::into_session).collect()
    }

    /// Find a session by Slack channel and thread timestamp.
    ///
    /// Returns `None` if no matching session exists.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn find_by_channel_and_thread(
        &self,
        channel_id: &str,
        thread_ts: &str,
    ) -> Result<Option<Session>> {
        let row: Option<SessionRow> =
            sqlx::query_as("SELECT * FROM session WHERE channel_id = ?1 AND thread_ts = ?2")
                .bind(channel_id)
                .bind(thread_ts)
                .fetch_optional(self.db.as_ref())
                .await?;

        row.map(SessionRow::into_session).transpose()
    }

    /// Set the connectivity status of a session.
    ///
    /// Used by the ACP reader on connect (`Online`) and by the stall detector
    /// on inactivity (`Stalled`) to track the agent's reachability state.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn set_connectivity_status(
        &self,
        id: &str,
        status: ConnectivityStatus,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let status_s = connectivity_status_str(status);

        sqlx::query("UPDATE session SET connectivity_status = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(status_s)
            .bind(&now)
            .bind(id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }

    /// Set the Slack thread timestamp for a session.
    ///
    /// This is a write-once field: subsequent calls are a no-op if `thread_ts`
    /// is already set. Callers that need to update an existing value should use
    /// a direct query instead.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn set_thread_ts(&self, session_id: &str, thread_ts: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE session SET thread_ts = ?1, updated_at = ?2
             WHERE id = ?3 AND thread_ts IS NULL",
        )
        .bind(thread_ts)
        .bind(&now)
        .bind(session_id)
        .execute(self.db.as_ref())
        .await?;

        Ok(())
    }

    /// Set the ACP agent-assigned session ID.
    ///
    /// Persists the `sessionId` returned by the ACP `session/new` handshake so
    /// that subsequent `session/prompt` messages can include the correct ID.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn set_agent_session_id(
        &self,
        session_id: &str,
        agent_session_id: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query("UPDATE session SET agent_session_id = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(agent_session_id)
            .bind(&now)
            .bind(session_id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }

    /// List all sessions associated with a Slack channel, regardless of status.
    ///
    /// Used by `/arc sessions --all` (HITL-002 / FR-048) to show the complete
    /// session history for the current channel, including terminated and
    /// interrupted sessions.
    ///
    /// Results are ordered by `updated_at` descending (most recent first).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_all_by_channel(&self, channel_id: &str) -> Result<Vec<Session>> {
        let rows: Vec<SessionRow> =
            sqlx::query_as("SELECT * FROM session WHERE channel_id = ?1 ORDER BY updated_at DESC")
                .bind(channel_id)
                .fetch_all(self.db.as_ref())
                .await?;

        rows.into_iter().map(SessionRow::into_session).collect()
    }
}
