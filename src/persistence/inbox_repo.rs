//! Task inbox repository for `SQLite` persistence.

use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::models::inbox::{InboxSource, TaskInboxItem};
use crate::{AppError, Result};

use super::db::Database;

/// Repository for task inbox records.
#[derive(Clone)]
pub struct InboxRepo {
    db: Arc<Database>,
}

/// Internal row struct for `SQLite` deserialization.
#[derive(sqlx::FromRow)]
struct InboxRow {
    id: String,
    channel_id: Option<String>,
    message: String,
    source: String,
    created_at: String,
    consumed: i64,
}

impl InboxRow {
    fn into_item(self) -> Result<TaskInboxItem> {
        let source = parse_source(&self.source)?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|e| AppError::Db(format!("invalid created_at: {e}")))?
            .with_timezone(&Utc);

        Ok(TaskInboxItem {
            id: self.id,
            channel_id: self.channel_id,
            message: self.message,
            source,
            created_at,
            consumed: self.consumed != 0,
        })
    }
}

fn parse_source(s: &str) -> Result<InboxSource> {
    match s {
        "slack" => Ok(InboxSource::Slack),
        "ipc" => Ok(InboxSource::Ipc),
        other => Err(AppError::Db(format!("invalid inbox source: {other}"))),
    }
}

fn source_str(source: InboxSource) -> &'static str {
    match source {
        InboxSource::Slack => "slack",
        InboxSource::Ipc => "ipc",
    }
}

impl InboxRepo {
    /// Create a new repository instance.
    #[must_use]
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Insert a new task inbox item.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the database insert fails.
    pub async fn insert(&self, item: &TaskInboxItem) -> Result<TaskInboxItem> {
        let source = source_str(item.source);
        let created_at = item.created_at.to_rfc3339();

        sqlx::query(
            "INSERT INTO task_inbox (id, channel_id, message, source, created_at, consumed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&item.id)
        .bind(&item.channel_id)
        .bind(&item.message)
        .bind(source)
        .bind(&created_at)
        .bind(i64::from(item.consumed))
        .execute(self.db.as_ref())
        .await?;

        Ok(item.clone())
    }

    /// Fetch all unconsumed task inbox items scoped to `channel_id`.
    ///
    /// When `channel_id` is `Some`, returns items with a matching channel or
    /// items with no channel set (`NULL`). When `channel_id` is `None`,
    /// returns items with no channel set only.
    ///
    /// Results are ordered by creation time (oldest first).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn fetch_unconsumed_by_channel(
        &self,
        channel_id: Option<&str>,
    ) -> Result<Vec<TaskInboxItem>> {
        let rows: Vec<InboxRow> = if let Some(cid) = channel_id {
            sqlx::query_as(
                "SELECT id, channel_id, message, source, created_at, consumed
                 FROM task_inbox
                 WHERE consumed = 0 AND (channel_id = ?1 OR channel_id IS NULL)
                 ORDER BY created_at ASC",
            )
            .bind(cid)
            .fetch_all(self.db.as_ref())
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, channel_id, message, source, created_at, consumed
                 FROM task_inbox
                 WHERE consumed = 0 AND channel_id IS NULL
                 ORDER BY created_at ASC",
            )
            .fetch_all(self.db.as_ref())
            .await?
        };

        rows.into_iter().map(InboxRow::into_item).collect()
    }

    /// Mark a task inbox item as consumed (delivered at session start).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn mark_consumed(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE task_inbox SET consumed = 1 WHERE id = ?1")
            .bind(id)
            .execute(self.db.as_ref())
            .await?;
        Ok(())
    }

    /// Purge task inbox items created before `before`.
    ///
    /// Returns the number of rows deleted.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the delete fails.
    pub async fn purge(&self, before: DateTime<Utc>) -> Result<u64> {
        let before_str = before.to_rfc3339();
        let result = sqlx::query("DELETE FROM task_inbox WHERE created_at < ?1")
            .bind(&before_str)
            .execute(self.db.as_ref())
            .await?;
        Ok(result.rows_affected())
    }
}
