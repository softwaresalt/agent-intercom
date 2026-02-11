//! Model Context Protocol server handler and tool router scaffolding.

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

use crate::config::GlobalConfig;

/// MCP server implementation that exposes the project's tool surface.
pub struct McpServer {
    _config: Arc<GlobalConfig>,
}

impl McpServer {
    /// Create a new MCP server bound to shared configuration.
    #[must_use]
    pub fn new(config: Arc<GlobalConfig>) -> Self {
        Self { _config: config }
    }

    fn tool_router() -> ToolRouter<Self> {
        let mut router = ToolRouter::new();

        for tool in Self::all_tools() {
            router.add_route(ToolRoute::new_dyn(tool, |_context| {
                Box::pin(async {
                    Err(rmcp::ErrorData::internal_error(
                        "tool not implemented",
                        None,
                    ))
                })
            }));
        }

        router
    }

    fn all_tools() -> Vec<Tool> {
        let tools = [
            "ask_approval",
            "accept_diff",
            "check_auto_approve",
            "forward_prompt",
            "remote_log",
            "recover_state",
            "set_operational_mode",
            "wait_for_instruction",
            "heartbeat",
        ];

        tools
            .iter()
            .map(|name| Tool {
                name: (*name).into(),
                description: None,
                input_schema: Arc::new(serde_json::Map::default()),
                output_schema: None,
                annotations: None,
            })
            .collect()
    }
}

impl ServerHandler for McpServer {
    fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_ {
        let router = Self::tool_router();

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
