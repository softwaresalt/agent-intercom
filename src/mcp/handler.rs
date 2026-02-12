//! MCP server handler, shared application state, and tool router.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use rmcp::handler::server::{
    tool::{ToolCallContext, ToolRoute, ToolRouter},
    ServerHandler,
};
use rmcp::model::{
    CallToolRequestParam, CallToolResult, ListToolsResult, PaginatedRequestParam, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use tokio::sync::{oneshot, Mutex};
use tracing::info_span;

use crate::config::GlobalConfig;
use crate::slack::client::SlackService;

/// Response payload delivered through a pending approval oneshot channel.
#[derive(Debug, Clone)]
pub struct ApprovalResponse {
    /// Operator decision: `approved`, `rejected`, or `timeout`.
    pub status: String,
    /// Optional rejection reason.
    pub reason: Option<String>,
}

/// Thread-safe map of pending approval `oneshot` senders keyed by `request_id`.
pub type PendingApprovals = Arc<Mutex<HashMap<String, oneshot::Sender<ApprovalResponse>>>>;

/// Shared application state accessible by all MCP tool handlers.
pub struct AppState {
    /// Global configuration.
    pub config: Arc<GlobalConfig>,
    /// `SurrealDB` connection pool.
    pub db: Arc<Surreal<Db>>,
    /// Slack client service (absent in local-only mode).
    pub slack: Option<Arc<SlackService>>,
    /// Pending approval request senders keyed by `request_id`.
    pub pending_approvals: PendingApprovals,
}

/// MCP server implementation that exposes the nine monocoque-agent-rem tools.
pub struct AgentRemServer {
    state: Arc<AppState>,
}

impl AgentRemServer {
    /// Create a new MCP server bound to shared application state.
    #[must_use]
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Access the shared application state.
    #[must_use]
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    fn tool_router() -> ToolRouter<Self> {
        let mut router = ToolRouter::new();

        for tool in Self::all_tools() {
            let name = tool.name.to_string();
            match name.as_str() {
                "ask_approval" => {
                    router.add_route(ToolRoute::new_dyn(tool, |context| {
                        Box::pin(crate::mcp::tools::ask_approval::handle(context))
                    }));
                }
                "accept_diff" => {
                    router.add_route(ToolRoute::new_dyn(tool, |context| {
                        Box::pin(crate::mcp::tools::accept_diff::handle(context))
                    }));
                }
                _ => {
                    router.add_route(ToolRoute::new_dyn(tool, |_context| {
                        Box::pin(async {
                            Err(rmcp::ErrorData::internal_error(
                                "tool not implemented",
                                None,
                            ))
                        })
                    }));
                }
            }
        }

        router
    }

    /// Convert a `serde_json::Value::Object` into the `Arc<Map>` expected by `Tool`.
    fn schema(value: serde_json::Value) -> Arc<serde_json::Map<String, serde_json::Value>> {
        match value {
            serde_json::Value::Object(map) => Arc::new(map),
            _ => Arc::new(serde_json::Map::default()),
        }
    }

    #[allow(clippy::too_many_lines)] // Tool definitions are intentionally verbose for clarity.
    fn all_tools() -> Vec<Tool> {
        vec![
            Tool {
                name: "ask_approval".into(),
                description: Some(
                    "Submit a code proposal for remote operator approval via Slack. \
                     Blocks until the operator responds or the timeout elapses."
                        .into(),
                ),
                input_schema: Self::schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "description": { "type": "string" },
                        "diff": { "type": "string" },
                        "file_path": { "type": "string" },
                        "risk_level": { "type": "string", "enum": ["low", "high", "critical"], "default": "low" }
                    },
                    "required": ["title", "diff", "file_path"]
                })),
                output_schema: None,
                annotations: None,
            },
            Tool {
                name: "accept_diff".into(),
                description: Some(
                    "Apply previously approved code changes to the local file system.".into(),
                ),
                input_schema: Self::schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "request_id": { "type": "string" },
                        "force": { "type": "boolean", "default": false }
                    },
                    "required": ["request_id"]
                })),
                output_schema: None,
                annotations: None,
            },
            Tool {
                name: "check_auto_approve".into(),
                description: Some(
                    "Query the workspace auto-approve policy to determine whether an \
                     operation can bypass the remote approval gate."
                        .into(),
                ),
                input_schema: Self::schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "tool_name": { "type": "string" },
                        "context": { "type": "object" }
                    },
                    "required": ["tool_name"]
                })),
                output_schema: None,
                annotations: None,
            },
            Tool {
                name: "forward_prompt".into(),
                description: Some(
                    "Forward an agent-generated continuation prompt to the remote \
                     operator via Slack. Blocks until the operator responds."
                        .into(),
                ),
                input_schema: Self::schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "prompt_text": { "type": "string" },
                        "prompt_type": { "type": "string", "enum": ["continuation", "clarification", "error_recovery", "resource_warning"], "default": "continuation" },
                        "elapsed_seconds": { "type": "integer" },
                        "actions_taken": { "type": "integer" }
                    },
                    "required": ["prompt_text"]
                })),
                output_schema: None,
                annotations: None,
            },
            Tool {
                name: "remote_log".into(),
                description: Some(
                    "Send a non-blocking status log message to the Slack channel.".into(),
                ),
                input_schema: Self::schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" },
                        "level": { "type": "string", "enum": ["info", "success", "warning", "error"], "default": "info" },
                        "thread_ts": { "type": "string" }
                    },
                    "required": ["message"]
                })),
                output_schema: None,
                annotations: None,
            },
            Tool {
                name: "recover_state".into(),
                description: Some(
                    "Retrieve the last known state from persistent storage. Called on \
                     startup to check for interrupted sessions or pending requests."
                        .into(),
                ),
                input_schema: Self::schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string" }
                    }
                })),
                output_schema: None,
                annotations: None,
            },
            Tool {
                name: "set_operational_mode".into(),
                description: Some(
                    "Switch between remote, local, and hybrid operational modes at runtime.".into(),
                ),
                input_schema: Self::schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "mode": { "type": "string", "enum": ["remote", "local", "hybrid"] }
                    },
                    "required": ["mode"]
                })),
                output_schema: None,
                annotations: None,
            },
            Tool {
                name: "wait_for_instruction".into(),
                description: Some(
                    "Place the agent in standby, polling for a resume signal or new \
                     command from the operator via Slack."
                        .into(),
                ),
                input_schema: Self::schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string", "default": "Agent is idle and awaiting instructions." },
                        "timeout_seconds": { "type": "integer", "default": 0 }
                    }
                })),
                output_schema: None,
                annotations: None,
            },
            Tool {
                name: "heartbeat".into(),
                description: Some(
                    "Lightweight liveness signal. Resets the stall detection timer and \
                     optionally stores a structured progress snapshot."
                        .into(),
                ),
                input_schema: Self::schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "status_message": { "type": "string" },
                        "progress_snapshot": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "label": { "type": "string" },
                                    "status": { "type": "string", "enum": ["done", "in_progress", "pending"] }
                                },
                                "required": ["label", "status"]
                            }
                        }
                    }
                })),
                output_schema: None,
                annotations: None,
            },
        ]
    }
}

impl ServerHandler for AgentRemServer {
    fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_ {
        let router = Self::tool_router();
        let _span = info_span!("call_tool", tool = %request.name).entered();

        async move {
            router
                .call(ToolCallContext::new(self, request, context))
                .await
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_ {
        let tools = Self::all_tools();

        std::future::ready(Ok(ListToolsResult::with_all_items(tools)))
    }
}
