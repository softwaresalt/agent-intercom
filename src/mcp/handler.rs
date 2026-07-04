//! MCP server handler and tool router.

use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rmcp::handler::server::{
    tool::{ToolCallContext, ToolRoute, ToolRouter},
    ServerHandler,
};
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Implementation, ListResourceTemplatesResult,
    ListResourcesResult, ListToolsResult, PaginatedRequestParam, ReadResourceRequestParam,
    ReadResourceResult, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{NotificationContext, RequestContext, RoleServer};
use tokio_util::sync::CancellationToken;
use tracing::{info, info_span, warn};

use crate::audit::{AuditEntry, AuditEventType};
use crate::models::session::{Session, SessionMode, SessionStatus};
use crate::orchestrator::stall_detector::StallDetector;
use crate::persistence::session_repo::SessionRepo;

use crate::state::AppState;

/// Owner ID assigned to sessions created by direct (non-spawned) agent connections.
///
/// Distinguishes locally-initiated sessions from sessions spawned via the Slack
/// `/spawn` command (which use the operator's real Slack user ID).  Used by
/// `on_initialized` to clean up stale direct-connection sessions on reconnect.
const LOCAL_AGENT_OWNER: &str = "agent:local";

/// MCP server implementation that exposes the nine agent-intercom tools.
pub struct IntercomServer {
    state: Arc<AppState>,
    /// Per-session Slack channel override supplied via SSE query parameter.
    channel_id_override: Option<String>,
    /// Pre-existing session ID supplied by the spawner via SSE query parameter.
    session_id_override: Option<String>,
    /// DB session ID recorded by `on_initialized` (Case 2 — direct connections only).
    ///
    /// Set exactly once after the auto-created session is activated.  `Drop`
    /// reads this to mark the session `Terminated` when the transport closes.
    /// `Arc` allows the value to be cloned into the async future returned by
    /// `on_initialized` without requiring `&mut self`.
    session_db_id: Arc<OnceLock<String>>,
}

impl IntercomServer {
    /// Create a new MCP server bound to shared application state.
    #[must_use]
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            channel_id_override: None,
            session_id_override: None,
            session_db_id: Arc::new(OnceLock::new()),
        }
    }

    /// Create a new MCP server with a per-session Slack channel override.
    #[must_use]
    pub fn with_channel_override(state: Arc<AppState>, channel_id: Option<String>) -> Self {
        Self {
            state,
            channel_id_override: channel_id,
            session_id_override: None,
            session_db_id: Arc::new(OnceLock::new()),
        }
    }

    /// Create a new MCP server with per-session SSE query-parameter overrides.
    ///
    /// Used by the SSE transport when both a Slack channel and a pre-created
    /// session ID are supplied as query parameters.
    #[must_use]
    pub fn with_overrides(
        state: Arc<AppState>,
        channel_id: Option<String>,
        session_id: Option<String>,
    ) -> Self {
        Self {
            state,
            channel_id_override: channel_id,
            session_id_override: session_id,
            session_db_id: Arc::new(OnceLock::new()),
        }
    }

    /// Store the DB session ID that was created by `on_initialized`.
    ///
    /// **For testing only.** Injects the session ID that `on_initialized` would
    /// normally record after auto-creating a direct-connection session.  This
    /// lets integration tests drive the `Drop`-based cleanup path without
    /// needing a live MCP transport.
    pub fn set_session_id_for_testing(&self, session_id: String) {
        // Ignore the error if already set (idempotent for tests).
        let _ = self.session_db_id.set(session_id);
    }

    /// Return the effective Slack channel ID for this session, if one is
    /// configured.
    ///
    /// Returns `Some` when a per-session override was supplied via the
    /// `?channel_id=` SSE query parameter. Returns `None` when no
    /// channel is available (e.g. stdio / local connections without a
    /// workspace `mcp.json` channel).  The global `config.slack.channel_id`
    /// is treated as absent when it is empty.
    #[must_use]
    pub fn effective_channel_id(&self) -> Option<&str> {
        self.channel_id_override.as_deref().or_else(|| {
            let ch = &self.state.config.slack.channel_id;
            if ch.is_empty() {
                None
            } else {
                Some(ch.as_str())
            }
        })
    }

    /// Return the pre-existing DB session ID override for this connection (T112 / HITL-003).
    ///
    /// In ACP mode, spawned agent subprocesses connect to the HTTP MCP endpoint
    /// with `?session_id=<id>` to associate their tool calls with a specific
    /// ACP session.  This accessor exposes that override so MCP tool handlers
    /// can route requests to the correct session without relying on
    /// `list_active()`, which is ambiguous when multiple ACP sessions are running.
    ///
    /// Returns `None` in MCP mode or when the query parameter was not supplied.
    #[must_use]
    pub fn session_id_override(&self) -> Option<&str> {
        self.session_id_override.as_deref()
    }

    /// Access the shared application state.
    #[must_use]
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    fn tool_router() -> &'static ToolRouter<Self> {
        static ROUTER: std::sync::OnceLock<ToolRouter<IntercomServer>> = std::sync::OnceLock::new();
        ROUTER.get_or_init(|| {
            let mut router = ToolRouter::new();

            for tool in Self::all_tools() {
                let name = tool.name.to_string();
                match name.as_str() {
                    "check_clearance" => {
                        router.add_route(ToolRoute::new_dyn(tool, |context| {
                            Box::pin(crate::mcp::tools::ask_approval::handle(context))
                        }));
                    }
                    "check_diff" => {
                        router.add_route(ToolRoute::new_dyn(tool, |context| {
                            Box::pin(crate::mcp::tools::accept_diff::handle(context))
                        }));
                    }
                    "ping" => {
                        router.add_route(ToolRoute::new_dyn(tool, |context| {
                            Box::pin(crate::mcp::tools::heartbeat::handle(context))
                        }));
                    }
                    "broadcast" => {
                        router.add_route(ToolRoute::new_dyn(tool, |context| {
                            Box::pin(crate::mcp::tools::remote_log::handle(context))
                        }));
                    }
                    "transmit" => {
                        router.add_route(ToolRoute::new_dyn(tool, |context| {
                            Box::pin(crate::mcp::tools::forward_prompt::handle(context))
                        }));
                    }
                    "auto_check" => {
                        router.add_route(ToolRoute::new_dyn(tool, |context| {
                            Box::pin(crate::mcp::tools::check_auto_approve::handle(context))
                        }));
                    }
                    "reboot" => {
                        router.add_route(ToolRoute::new_dyn(tool, |context| {
                            Box::pin(crate::mcp::tools::recover_state::handle(context))
                        }));
                    }
                    "switch_freq" => {
                        router.add_route(ToolRoute::new_dyn(tool, |context| {
                            Box::pin(crate::mcp::tools::set_operational_mode::handle(context))
                        }));
                    }
                    "standby" => {
                        router.add_route(ToolRoute::new_dyn(tool, |context| {
                            Box::pin(crate::mcp::tools::wait_for_instruction::handle(context))
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
        })
    }

    /// Convert a `serde_json::Value::Object` into the `Arc<Map>` expected by `Tool`.
    fn schema(value: serde_json::Value) -> Arc<serde_json::Map<String, serde_json::Value>> {
        match value {
            serde_json::Value::Object(map) => Arc::new(map),
            _ => Arc::new(serde_json::Map::default()),
        }
    }

    #[allow(clippy::too_many_lines)] // Tool definitions are intentionally verbose for clarity.
    pub(crate) fn all_tools() -> Vec<Tool> {
        vec![
            Tool {
                name: "check_clearance".into(),
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
                title: None,
                icons: None,
                meta: None,
            },
            Tool {
                name: "check_diff".into(),
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
                title: None,
                icons: None,
                meta: None,
            },
            Tool {
                name: "auto_check".into(),
                description: Some(
                    "Query the workspace auto-approve policy to determine whether an \
                     operation can bypass the remote approval gate. When kind is \
                     'terminal_command' and the command is not already auto-approved, \
                     blocks and posts a Slack approval prompt to the operator."
                        .into(),
                ),
                input_schema: Self::schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "tool_name": {
                            "type": "string",
                            "description": "Name of the tool or shell command to check"
                        },
                        "kind": {
                            "type": "string",
                            "description": "Operation kind: 'terminal_command' for shell commands (triggers blocking Slack gate when not auto-approved), 'file_operation' for file changes, or omit for standard non-blocking policy check",
                            "enum": ["terminal_command", "file_operation"]
                        },
                        "context": {
                            "type": "object",
                            "description": "Optional metadata for fine-grained evaluation (e.g. file_path, risk_level)"
                        }
                    },
                    "required": ["tool_name"]
                })),
                output_schema: None,
                annotations: None,
                title: None,
                icons: None,
                meta: None,
            },
            Tool {
                name: "transmit".into(),
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
                title: None,
                icons: None,
                meta: None,
            },
            Tool {
                name: "broadcast".into(),
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
                title: None,
                icons: None,
                meta: None,
            },
            Tool {
                name: "reboot".into(),
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
                title: None,
                icons: None,
                meta: None,
            },
            Tool {
                name: "switch_freq".into(),
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
                title: None,
                icons: None,
                meta: None,
            },
            Tool {
                name: "standby".into(),
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
                title: None,
                icons: None,
                meta: None,
            },
            Tool {
                name: "ping".into(),
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
                title: None,
                icons: None,
                meta: None,
            },
        ]
    }
}

/// Spawn a per-session stall detector and insert its handle into the shared map.
///
/// No-op if stall detection is disabled in the global configuration or if no
/// stall event channel is present in `state`.
async fn spawn_stall_detector_for_session(state: &AppState, session_id: &str) {
    if !state.config.stall.enabled {
        return;
    }
    if let (Some(ref detectors), Some(ref tx)) = (&state.stall_detectors, &state.stall_event_tx) {
        let session_cancel = CancellationToken::new();
        let detector = StallDetector::new(
            session_id.to_owned(),
            Duration::from_secs(state.config.stall.inactivity_threshold_seconds),
            Duration::from_secs(state.config.stall.escalation_threshold_seconds),
            state.config.stall.max_retries,
            tx.clone(),
            session_cancel,
        );
        let handle = detector.spawn();
        detectors.lock().await.insert(session_id.to_owned(), handle);
        info!(session_id, "stall detector spawned");
    }
}

impl ServerHandler for IntercomServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: env!("CARGO_PKG_NAME").into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            ..Default::default()
        }
    }

    /// Auto-create or activate a session when the MCP handshake completes.
    ///
    /// Two cases are handled:
    /// 1. **Spawned agent** (`session_id_override` is `Some`): the session was
    ///    already created and activated by the Slack `/spawn` command before the
    ///    child process connected.  We verify it exists and log the connection.
    /// 2. **Direct connection** (Copilot Chat, Cursor, stdio, etc.): no prior
    ///    session exists, so we auto-create and activate one using the workspace
    ///    path resolved from `channel_id_override` (falling back to
    ///    `default_workspace_root` when no channel is provided).
    #[allow(clippy::too_many_lines)]
    fn on_initialized(
        &self,
        _context: NotificationContext<RoleServer>,
    ) -> impl Future<Output = ()> + Send + '_ {
        let state = Arc::clone(&self.state);
        let session_id_override = self.session_id_override.clone();
        let channel_id_override = self.channel_id_override.clone();
        let is_remote = self.channel_id_override.is_some();
        let session_db_id = Arc::clone(&self.session_db_id);

        async move {
            let session_repo = SessionRepo::new(Arc::clone(&state.db));

            // ── Case 1: Spawned agent with a pre-created session ─────────────
            if let Some(ref sid) = session_id_override {
                match session_repo.get_by_id(sid).await {
                    Ok(Some(session)) => {
                        info!(
                            session_id = %sid,
                            status = ?session.status,
                            "spawned agent connected to pre-created session"
                        );
                        // Spawn a per-session stall detector for the spawned agent (FR-028).
                        spawn_stall_detector_for_session(&state, &session.id).await;
                    }
                    Ok(None) => {
                        warn!(session_id = %sid, "pre-created session not found in database");
                    }
                    Err(err) => {
                        warn!(%err, session_id = %sid, "failed to look up pre-created session");
                    }
                }
                return;
            }

            // ── Case 2: Direct connection — auto-create a session ────────────
            //
            // Before creating, terminate any stale active direct-connection
            // sessions left behind by prior window reloads or reconnections.
            // Only sessions owned by LOCAL_AGENT_OWNER are cleaned up — spawned
            // sessions (owned by real Slack users) are left untouched.
            match session_repo.list_active().await {
                Ok(stale_sessions) => {
                    for stale in &stale_sessions {
                        if stale.owner_user_id == LOCAL_AGENT_OWNER {
                            match session_repo
                                .set_terminated(&stale.id, SessionStatus::Terminated)
                                .await
                            {
                                Ok(_) => {
                                    info!(
                                        session_id = %stale.id,
                                        "terminated stale direct-connection session"
                                    );
                                }
                                Err(err) => {
                                    warn!(
                                        %err,
                                        session_id = %stale.id,
                                        "failed to terminate stale session"
                                    );
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    warn!(%err, "failed to query active sessions for stale cleanup");
                }
            }

            let workspace_root = channel_id_override
                .as_deref()
                .map_or_else(
                    || state.config.default_workspace_root(),
                    |ch| state.config.workspace_root_for_channel(ch),
                )
                .to_string_lossy()
                .into_owned();
            let mode = if is_remote {
                SessionMode::Remote
            } else {
                SessionMode::Local
            };
            let session = Session::new(
                LOCAL_AGENT_OWNER.to_owned(),
                workspace_root,
                Some("Direct agent connection".to_owned()),
                mode,
            );

            match session_repo.create(&session).await {
                Ok(created) => {
                    match session_repo
                        .update_status(&created.id, SessionStatus::Active)
                        .await
                    {
                        Ok(_) => {
                            info!(
                                session_id = %created.id,
                                mode = ?mode,
                                "auto-created session activated on direct connection"
                            );
                            // Record the session ID so Drop can terminate it
                            // when the transport closes (T045/T046).
                            if session_db_id.set(created.id.clone()).is_err() {
                                warn!(
                                    session_id = %created.id,
                                    "session_db_id was already set (unexpected)"
                                );
                            }
                            // Audit-log session start (T061).
                            if let Some(ref logger) = state.audit_logger {
                                let entry = AuditEntry::new(AuditEventType::SessionStart)
                                    .with_session(created.id.clone());
                                if let Err(err) = logger.log_entry(entry) {
                                    warn!(%err, "audit log write failed (session start)");
                                }
                            }
                            // Spawn a per-session stall detector for direct connections (FR-028).
                            spawn_stall_detector_for_session(&state, &created.id).await;

                            // T058 / S036: For remote direct connections that have a
                            // channel_id, post the session-started message and record
                            // the returned Slack ts as the session's thread_ts.
                            if let (Some(ref ch), Some(slack)) =
                                (&channel_id_override, state.slack.as_ref())
                            {
                                if let Ok(Some(ref session)) =
                                    session_repo.get_by_id(&created.id).await
                                {
                                    let started_blocks =
                                        crate::slack::blocks::session_started_blocks(session);
                                    let msg = crate::slack::client::SlackMessage {
                                        channel: slack_morphism::prelude::SlackChannelId(
                                            ch.clone(),
                                        ),
                                        text: Some(format!(
                                            "\u{1f680} Session `{}` connected",
                                            session.id.chars().take(8).collect::<String>()
                                        )),
                                        blocks: Some(started_blocks),
                                        thread_ts: None,
                                    };
                                    match slack.post_message_direct(msg).await {
                                        Ok(ts) => {
                                            if let Err(err) =
                                                session_repo.set_thread_ts(&session.id, &ts.0).await
                                            {
                                                warn!(%err,
                                                    session_id = %session.id,
                                                    "failed to record thread_ts"
                                                );
                                            } else {
                                                info!(
                                                    session_id = %session.id,
                                                    thread_ts = %ts.0,
                                                    "thread_ts recorded for direct connection"
                                                );
                                            }
                                        }
                                        Err(err) => {
                                            warn!(%err,
                                                session_id = %session.id,
                                                "failed to post session-started message"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            warn!(
                                %err,
                                session_id = %created.id,
                                "failed to activate auto-created session"
                            );
                        }
                    }
                }
                Err(err) => {
                    warn!(%err, "failed to auto-create session on direct connection");
                }
            }
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_ {
        let router = Self::tool_router();
        let tool_name = request.name.to_string();
        let _span = info_span!("call_tool", tool = %tool_name).entered();

        // Reset stall timer on every tool call (T053).
        let state = Arc::clone(&self.state);
        let audit_logger = self.state.audit_logger.clone();
        let session_id = self.session_db_id.get().cloned();
        // For spawned agents session_db_id is never set; fall back to the
        // pre-assigned session_id_override so the correct detector is reset.
        let effective_session_id = self
            .session_id_override
            .clone()
            .or_else(|| session_id.clone());

        async move {
            // Reset stall detector only for the calling session (T053).
            if let (Some(ref detectors), Some(ref sid)) =
                (&state.stall_detectors, &effective_session_id)
            {
                if let Some(handle) = detectors.lock().await.get(sid.as_str()) {
                    handle.reset();
                }
            }

            let result = router
                .call(ToolCallContext::new(self, request, context))
                .await;

            // Reset again after tool completion to avoid false stall triggers.
            if let (Some(ref detectors), Some(ref sid)) =
                (&state.stall_detectors, &effective_session_id)
            {
                if let Some(handle) = detectors.lock().await.get(sid.as_str()) {
                    handle.reset();
                }
            }

            // Audit-log every tool call (T058).
            if let Some(ref logger) = audit_logger {
                let summary = if result.is_ok() { "ok" } else { "error" };
                let mut entry = AuditEntry::new(AuditEventType::ToolCall)
                    .with_tool(tool_name.clone())
                    .with_result(summary.to_owned());
                if let Some(ref sid) = effective_session_id {
                    entry = entry.with_session(sid.clone());
                }
                if let Err(err) = logger.log_entry(entry) {
                    warn!(%err, "audit log write failed (tool call)");
                }
            }

            info!(tool = %tool_name, "tool call completed");
            result
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

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, rmcp::ErrorData>> + Send + '_ {
        let result = match self.effective_channel_id() {
            Some(channel_id) => crate::mcp::resources::slack_channel::list_resources(channel_id),
            None => ListResourcesResult {
                resources: vec![],
                next_cursor: None,
                ..Default::default()
            },
        };
        std::future::ready(Ok(result))
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, rmcp::ErrorData>> + Send + '_
    {
        std::future::ready(Ok(
            crate::mcp::resources::slack_channel::resource_templates(),
        ))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, rmcp::ErrorData>> + Send + '_ {
        let state = Arc::clone(&self.state);
        let effective_channel = self.effective_channel_id().map(str::to_owned);
        async move {
            let channel = effective_channel.ok_or_else(|| {
                rmcp::ErrorData::invalid_params(
                    "no Slack channel configured for this session; \
                     supply ?channel_id= in the SSE URL",
                    None,
                )
            })?;
            crate::mcp::resources::slack_channel::read_resource(&request, &state, &channel)
                .await
                .map_err(|err| {
                    rmcp::ErrorData::internal_error(format!("resource read failed: {err}"), None)
                })
        }
    }
}

impl Drop for IntercomServer {
    /// Mark the associated DB session as `Terminated` when the MCP transport closes.
    ///
    /// This hook fires when rmcp disposes of the `IntercomServer` instance after
    /// the SSE stream or stdio connection drops.  Because `Drop` is synchronous
    /// and the DB write is async, the cleanup is submitted to the current Tokio
    /// runtime via [`tokio::runtime::Handle::try_current`].  If no runtime is
    /// available (e.g. during unit tests that do not use a runtime), the cleanup
    /// is silently skipped — sessions will be reclaimed by the stale-session
    /// sweep in the next `on_initialized` call.
    ///
    /// Only direct-connection sessions (Case 2 of `on_initialized`) store an ID
    /// in `session_db_id`.  Spawned-agent servers leave it unset, so their Drop
    /// is always a no-op for the DB path but still cleans up the stall detector.
    fn drop(&mut self) {
        // ── Remove stall detector for both Case 1 and Case 2 ────────────────
        // Case 1 (spawned): session_id_override is set; Case 2 (direct): session_db_id is set.
        let stall_sid = self
            .session_id_override
            .clone()
            .or_else(|| self.session_db_id.get().cloned());

        if let Some(sid) = stall_sid {
            let state = Arc::clone(&self.state);
            if let Ok(rt) = tokio::runtime::Handle::try_current() {
                rt.spawn(async move {
                    if let Some(ref detectors) = state.stall_detectors {
                        if detectors.lock().await.remove(&sid).is_some() {
                            info!(session_id = %sid, "stall detector removed on session end");
                        }
                    }
                });
            }
        }

        // ── Terminate direct-connection session in the database (Case 2) ────
        if let Some(id) = self.session_db_id.get().cloned() {
            let db = Arc::clone(&self.state.db);
            let audit_logger = self.state.audit_logger.clone();
            let slack = self.state.slack.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    let session_repo = SessionRepo::new(db);
                    match session_repo
                        .set_terminated(&id, SessionStatus::Terminated)
                        .await
                    {
                        Ok(terminated_session) => {
                            info!(session_id = %id, "session terminated on transport disconnect");
                            // T060: Post session-ended summary as thread reply.
                            if let Some(ref s) = slack {
                                crate::orchestrator::session_manager::notify_session_ended(
                                    &terminated_session,
                                    "transport disconnected",
                                    s,
                                )
                                .await;
                            }
                        }
                        Err(err) => {
                            warn!(
                                %err,
                                session_id = %id,
                                "failed to terminate session on disconnect"
                            );
                        }
                    }
                    // Audit-log session termination (T061).
                    if let Some(ref logger) = audit_logger {
                        let entry = AuditEntry::new(AuditEventType::SessionTerminate)
                            .with_session(id.clone());
                        if let Err(err) = logger.log_entry(entry) {
                            warn!(%err, "audit log write failed (session terminate)");
                        }
                    }
                });
            }
            // If no runtime is active, the stale-session sweep in the next
            // `on_initialized` call will reclaim the session.
        }
    }
}
