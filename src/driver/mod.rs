//! Protocol-agnostic agent driver abstraction.
//!
//! The [`AgentDriver`] trait decouples the shared application core
//! (Slack handlers, persistence, policy) from the agent communication
//! protocol (MCP or ACP). All operator actions that affect agent flow
//! route through this trait.

pub mod acp_driver;
pub mod mcp_driver;

use std::future::Future;
use std::pin::Pin;

use crate::models::progress::ProgressItem;
use crate::Result;

/// Events emitted by driver implementations into the shared event channel.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Agent requests operator approval for a file operation.
    ClearanceRequested {
        /// Unique request identifier.
        request_id: String,
        /// Session this request belongs to.
        session_id: String,
        /// Short description of the proposed change.
        title: String,
        /// Detailed description of the proposed change.
        description: String,
        /// Unified diff of the proposed change.
        diff: Option<String>,
        /// Target file path.
        file_path: String,
        /// Risk classification (`low`, `high`, `critical`).
        risk_level: String,
    },
    /// Agent emitted a status or log message.
    StatusUpdated {
        /// Session this update belongs to.
        session_id: String,
        /// Human-readable status message.
        message: String,
    },
    /// Agent forwarded a continuation prompt for operator decision.
    PromptForwarded {
        /// Session this prompt belongs to.
        session_id: String,
        /// Unique prompt identifier.
        prompt_id: String,
        /// Prompt text to display to the operator.
        prompt_text: String,
        /// Prompt type classification.
        prompt_type: String,
    },
    /// Agent sent a heartbeat with optional progress snapshot.
    HeartbeatReceived {
        /// Session this heartbeat belongs to.
        session_id: String,
        /// Optional list of in-progress tasks.
        progress: Option<Vec<ProgressItem>>,
    },
    /// Agent process terminated.
    SessionTerminated {
        /// Session that terminated.
        session_id: String,
        /// Process exit code, if available.
        exit_code: Option<i32>,
        /// Human-readable reason for termination.
        reason: String,
    },
}

/// Protocol-agnostic interface between the application core and an agent.
///
/// Implementations provide MCP or ACP protocol-specific communication
/// while exposing a uniform surface to Slack handlers and the orchestrator.
pub trait AgentDriver: Send + Sync {
    /// Resolve a pending clearance request.
    ///
    /// In MCP: Sends the response through the oneshot channel.
    /// In ACP: Writes a `clearance/response` message to the agent stream.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::NotFound`](crate::AppError::NotFound) if `request_id` is unknown.
    /// Returns [`AppError::Acp`](crate::AppError::Acp) if the stream write fails.
    fn resolve_clearance(
        &self,
        request_id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Send a new prompt or instruction to the agent.
    ///
    /// In MCP: Posts an MCP `intercom/nudge` notification.
    /// In ACP: Writes a `prompt/send` message to the agent stream.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::NotFound`](crate::AppError::NotFound) if `session_id` is unknown.
    /// Returns [`AppError::Acp`](crate::AppError::Acp) if the stream write fails.
    fn send_prompt(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Interrupt/cancel the agent's current work.
    ///
    /// In MCP: Sends a cancellation signal via the MCP transport.
    /// In ACP: Writes a `session/interrupt` message to the agent stream.
    ///
    /// This operation is idempotent — calling on an already-terminated session
    /// returns `Ok(())`.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Acp`](crate::AppError::Acp) if the stream write fails for active sessions.
    fn interrupt(&self, session_id: &str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Resolve a pending continuation prompt.
    ///
    /// In MCP: Sends the response through the prompt oneshot channel.
    /// In ACP: Writes the decision back to the agent stream.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::NotFound`](crate::AppError::NotFound) if `prompt_id` is unknown.
    /// Returns [`AppError::Acp`](crate::AppError::Acp) if the stream write fails.
    fn resolve_prompt(
        &self,
        prompt_id: &str,
        decision: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Resolve a pending wait-for-instruction (standby).
    ///
    /// In MCP: Sends through the wait oneshot channel.
    /// In ACP: Writes a `prompt/send` message with the instruction.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::NotFound`](crate::AppError::NotFound) if `session_id` is unknown.
    /// Returns [`AppError::Acp`](crate::AppError::Acp) if the stream write fails.
    fn resolve_wait(
        &self,
        session_id: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}
