//! Continuation prompt repository for `SQLite` persistence.

use std::sync::Arc;

use chrono::Utc;

use crate::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use crate::{AppError, Result};

use super::db::Database;

/// Repository wrapper around `SQLite` for continuation prompt records.
#[derive(Clone)]
pub struct PromptRepo {
    db: Arc<Database>,
}

/// Internal row struct for `SQLite` deserialization.
#[derive(sqlx::FromRow)]
struct PromptRow {
    id: String,
    session_id: String,
    prompt_text: String,
    prompt_type: String,
    elapsed_seconds: Option<i64>,
    actions_taken: Option<i64>,
    decision: Option<String>,
    instruction: Option<String>,
    slack_ts: Option<String>,
    created_at: String,
}

impl PromptRow {
    /// Convert a database row into the domain model.
    fn into_prompt(self) -> Result<ContinuationPrompt> {
        let prompt_type = parse_prompt_type(&self.prompt_type)?;
        let decision = self.decision.as_deref().map(parse_decision).transpose()?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|e| AppError::Db(format!("invalid created_at: {e}")))?
            .with_timezone(&Utc);

        Ok(ContinuationPrompt {
            id: self.id,
            session_id: self.session_id,
            prompt_text: self.prompt_text,
            prompt_type,
            elapsed_seconds: self.elapsed_seconds,
            actions_taken: self.actions_taken,
            decision,
            instruction: self.instruction,
            slack_ts: self.slack_ts,
            created_at,
        })
    }
}

fn parse_prompt_type(s: &str) -> Result<PromptType> {
    match s {
        "continuation" => Ok(PromptType::Continuation),
        "clarification" => Ok(PromptType::Clarification),
        "error_recovery" => Ok(PromptType::ErrorRecovery),
        "resource_warning" => Ok(PromptType::ResourceWarning),
        other => Err(AppError::Db(format!("invalid prompt_type: {other}"))),
    }
}

fn prompt_type_str(t: PromptType) -> &'static str {
    match t {
        PromptType::Continuation => "continuation",
        PromptType::Clarification => "clarification",
        PromptType::ErrorRecovery => "error_recovery",
        PromptType::ResourceWarning => "resource_warning",
    }
}

fn parse_decision(s: &str) -> Result<PromptDecision> {
    match s {
        "continue" => Ok(PromptDecision::Continue),
        "refine" => Ok(PromptDecision::Refine),
        "stop" => Ok(PromptDecision::Stop),
        other => Err(AppError::Db(format!("invalid prompt decision: {other}"))),
    }
}

fn decision_str(d: PromptDecision) -> &'static str {
    match d {
        PromptDecision::Continue => "continue",
        PromptDecision::Refine => "refine",
        PromptDecision::Stop => "stop",
    }
}

impl PromptRepo {
    /// Create a new repository instance.
    #[must_use]
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Insert a new continuation prompt record.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the database insert fails.
    pub async fn create(&self, prompt: &ContinuationPrompt) -> Result<ContinuationPrompt> {
        let prompt_type = prompt_type_str(prompt.prompt_type);
        let decision = prompt.decision.map(decision_str);
        let created_at = prompt.created_at.to_rfc3339();

        sqlx::query(
            "INSERT INTO continuation_prompt (id, session_id, prompt_text, prompt_type,
             elapsed_seconds, actions_taken, decision, instruction, slack_ts, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&prompt.id)
        .bind(&prompt.session_id)
        .bind(&prompt.prompt_text)
        .bind(prompt_type)
        .bind(prompt.elapsed_seconds)
        .bind(prompt.actions_taken)
        .bind(decision)
        .bind(&prompt.instruction)
        .bind(&prompt.slack_ts)
        .bind(&created_at)
        .execute(self.db.as_ref())
        .await?;

        Ok(prompt.clone())
    }

    /// Retrieve a prompt by identifier.
    ///
    /// Returns `Ok(None)` if the prompt does not exist.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<ContinuationPrompt>> {
        let row: Option<PromptRow> =
            sqlx::query_as("SELECT * FROM continuation_prompt WHERE id = ?1")
                .bind(id)
                .fetch_optional(self.db.as_ref())
                .await?;

        row.map(PromptRow::into_prompt).transpose()
    }

    /// Retrieve the pending prompt for a session, if any.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn get_pending_for_session(
        &self,
        session_id: &str,
    ) -> Result<Option<ContinuationPrompt>> {
        let row: Option<PromptRow> = sqlx::query_as(
            "SELECT * FROM continuation_prompt \
             WHERE session_id = ?1 AND decision IS NULL LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(self.db.as_ref())
        .await?;

        row.map(PromptRow::into_prompt).transpose()
    }

    /// Update the decision and optional instruction on a prompt.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the update fails.
    pub async fn update_decision(
        &self,
        id: &str,
        decision: PromptDecision,
        instruction: Option<String>,
    ) -> Result<()> {
        let decision_s = decision_str(decision);

        sqlx::query("UPDATE continuation_prompt SET decision = ?1, instruction = ?2 WHERE id = ?3")
            .bind(decision_s)
            .bind(&instruction)
            .bind(id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }

    /// List all pending prompts (no decision yet) across sessions.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Db` if the query fails.
    pub async fn list_pending(&self) -> Result<Vec<ContinuationPrompt>> {
        let rows: Vec<PromptRow> =
            sqlx::query_as("SELECT * FROM continuation_prompt WHERE decision IS NULL")
                .fetch_all(self.db.as_ref())
                .await?;

        rows.into_iter().map(PromptRow::into_prompt).collect()
    }
}
