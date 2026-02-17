//! Stall alert repository for `SQLite` persistence.

use std::sync::Arc;

use crate::models::stall::{StallAlert, StallAlertStatus};
use crate::Result;

use super::db::Database;

/// Repository wrapper around `SQLite` for stall alert records.
#[derive(Clone)]
pub struct StallAlertRepo {
    db: Arc<Database>,
}

#[allow(clippy::unused_async)] // todo!() stubs lack .await — Phase 3 will add real queries
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
    pub async fn create(&self, _alert: &StallAlert) -> Result<StallAlert> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T027)")
    }

    /// Retrieve a stall alert by its ID.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_by_id(&self, _id: &str) -> Result<Option<StallAlert>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T027)")
    }

    /// Retrieve the active (`pending` or `nudged`) stall alert for a session.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_active_for_session(&self, _session_id: &str) -> Result<Option<StallAlert>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T027)")
    }

    /// Update the status of a stall alert.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_status(&self, _id: &str, _status: StallAlertStatus) -> Result<StallAlert> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T027)")
    }

    /// Increment the nudge count on an alert and set status to `Nudged`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn increment_nudge_count(&self, _id: &str) -> Result<StallAlert> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T027)")
    }

    /// Dismiss a stall alert by setting its status to `Dismissed`.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn dismiss(&self, _id: &str) -> Result<StallAlert> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T027)")
    }
}
