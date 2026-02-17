//! Stall alert repository for `SQLite` persistence.

use std::sync::Arc;

use chrono::Utc;

use crate::models::stall::{StallAlert, StallAlertStatus};
use crate::{AppError, Result};

use super::db::Database;

/// Repository wrapper around `SQLite` for stall alert records.
#[derive(Clone)]
pub struct StallAlertRepo {
    db: Arc<Database>,
}

/// Internal row struct for `SQLite` deserialization.
#[derive(sqlx::FromRow)]
struct StallAlertRow {
    id: String,
    session_id: String,
    last_tool: Option<String>,
    last_activity_at: String,
    idle_seconds: i64,
    nudge_count: i64,
    status: String,
    nudge_message: Option<String>,
    progress_snapshot: Option<String>,
    slack_ts: Option<String>,
    created_at: String,
}

impl StallAlertRow {
    /// Convert a database row into the domain model.
    fn into_stall_alert(self) -> Result<StallAlert> {
        let status = parse_stall_status(&self.status)?;
        let last_activity_at = chrono::DateTime::parse_from_rfc3339(&self.last_activity_at)
            .map_err(|e| AppError::Db(format!("invalid last_activity_at: {e}")))?
            .with_timezone(&Utc);
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|e| AppError::Db(format!("invalid created_at: {e}")))?
            .with_timezone(&Utc);
        let progress_snapshot = self
            .progress_snapshot
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| AppError::Db(format!("invalid progress_snapshot: {e}")))?;

        Ok(StallAlert {
            id: self.id,
            session_id: self.session_id,
            last_tool: self.last_tool,
            last_activity_at,
            idle_seconds: self.idle_seconds,
            nudge_count: self.nudge_count,
            status,
            nudge_message: self.nudge_message,
            progress_snapshot,
            slack_ts: self.slack_ts,
            created_at,
        })
    }
}

fn parse_stall_status(s: &str) -> Result<StallAlertStatus> {
    match s {
        "pending" => Ok(StallAlertStatus::Pending),
        "nudged" => Ok(StallAlertStatus::Nudged),
        "self_recovered" => Ok(StallAlertStatus::SelfRecovered),
        "escalated" => Ok(StallAlertStatus::Escalated),
        "dismissed" => Ok(StallAlertStatus::Dismissed),
        other => Err(AppError::Db(format!("invalid stall alert status: {other}"))),
    }
}

fn stall_status_str(s: StallAlertStatus) -> &'static str {
    match s {
        StallAlertStatus::Pending => "pending",
        StallAlertStatus::Nudged => "nudged",
        StallAlertStatus::SelfRecovered => "self_recovered",
        StallAlertStatus::Escalated => "escalated",
        StallAlertStatus::Dismissed => "dismissed",
    }
}

impl StallAlertRepo {
    /// Create a new repository instance.
    #[must_use]
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Insert a new stall alert record.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the database insert fails.
    pub async fn create(&self, alert: &StallAlert) -> Result<StallAlert> {
        let status = stall_status_str(alert.status);
        let last_activity_at = alert.last_activity_at.to_rfc3339();
        let created_at = alert.created_at.to_rfc3339();
        let progress_snapshot = alert
            .progress_snapshot
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| AppError::Db(format!("serialize progress_snapshot: {e}")))?;

        sqlx::query(
            "INSERT INTO stall_alert (id, session_id, last_tool, last_activity_at,
             idle_seconds, nudge_count, status, nudge_message, progress_snapshot,
             slack_ts, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )
        .bind(&alert.id)
        .bind(&alert.session_id)
        .bind(&alert.last_tool)
        .bind(&last_activity_at)
        .bind(alert.idle_seconds)
        .bind(alert.nudge_count)
        .bind(status)
        .bind(&alert.nudge_message)
        .bind(&progress_snapshot)
        .bind(&alert.slack_ts)
        .bind(&created_at)
        .execute(self.db.as_ref())
        .await?;

        Ok(alert.clone())
    }

    /// Retrieve a stall alert by its ID.
    ///
    /// Returns `Ok(None)` if the alert does not exist.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<StallAlert>> {
        let row: Option<StallAlertRow> = sqlx::query_as("SELECT * FROM stall_alert WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.db.as_ref())
            .await?;

        row.map(StallAlertRow::into_stall_alert).transpose()
    }

    /// Retrieve the active (`pending` or `nudged`) stall alert for a session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_active_for_session(&self, session_id: &str) -> Result<Option<StallAlert>> {
        let row: Option<StallAlertRow> = sqlx::query_as(
            "SELECT * FROM stall_alert \
             WHERE session_id = ?1 AND status IN ('pending', 'nudged') LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(self.db.as_ref())
        .await?;

        row.map(StallAlertRow::into_stall_alert).transpose()
    }

    /// Update the status of a stall alert.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_status(&self, id: &str, status: StallAlertStatus) -> Result<()> {
        let status_s = stall_status_str(status);

        sqlx::query("UPDATE stall_alert SET status = ?1 WHERE id = ?2")
            .bind(status_s)
            .bind(id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }

    /// Increment the nudge count on an alert and set status to `Nudged`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn increment_nudge_count(&self, id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE stall_alert SET nudge_count = nudge_count + 1, status = 'nudged' \
             WHERE id = ?1",
        )
        .bind(id)
        .execute(self.db.as_ref())
        .await?;

        Ok(())
    }

    /// Dismiss a stall alert by setting its status to `Dismissed`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn dismiss(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE stall_alert SET status = 'dismissed' WHERE id = ?1")
            .bind(id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }
}
