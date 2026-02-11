//! Approval request repository for `SurrealDB` persistence.

use std::sync::Arc;

use chrono::Utc;

use crate::models::approval::{ApprovalRequest, ApprovalStatus};
use crate::{AppError, Result};

use super::db::Database;

/// Repository wrapper around `SurrealDB` for approval request records.
#[derive(Clone)]
pub struct ApprovalRepo {
    db: Arc<Database>,
}

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
    pub async fn create(&self, request: &ApprovalRequest) -> Result<ApprovalRequest> {
        self.db
            .create(("approval_request", request.id.clone()))
            .content(request)
            .await?
            .ok_or_else(|| AppError::Db("failed to create approval request".into()))
    }

    /// Retrieve an approval request by identifier.
    ///
    /// # Errors
    ///
    /// Returns `AppError::NotFound` if the request does not exist.
    pub async fn get_by_id(&self, id: &str) -> Result<ApprovalRequest> {
        let request: Option<ApprovalRequest> =
            self.db.select(("approval_request", id)).await?;
        request.ok_or_else(|| AppError::NotFound("approval request not found".into()))
    }

    /// Retrieve the pending approval request for a session, if any.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_pending_for_session(
        &self,
        session_id: &str,
    ) -> Result<Option<ApprovalRequest>> {
        let mut response = self
            .db
            .query(
                "SELECT * FROM approval_request \
                 WHERE session_id = $sid AND status = 'pending' \
                 LIMIT 1",
            )
            .bind(("sid", session_id))
            .await?;
        let requests: Vec<ApprovalRequest> = response.take(0)?;
        Ok(requests.into_iter().next())
    }

    /// Update the status of an approval request.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_status(
        &self,
        id: &str,
        status: ApprovalStatus,
    ) -> Result<ApprovalRequest> {
        let mut current = self.get_by_id(id).await?;
        current.status = status;
        self.db
            .update(("approval_request", id))
            .content(&current)
            .await?
            .ok_or_else(|| AppError::Db("failed to update approval request status".into()))
    }

    /// Mark an approved request as consumed with a timestamp.
    ///
    /// # Errors
    ///
    /// Returns `AppError::AlreadyConsumed` if the request was previously consumed.
    /// Returns `AppError::Db` if the status is not `Approved`.
    pub async fn mark_consumed(&self, id: &str) -> Result<ApprovalRequest> {
        let mut current = self.get_by_id(id).await?;
        if current.status == ApprovalStatus::Consumed {
            return Err(AppError::AlreadyConsumed(
                "approval request already consumed".into(),
            ));
        }
        if current.status != ApprovalStatus::Approved {
            return Err(AppError::Db(
                "only approved requests can be consumed".into(),
            ));
        }
        current.status = ApprovalStatus::Consumed;
        current.consumed_at = Some(Utc::now());
        self.db
            .update(("approval_request", id))
            .content(&current)
            .await?
            .ok_or_else(|| AppError::Db("failed to mark approval consumed".into()))
    }

    /// List all pending approval requests across sessions.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_pending(&self) -> Result<Vec<ApprovalRequest>> {
        let mut response = self
            .db
            .query("SELECT * FROM approval_request WHERE status = 'pending'")
            .await?;
        response.take::<Vec<ApprovalRequest>>(0).map_err(AppError::from)
    }
}
