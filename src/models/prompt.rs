//! Continuation prompt model for forwarded agent prompts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Category of a continuation prompt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromptType {
    /// Standard continuation prompt.
    Continuation,
    /// Agent needs clarification from operator.
    Clarification,
    /// Agent encountered an error and seeks guidance.
    ErrorRecovery,
    /// Agent is warning about resource constraints.
    ResourceWarning,
}

/// Operator decision on a forwarded prompt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromptDecision {
    /// Continue with current task.
    Continue,
    /// Refine the task with revised instructions.
    Refine,
    /// Stop the current task.
    Stop,
}

/// A forwarded meta-prompt from an agent requiring operator decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ContinuationPrompt {
    /// Unique record identifier.
    pub id: String,
    /// Owning session identifier.
    pub session_id: String,
    /// Raw text of the continuation prompt.
    pub prompt_text: String,
    /// Category of the prompt.
    pub prompt_type: PromptType,
    /// Seconds since last user interaction.
    pub elapsed_seconds: Option<i64>,
    /// Count of actions performed in this iteration.
    pub actions_taken: Option<i64>,
    /// Operator's response decision.
    pub decision: Option<PromptDecision>,
    /// Revised instruction text (when decision is `Refine`).
    pub instruction: Option<String>,
    /// Slack message timestamp.
    pub slack_ts: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl ContinuationPrompt {
    /// Construct a new pending continuation prompt.
    #[must_use]
    pub fn new(
        session_id: String,
        prompt_text: String,
        prompt_type: PromptType,
        elapsed_seconds: Option<i64>,
        actions_taken: Option<i64>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id,
            prompt_text,
            prompt_type,
            elapsed_seconds,
            actions_taken,
            decision: None,
            instruction: None,
            slack_ts: None,
            created_at: Utc::now(),
        }
    }
}
