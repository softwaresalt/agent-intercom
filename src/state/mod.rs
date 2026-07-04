//! Protocol-neutral shared application state.
//!
//! Holds the core runtime context (database, configuration, Slack service,
//! pending-request channels, stall detectors, driver, etc.) shared across tool
//! handlers, Slack event handlers, and background tasks. This state is
//! independent of the MCP or ACP protocol layers, so it lives outside
//! `src/mcp/` and survives the eventual removal of the MCP surface.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::process::Child;
use tokio::sync::{oneshot, Mutex};

use crate::audit::AuditLogger;
use crate::config::GlobalConfig;
use crate::driver::AgentDriver;
use crate::mode::ServerMode;
use crate::orchestrator::stall_detector::{StallDetectorHandle, StallEvent};
use crate::policy::watcher::PolicyCache;
use crate::slack::client::SlackService;

/// Response payload delivered through a pending approval oneshot channel.
#[derive(Debug, Clone)]
pub struct ApprovalResponse {
    /// Operator decision: `approved`, `rejected`, or `timeout`.
    pub status: String,
    /// Optional rejection reason.
    pub reason: Option<String>,
}

/// Response payload delivered through a pending prompt oneshot channel.
#[derive(Debug, Clone)]
pub struct PromptResponse {
    /// Operator decision: `continue`, `refine`, or `stop`.
    pub decision: String,
    /// Revised instruction text (present only when decision is `refine`).
    pub instruction: Option<String>,
}

/// Response payload delivered through a pending wait-for-instruction oneshot channel.
#[derive(Debug, Clone)]
pub struct WaitResponse {
    /// Outcome: `resumed` or `timeout`.
    pub status: String,
    /// Optional instruction text from the operator.
    pub instruction: Option<String>,
}

/// Thread-safe map of pending approval `oneshot` senders keyed by `request_id`.
pub type PendingApprovals = Arc<Mutex<HashMap<String, oneshot::Sender<ApprovalResponse>>>>;

/// Thread-safe map of pending prompt `oneshot` senders keyed by `prompt_id`.
pub type PendingPrompts = Arc<Mutex<HashMap<String, oneshot::Sender<PromptResponse>>>>;

/// Thread-safe map of pending wait-for-instruction `oneshot` senders keyed by `session_id`.
pub type PendingWaits = Arc<Mutex<HashMap<String, oneshot::Sender<WaitResponse>>>>;

/// Thread-safe map of pending terminal-command approval `request_id`s to their original
/// command strings.
///
/// Populated by the `check_auto_approve` tool when `kind = "terminal_command"` and the
/// command is not in the workspace auto-approve policy. The approval handler reads this
/// map to distinguish command approvals (no DB record) from diff approvals (DB record
/// exists) and to supply the command string for the auto-approve suggestion UI.
pub type PendingCommandApprovals = Arc<Mutex<HashMap<String, String>>>;

/// Thread-safe map of per-session stall detector handles keyed by `session_id`.
pub type StallDetectors = Arc<Mutex<HashMap<String, StallDetectorHandle>>>;

/// Cached original-message coordinates for modal-submission button updates.
///
/// When a button handler opens a Slack modal (e.g. "Resume with Instructions"
/// or "Refine"), it stores the original message's `(channel_id, message_ts)`
/// keyed by the modal `callback_id`. The `ViewSubmission` handler retrieves
/// these later to replace the "⏳ Processing…" indicator with a final status
/// line (FR-022).
pub type PendingModalContexts = Arc<Mutex<HashMap<String, (String, String)>>>;

/// Thread-safe map of pending thread-reply oneshot senders keyed by `thread_ts`.
///
/// Used by the thread-reply fallback (F-16/F-17) when Slack modals cannot be
/// opened (e.g. `trigger_id` expiry in Socket Mode). The button handler
/// registers a `(session_id, authorized_user_id, sender)` triple here. The push-event
/// handler delivers the operator's reply text through the oneshot only when the reply
/// comes from the stored authorized user. The `session_id` enables cleanup when the
/// owning session terminates (`cleanup_session_fallbacks` — F-20).
///
/// Canonical definition lives in [`crate::slack::handlers::thread_reply`]; re-exported
/// here so callers that import `AppState` from this module get the type without a
/// separate use-path.
pub use crate::slack::handlers::thread_reply::PendingThreadReplies;

/// Live child processes spawned by the `session-start` slash command,
/// keyed by `session_id`. Keeping them here prevents `kill_on_drop` from
/// terminating the process the moment `spawn_session` returns.
pub type ActiveChildren = Arc<Mutex<HashMap<String, Child>>>;

/// Shared application state accessible by all tool handlers, Slack event
/// handlers, and background tasks.
pub struct AppState {
    /// Global configuration.
    pub config: Arc<GlobalConfig>,
    /// `SQLite` connection pool.
    pub db: Arc<SqlitePool>,
    /// Slack client service (absent in local-only mode).
    pub slack: Option<Arc<SlackService>>,
    /// Pending approval request senders keyed by `request_id`.
    pub pending_approvals: PendingApprovals,
    /// Pending continuation prompt senders keyed by `prompt_id`.
    pub pending_prompts: PendingPrompts,
    /// Pending wait-for-instruction senders keyed by `session_id`.
    pub pending_waits: PendingWaits,
    /// Pending terminal-command approval `request_id`s → original command strings.
    ///
    /// Keyed by `request_id`; used by the Slack approval handler to identify
    /// command approvals and skip the DB approval record path.
    pub pending_command_approvals: PendingCommandApprovals,
    /// Cached modal message contexts for FR-022 button replacement.
    pub pending_modal_contexts: PendingModalContexts,
    /// Pending thread-reply oneshot senders keyed by `thread_ts` (F-16/F-17 fallback).
    ///
    /// When `views.open` fails, a oneshot sender is registered here by the
    /// button handler. The push-event handler ([`crate::slack::push_events`])
    /// delivers the operator's reply text through the oneshot when the
    /// authorized user replies in the fallback thread.
    pub pending_thread_replies: PendingThreadReplies,
    /// Per-session stall detector handles keyed by `session_id`.
    pub stall_detectors: Option<StallDetectors>,
    /// Shared secret for IPC authentication (`None` disables auth).
    pub ipc_auth_token: Option<String>,
    /// Shared workspace policy cache for hot-reload.
    pub policy_cache: PolicyCache,
    /// Audit log writer (absent if audit logging is disabled).
    pub audit_logger: Option<Arc<dyn AuditLogger>>,
    /// Live child processes spawned by session-start, keyed by `session_id`.
    pub active_children: ActiveChildren,
    /// Shared sender for stall events. Each per-session stall detector
    /// clones from this to emit events into the single consumer task.
    pub stall_event_tx: Option<tokio::sync::mpsc::Sender<StallEvent>>,
    /// Protocol-agnostic agent driver for resolving pending clearances,
    /// prompts, and waits via Slack handlers.
    pub driver: Arc<dyn AgentDriver>,
    /// Server protocol mode chosen at startup (`mcp` or `acp`).
    ///
    /// Determines which session-start path the Slack command handler
    /// takes: MCP spawns an HTTP/SSE-connecting process; ACP spawns a
    /// headless stdio-connected process.
    pub server_mode: ServerMode,
    /// Hot-reloadable workspace-to-channel mapping table.
    ///
    /// Populated from `[[workspace]]` entries in `config.toml` at startup.
    /// When a [`crate::config_watcher::ConfigWatcher`] is active it updates
    /// this `Arc` in-place, so new sessions always see the latest mappings
    /// without requiring a server restart (FR-014).
    ///
    /// The SSE transport reads this at session-creation time to resolve a
    /// `?workspace_id=` query parameter to the configured `channel_id`.
    pub workspace_mappings: Arc<std::sync::RwLock<Vec<crate::config::WorkspaceMapping>>>,
    /// Shared sender for ACP agent events (absent in MCP mode).
    ///
    /// Each ACP session's reader task emits [`crate::driver::AgentEvent`]s through
    /// this channel. A single consumer task started in `main.rs` dispatches events
    /// (status updates, clearance requests, heartbeats, terminations) to the
    /// appropriate Slack handler as the ACP feature grows across phases.
    pub acp_event_tx: Option<tokio::sync::mpsc::Sender<crate::driver::AgentEvent>>,
    /// ACP-specific driver for per-session stream registration (absent in MCP mode).
    ///
    /// Holds the per-session outbound writer channels used by `handle_acp_session_start`
    /// to wire the reader/writer tasks. Slack handlers resolve operator decisions
    /// through the `driver` field (which points to the same underlying `AcpDriver`).
    pub acp_driver: Option<Arc<crate::driver::acp_driver::AcpDriver>>,
}
