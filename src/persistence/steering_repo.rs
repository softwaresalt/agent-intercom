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
            "INSERT INTO steering_message (id, session_id, channel_id, message, source, created_at, consumed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(&msg.id)
        .bind(&msg.session_id)
        .bind(&msg.channel_id)
        .bind(&msg.message)
        .bind(source)
        .bind(&created_at)
        .bind(i64::from(msg.consumed))
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
            "SELECT id, session_id, channel_id, message, source, created_at, consumed
             FROM steering_message
             WHERE session_id = ?1 AND consumed = 0
             ORDER BY created_at ASC",
        )
        .bind(session_id)
        .fetch_all(self.db.as_ref())
        .await?;

        rows.into_iter().map(SteeringRow::into_steering).collect()
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
