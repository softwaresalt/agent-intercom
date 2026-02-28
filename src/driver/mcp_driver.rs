//! MCP protocol implementation of [`AgentDriver`].
//!
//! Bridges operator actions (Slack button presses, modal submissions) with
//! in-flight MCP tool calls that are suspended on `oneshot` channels.
//! All maps are `Arc<tokio::sync::Mutex<...>>` clones of the maps held by
//! [`AppState`](crate::mcp::handler::AppState) so every Slack handler and
//! every MCP tool handler share the same in-memory state.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::warn;

use crate::driver::AgentDriver;
use crate::mcp::handler::{
    ApprovalResponse, PendingApprovals, PendingPrompts, PendingWaits, PromptResponse, WaitResponse,
};
use crate::AppError;
use crate::Result;

/// MCP-protocol implementation of [`AgentDriver`].
///
/// Resolves pending agent requests by delivering responses through the
/// `oneshot` channels registered in [`AppState`](crate::mcp::handler::AppState).
/// This makes the Slack interaction layer independent of MCP internals — the
/// handlers only call trait methods, never touch the maps directly.
// The `pending_` prefix is intentional and mirrors the field names on `AppState`
// so that the two sides of each channel are obviously paired.
#[allow(clippy::struct_field_names)]
pub struct McpDriver {
    /// Pending approval `oneshot` senders keyed by `request_id`.
    pending_approvals: PendingApprovals,
    /// Pending continuation-prompt `oneshot` senders keyed by `prompt_id`.
    pending_prompts: PendingPrompts,
    /// Pending wait-for-instruction `oneshot` senders keyed by `session_id`.
    pending_waits: PendingWaits,
}

impl McpDriver {
    /// Create a new `McpDriver` from shared pending-request maps.
    ///
    /// The maps must be the same `Arc` instances held by `AppState` so that
    /// senders inserted by MCP tool handlers are visible to the Slack handlers
    /// resolving them through this driver.
    #[must_use]
    pub fn new(approvals: PendingApprovals, prompts: PendingPrompts, waits: PendingWaits) -> Self {
        Self {
            pending_approvals: approvals,
            pending_prompts: prompts,
            pending_waits: waits,
        }
    }

    /// Create an `McpDriver` backed by empty maps, wrapped in `Arc<dyn AgentDriver>`.
    ///
    /// Intended for use in tests that do not need pre-seeded pending channels.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let driver = McpDriver::new_empty();
    /// assert!(driver.interrupt("any-session").await.is_ok());
    /// ```
    #[must_use]
    pub fn new_empty() -> Arc<dyn AgentDriver> {
        Arc::new(Self {
            pending_approvals: Arc::new(Mutex::new(HashMap::new())),
            pending_prompts: Arc::new(Mutex::new(HashMap::new())),
            pending_waits: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

impl AgentDriver for McpDriver {
    /// Resolve a pending clearance request by delivering an `ApprovalResponse`.
    ///
    /// Removes the sender from `pending_approvals` and delivers the decision.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::NotFound`] if `request_id` has no pending entry.
    fn resolve_clearance(
        &self,
        request_id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let request_id = request_id.to_owned();
        Box::pin(async move {
            let tx = {
                let mut pending = self.pending_approvals.lock().await;
                pending.remove(&request_id)
            };

            let Some(tx) = tx else {
                return Err(AppError::NotFound(format!(
                    "no pending clearance for request_id '{request_id}'"
                )));
            };

            let response = ApprovalResponse {
                status: if approved {
                    "approved".to_owned()
                } else {
                    "rejected".to_owned()
                },
                reason,
            };

            if tx.send(response).is_err() {
                warn!(request_id, "clearance oneshot receiver already dropped");
            }

            Ok(())
        })
    }

    /// Send a prompt or nudge to the agent.
    ///
    /// In the MCP implementation this is a stub — ACP stream wiring is added
    /// in a later phase. Always returns `Ok(())`.
    fn send_prompt(
        &self,
        _session_id: &str,
        _prompt: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move { Ok(()) })
    }

    /// Interrupt / cancel the agent's current work.
    ///
    /// This operation is idempotent — calling on an unknown or already-terminated
    /// session always returns `Ok(())`.
    fn interrupt(
        &self,
        _session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move { Ok(()) })
    }

    /// Resolve a pending continuation prompt by delivering a `PromptResponse`.
    ///
    /// Removes the sender from `pending_prompts` and delivers the decision.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::NotFound`] if `prompt_id` has no pending entry.
    fn resolve_prompt(
        &self,
        prompt_id: &str,
        decision: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let prompt_id = prompt_id.to_owned();
        let decision = decision.to_owned();
        Box::pin(async move {
            let tx = {
                let mut pending = self.pending_prompts.lock().await;
                pending.remove(&prompt_id)
            };

            let Some(tx) = tx else {
                return Err(AppError::NotFound(format!(
                    "no pending prompt for prompt_id '{prompt_id}'"
                )));
            };

            let response = PromptResponse {
                decision,
                instruction,
            };

            if tx.send(response).is_err() {
                warn!(prompt_id, "prompt oneshot receiver already dropped");
            }

            Ok(())
        })
    }

    /// Resolve a pending wait-for-instruction by delivering a `WaitResponse`.
    ///
    /// Removes the sender from `pending_waits` and delivers the instruction.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::NotFound`] if `session_id` has no pending wait entry.
    fn resolve_wait(
        &self,
        session_id: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let session_id = session_id.to_owned();
        Box::pin(async move {
            let tx = {
                let mut pending = self.pending_waits.lock().await;
                pending.remove(&session_id)
            };

            let Some(tx) = tx else {
                return Err(AppError::NotFound(format!(
                    "no pending wait for session_id '{session_id}'"
                )));
            };

            let response = WaitResponse {
                status: "resumed".to_owned(),
                instruction,
            };

            if tx.send(response).is_err() {
                warn!(session_id, "wait oneshot receiver already dropped");
            }

            Ok(())
        })
    }
}

// Verify Send + Sync at compile time.
const _: () = {
    fn assert_send_sync<T: Send + Sync>() {}
    fn check() {
        assert_send_sync::<McpDriver>();
    }
    let _ = check;
};
