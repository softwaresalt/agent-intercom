//! Continuation prompt repository for `SurrealDB` persistence.

use std::sync::Arc;

use crate::models::prompt::{ContinuationPrompt, PromptDecision};
use crate::{AppError, Result};

use super::db::Database;

/// Repository wrapper around `SurrealDB` for continuation prompt records.
#[derive(Clone)]
pub struct PromptRepo {
    db: Arc<Database>,
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
        self.db
            .create(("continuation_prompt", prompt.id.as_str()))
            .content(prompt)
            .await?
            .ok_or_else(|| AppError::Db("failed to create continuation prompt".into()))
    }

    /// Retrieve a prompt by identifier.
    ///
    /// # Errors
    ///
    /// Returns `AppError::NotFound` if the prompt does not exist.
    pub async fn get_by_id(&self, id: &str) -> Result<ContinuationPrompt> {
        let prompt: Option<ContinuationPrompt> =
            self.db.select(("continuation_prompt", id)).await?;
        prompt.ok_or_else(|| AppError::NotFound("continuation prompt not found".into()))
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
        let mut response = self
            .db
            .query(
                "SELECT * FROM continuation_prompt \
                 WHERE session_id = $sid AND decision IS NONE \
                 LIMIT 1",
            )
            .bind(("sid", session_id))
            .await?;
        let prompts: Vec<ContinuationPrompt> = response.take(0)?;
        Ok(prompts.into_iter().next())
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
    ) -> Result<ContinuationPrompt> {
        let mut current = self.get_by_id(id).await?;
        current.decision = Some(decision);
        current.instruction = instruction;
        self.db
            .update(("continuation_prompt", id))
            .content(&current)
            .await?
            .ok_or_else(|| AppError::Db("failed to update prompt decision".into()))
    }
}
