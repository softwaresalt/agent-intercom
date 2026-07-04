//! Steering message repository for `SQLite` persistence.

use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::models::steering::{SteeringMessage, SteeringSource};
use crate::{AppError, Result};

use super::db::Database;

/// Repository for steering message records.
#[derive(Clone)]
pub struct SteeringRepo {
    db: Arc<Database>,
}

/// Internal row struct for `SQLite` deserialization.
#[derive(sqlx::FromRow)]
struct SteeringRow {
    id: String,
    session_id: String,
    channel_id: Option<String>,
    message: String,
    source: String,
    created_at: String,
    consumed: i64,
    origin_session_id: Option<String>,
}

impl SteeringRow {
    fn into_steering(self) -> Result<SteeringMessage> {
        let source = parse_source(&self.source)?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|e| AppError::Db(format!("invalid created_at: {e}")))?
            .with_timezone(&Utc);

        Ok(SteeringMessage {
            id: self.id,
            session_id: self.session_id,
            channel_id: self.channel_id,
            message: self.message,
            source,
            created_at,
            consumed: self.consumed != 0,
            origin_session_id: self.origin_session_id,
        })
    }
}

fn parse_source(s: &str) -> Result<SteeringSource> {
    match s {
        "slack" => Ok(SteeringSource::Slack),
        "ipc" => Ok(SteeringSource::Ipc),
        other => Err(AppError::Db(format!("invalid steering source: {other}"))),
    }
}

fn source_str(s: SteeringSource) -> &'static str {
    match s {
        SteeringSource::Slack => "slack",
        SteeringSource::Ipc => "ipc",
    }
}

impl SteeringRepo {
    /// Create a new repository instance.
    #[must_use]
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Insert a new steering message record.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the database insert fails.
    pub async fn insert(&self, msg: &SteeringMessage) -> Result<SteeringMessage> {
        let source = source_str(msg.source);
        let created_at = msg.created_at.to_rfc3339();

        sqlx::query(
            "INSERT INTO steering_message (id, session_id, channel_id, message, source, created_at, consumed, origin_session_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&msg.id)
        .bind(&msg.session_id)
        .bind(&msg.channel_id)
        .bind(&msg.message)
        .bind(source)
        .bind(&created_at)
        .bind(i64::from(msg.consumed))
        .bind(&msg.origin_session_id)
        .execute(self.db.as_ref())
        .await?;

        Ok(msg.clone())
    }

    /// Fetch all unconsumed steering messages for a session.
    ///
    /// Returns messages ordered by creation time (oldest first).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn fetch_unconsumed(&self, session_id: &str) -> Result<Vec<SteeringMessage>> {
        let rows: Vec<SteeringRow> = sqlx::query_as(
            "SELECT id, session_id, channel_id, message, source, created_at, consumed, origin_session_id
             FROM steering_message
             WHERE session_id = ?1 AND consumed = 0
             ORDER BY created_at ASC, rowid ASC",
        )
        .bind(session_id)
        .fetch_all(self.db.as_ref())
        .await?;

        rows.into_iter().map(SteeringRow::into_steering).collect()
    }

    /// Rebind a crashed session's *unconsumed* steering messages to a resumed
    /// session so the pending queue survives a respawn (F.3-T2).
    ///
    /// Moves every unconsumed message from `from_session_id` to
    /// `to_session_id`, recording the original owning session in
    /// `origin_session_id` the first time a message is reassigned. Already
    /// consumed messages are left untouched. Returns the number of messages
    /// carried forward.
    ///
    /// This is the durable persistence primitive that the resume-state
    /// contract (F.3-T4) wires into the respawn path.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn reassign_unconsumed_to_session(
        &self,
        from_session_id: &str,
        to_session_id: &str,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE steering_message
             SET session_id = ?1,
                 origin_session_id = COALESCE(origin_session_id, ?2)
             WHERE session_id = ?2 AND consumed = 0",
        )
        .bind(to_session_id)
        .bind(from_session_id)
        .execute(self.db.as_ref())
        .await?;

        Ok(result.rows_affected())
    }

    /// Mark a steering message as consumed (delivered via `ping`).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn mark_consumed(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE steering_message SET consumed = 1 WHERE id = ?1")
            .bind(id)
            .execute(self.db.as_ref())
            .await?;
        Ok(())
    }

    /// Purge steering messages created before `before`.
    ///
    /// Returns the number of rows deleted.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the delete fails.
    pub async fn purge(&self, before: DateTime<Utc>) -> Result<u64> {
        let before_str = before.to_rfc3339();
        let result = sqlx::query("DELETE FROM steering_message WHERE created_at < ?1")
            .bind(&before_str)
            .execute(self.db.as_ref())
            .await?;
        Ok(result.rows_affected())
    }
}
