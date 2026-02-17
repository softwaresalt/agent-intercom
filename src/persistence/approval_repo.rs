//! Approval request repository for `SQLite` persistence.

use std::sync::Arc;

use chrono::Utc;

use crate::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use crate::{AppError, Result};

use super::db::Database;

/// Repository wrapper around `SQLite` for approval request records.
#[derive(Clone)]
pub struct ApprovalRepo {
    db: Arc<Database>,
}

/// Internal row struct for `SQLite` deserialization.
#[derive(sqlx::FromRow)]
struct ApprovalRow {
    id: String,
    session_id: String,
    title: String,
    description: Option<String>,
    diff_content: String,
    file_path: String,
    risk_level: String,
    status: String,
    original_hash: String,
    slack_ts: Option<String>,
    created_at: String,
    consumed_at: Option<String>,
}

impl ApprovalRow {
    /// Convert a database row into the domain model.
    fn into_approval(self) -> Result<ApprovalRequest> {
        let risk_level = parse_risk_level(&self.risk_level)?;
        let status = parse_approval_status(&self.status)?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|e| AppError::Db(format!("invalid created_at: {e}")))?
            .with_timezone(&Utc);
        let consumed_at = self
            .consumed_at
            .as_deref()
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| AppError::Db(format!("invalid consumed_at: {e}")))
            })
            .transpose()?;

        Ok(ApprovalRequest {
            id: self.id,
            session_id: self.session_id,
            title: self.title,
            description: self.description,
            diff_content: self.diff_content,
            file_path: self.file_path,
            risk_level,
            status,
            original_hash: self.original_hash,
            slack_ts: self.slack_ts,
            created_at,
            consumed_at,
        })
    }
}

fn parse_risk_level(s: &str) -> Result<RiskLevel> {
    match s {
        "low" => Ok(RiskLevel::Low),
        "high" => Ok(RiskLevel::High),
        "critical" => Ok(RiskLevel::Critical),
        other => Err(AppError::Db(format!("invalid risk_level: {other}"))),
    }
}

fn risk_level_str(r: RiskLevel) -> &'static str {
    match r {
        RiskLevel::Low => "low",
        RiskLevel::High => "high",
        RiskLevel::Critical => "critical",
    }
}

fn parse_approval_status(s: &str) -> Result<ApprovalStatus> {
    match s {
        "pending" => Ok(ApprovalStatus::Pending),
        "approved" => Ok(ApprovalStatus::Approved),
        "rejected" => Ok(ApprovalStatus::Rejected),
        "expired" => Ok(ApprovalStatus::Expired),
        "consumed" => Ok(ApprovalStatus::Consumed),
        "interrupted" => Ok(ApprovalStatus::Interrupted),
        other => Err(AppError::Db(format!("invalid approval status: {other}"))),
    }
}

fn approval_status_str(s: ApprovalStatus) -> &'static str {
    match s {
        ApprovalStatus::Pending => "pending",
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Rejected => "rejected",
        ApprovalStatus::Expired => "expired",
        ApprovalStatus::Consumed => "consumed",
        ApprovalStatus::Interrupted => "interrupted",
    }
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
        let risk_level = risk_level_str(request.risk_level);
        let status = approval_status_str(request.status);
        let created_at = request.created_at.to_rfc3339();
        let consumed_at = request.consumed_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            "INSERT INTO approval_request (id, session_id, title, description, diff_content,
             file_path, risk_level, status, original_hash, slack_ts, created_at, consumed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )
        .bind(&request.id)
        .bind(&request.session_id)
        .bind(&request.title)
        .bind(&request.description)
        .bind(&request.diff_content)
        .bind(&request.file_path)
        .bind(risk_level)
        .bind(status)
        .bind(&request.original_hash)
        .bind(&request.slack_ts)
        .bind(&created_at)
        .bind(&consumed_at)
        .execute(self.db.as_ref())
        .await?;

        Ok(request.clone())
    }

    /// Retrieve an approval request by identifier.
    ///
    /// Returns `Ok(None)` if the request does not exist.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<ApprovalRequest>> {
        let row: Option<ApprovalRow> =
            sqlx::query_as("SELECT * FROM approval_request WHERE id = ?1")
                .bind(id)
                .fetch_optional(self.db.as_ref())
                .await?;

        row.map(ApprovalRow::into_approval).transpose()
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
        let row: Option<ApprovalRow> = sqlx::query_as(
            "SELECT * FROM approval_request WHERE session_id = ?1 AND status = 'pending' LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(self.db.as_ref())
        .await?;

        row.map(ApprovalRow::into_approval).transpose()
    }

    /// Update the status of an approval request.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_status(&self, id: &str, status: ApprovalStatus) -> Result<()> {
        let status_s = approval_status_str(status);

        sqlx::query("UPDATE approval_request SET status = ?1 WHERE id = ?2")
            .bind(status_s)
            .bind(id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }

    /// Mark an approved request as consumed with a timestamp.
    ///
    /// # Errors
    ///
    /// Returns `AppError::AlreadyConsumed` if the request was previously consumed.
    /// Returns `AppError::Db` if the status is not `Approved`.
    pub async fn mark_consumed(&self, id: &str) -> Result<()> {
        let current = self
            .get_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("approval request {id} not found")))?;

        if current.status == ApprovalStatus::Consumed {
            return Err(AppError::AlreadyConsumed(format!(
                "approval request {id} already consumed"
            )));
        }
        if current.status != ApprovalStatus::Approved {
            return Err(AppError::Db(format!(
                "cannot consume approval in status {}",
                approval_status_str(current.status)
            )));
        }

        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE approval_request SET status = 'consumed', consumed_at = ?1 WHERE id = ?2",
        )
        .bind(&now)
        .bind(id)
        .execute(self.db.as_ref())
        .await?;

        Ok(())
    }

    /// List all pending approval requests across sessions.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_pending(&self) -> Result<Vec<ApprovalRequest>> {
        let rows: Vec<ApprovalRow> =
            sqlx::query_as("SELECT * FROM approval_request WHERE status = 'pending'")
                .fetch_all(self.db.as_ref())
                .await?;

        rows.into_iter().map(ApprovalRow::into_approval).collect()
    }
}
