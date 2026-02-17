//! Approval request repository for `SQLite` persistence.

use std::sync::Arc;

use crate::models::approval::{ApprovalRequest, ApprovalStatus};
use crate::Result;

use super::db::Database;

/// Repository wrapper around `SQLite` for approval request records.
#[derive(Clone)]
pub struct ApprovalRepo {
    db: Arc<Database>,
}

#[allow(clippy::unused_async)] // todo!() stubs lack .await — Phase 3 will add real queries
impl ApprovalRepo {
    /// Create a new repository instance.
    #[must_use]
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Insert a new approval request record.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the database insert fails.
    pub async fn create(&self, _request: &ApprovalRequest) -> Result<ApprovalRequest> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T024)")
    }

    /// Retrieve an approval request by identifier.
    ///
    /// # Errors
    ///
    /// Returns `AppError::NotFound` if the request does not exist.
    pub async fn get_by_id(&self, _id: &str) -> Result<ApprovalRequest> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T024)")
    }

    /// Retrieve the pending approval request for a session, if any.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_pending_for_session(
        &self,
        _session_id: &str,
    ) -> Result<Option<ApprovalRequest>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T024)")
    }

    /// Update the status of an approval request.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_status(
        &self,
        _id: &str,
        _status: ApprovalStatus,
    ) -> Result<ApprovalRequest> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T024)")
    }

    /// Mark an approved request as consumed with a timestamp.
    ///
    /// # Errors
    ///
    /// Returns `AppError::AlreadyConsumed` if the request was previously consumed.
    /// Returns `AppError::Db` if the status is not `Approved`.
    pub async fn mark_consumed(&self, _id: &str) -> Result<ApprovalRequest> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T024)")
    }

    /// List all pending approval requests across sessions.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_pending(&self) -> Result<Vec<ApprovalRequest>> {
        let _ = &self.db;
        todo!("rewrite with sqlx in Phase 3 (T024)")
    }
}
