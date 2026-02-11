//! Stall alert repository for `SurrealDB` persistence.

use std::sync::Arc;

use crate::models::stall::{StallAlert, StallAlertStatus};
use crate::{AppError, Result};

use super::db::Database;

/// Repository wrapper around `SurrealDB` for stall alert records.
#[derive(Clone)]
pub struct StallAlertRepo {
    db: Arc<Database>,
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
        self.db
            .create(("stall_alert", alert.id.as_str()))
            .content(alert)
            .await?
            .ok_or_else(|| AppError::Db("failed to create stall alert".into()))
    }

    /// Retrieve the active (`pending` or `nudged`) stall alert for a session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_active_for_session(&self, session_id: &str) -> Result<Option<StallAlert>> {
        let mut response = self
            .db
            .query(
                "SELECT * FROM stall_alert \
                 WHERE session_id = $sid \
                   AND (status = 'pending' OR status = 'nudged') \
                 LIMIT 1",
            )
            .bind(("sid", session_id))
            .await?;
        let alerts: Vec<StallAlert> = response.take(0)?;
        Ok(alerts.into_iter().next())
    }

    /// Update the status of a stall alert.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_status(&self, id: &str, status: StallAlertStatus) -> Result<StallAlert> {
        let mut current: StallAlert = self
            .db
            .select(("stall_alert", id))
            .await?
            .ok_or_else(|| AppError::NotFound("stall alert not found".into()))?;

        current.status = status;
        self.db
            .update(("stall_alert", id))
            .content(&current)
            .await?
            .ok_or_else(|| AppError::Db("failed to update stall alert status".into()))
    }

    /// Increment the nudge count on an alert and set status to `Nudged`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn increment_nudge_count(&self, id: &str) -> Result<StallAlert> {
        let mut current: StallAlert = self
            .db
            .select(("stall_alert", id))
            .await?
            .ok_or_else(|| AppError::NotFound("stall alert not found".into()))?;

        current.nudge_count += 1;
        current.status = StallAlertStatus::Nudged;
        self.db
            .update(("stall_alert", id))
            .content(&current)
            .await?
            .ok_or_else(|| AppError::Db("failed to increment nudge count".into()))
    }

    /// Dismiss a stall alert by setting its status to `Dismissed`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn dismiss(&self, id: &str) -> Result<StallAlert> {
        self.update_status(id, StallAlertStatus::Dismissed).await
    }
}
