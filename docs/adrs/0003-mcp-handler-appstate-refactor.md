# ADR-0003: MCP Server Handler Refactored from McpServer to AgentRcServer with AppState

**Status**: Accepted
**Date**: 2026-02-11
**Phase**: 2 (Foundational), Tasks T029-T033

## Context

Phase 1 created a minimal `McpServer` struct in `src/mcp/server.rs` that held only a `_config: Arc<GlobalConfig>` field and placeholder tool definitions with no schemas. The MCP tool handlers need access to the database, Slack client, and configuration to implement real functionality in Phase 3+.

## Decision

Renamed `server.rs` to `handler.rs` and replaced `McpServer` with two structs:

- **`AppState`** — shared application state holding `Arc<GlobalConfig>`, `Arc<Surreal<Db>>`, and `Option<Arc<SlackService>>`. Constructed once during bootstrap and shared across all transport connections.
- **`AgentRcServer`** — MCP `ServerHandler` implementation that holds `Arc<AppState>`. One instance per transport connection (stdio gets one, each SSE connection gets its own).

All nine tool definitions now carry full JSON schemas matching the contract in `contracts/mcp-tools.json`. Tool handlers still return "not implemented" errors, but the router infrastructure is ready for Phase 3 wiring.

Added three new modules:
- `context.rs` — `ToolContext` struct bundling session + workspace + infrastructure for per-request use.
- `transport.rs` — stdio transport setup using `rmcp::transport::io::stdio()`.
- `sse.rs` — HTTP/SSE transport using `SseServer` with axum and per-connection `AgentRcServer` instances.

## Consequences

**Positive**:
- Clean separation between shared state (`AppState`) and per-connection server (`AgentRcServer`).
- Tool schemas are contract-tested and match the spec.
- Transport modules are independently testable.

**Negative**:
- `all_tools()` is a ~200-line function (allowed via `#[allow(clippy::too_many_lines)]`).
- `AppState.slack` is `Option` during Phase 2 since Slack client initialization is deferred.
