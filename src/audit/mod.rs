//! Structured audit logging for agent interaction events.
//!
//! Provides the [`AuditLogger`] trait and associated types. The primary
//! implementation, [`JsonlAuditWriter`], appends JSONL records to
//! daily-rotating files in `.intercom/logs/`.

pub mod writer;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Event type classification for audit log entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// MCP tool invocation.
    ToolCall,
    /// Operator approved a code proposal.
    Approval,
    /// Operator rejected a code proposal.
    Rejection,
    /// Operator approved a terminal command.
    CommandApproval,
    /// Operator rejected a terminal command.
    CommandRejection,
    /// Agent session started.
    SessionStart,
    /// Agent session terminated explicitly.
    SessionTerminate,
    /// Agent session interrupted by crash or shutdown.
    SessionInterrupt,
}

/// A structured record of an agent interaction event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// ISO 8601 timestamp with timezone.
    pub timestamp: DateTime<Utc>,
    /// Associated session identifier (optional for server-level events).
    pub session_id: Option<String>,
    /// Event classification.
    pub event_type: AuditEventType,
    /// MCP tool name (for `tool_call` events).
    pub tool_name: Option<String>,
    /// Tool call parameters (for `tool_call` events).
    pub parameters: Option<serde_json::Value>,
    /// Brief result description.
    pub result_summary: Option<String>,
    /// Slack user ID of the operator (for approval/rejection events).
    pub operator_id: Option<String>,
    /// Rejection reason (for rejection events).
    pub reason: Option<String>,
    /// Approval request identifier (for approval/rejection events).
    pub request_id: Option<String>,
    /// Terminal command (for command approval/rejection events).
    pub command: Option<String>,
}

impl AuditEntry {
    /// Construct a minimal audit entry for the given event type.
    #[must_use]
    pub fn new(event_type: AuditEventType) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id: None,
            event_type,
            tool_name: None,
            parameters: None,
            result_summary: None,
            operator_id: None,
            reason: None,
            request_id: None,
            command: None,
        }
    }

    /// Set the session identifier for this entry.
    #[must_use]
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Set the MCP tool name for this entry.
    #[must_use]
    pub fn with_tool(mut self, tool_name: String) -> Self {
        self.tool_name = Some(tool_name);
        self
    }

    /// Set the result summary for this entry.
    #[must_use]
    pub fn with_result(mut self, summary: String) -> Self {
        self.result_summary = Some(summary);
        self
    }

    /// Set the operator Slack user ID for this entry.
    #[must_use]
    pub fn with_operator(mut self, operator_id: String) -> Self {
        self.operator_id = Some(operator_id);
        self
    }

    /// Set the approval request identifier for this entry.
    #[must_use]
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Set the rejection (or cancellation) reason for this entry.
    #[must_use]
    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    /// Set the terminal command for this entry.
    #[must_use]
    pub fn with_command(mut self, command: String) -> Self {
        self.command = Some(command);
        self
    }
}

/// Writes structured audit entries to a persistent store.
///
/// Implementations must be [`Send`] and [`Sync`] to allow sharing across
/// async task boundaries via [`std::sync::Arc`].
pub trait AuditLogger: Send + Sync {
    /// Record a single audit entry.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying write operation fails.
    fn log_entry(&self, entry: AuditEntry) -> crate::Result<()>;
}

pub use writer::JsonlAuditWriter;
