//! Integration tests for full MCP tool dispatch through the HTTP/SSE transport.
//!
//! Validates:
//! - S001: Heartbeat tool call dispatched via HTTP transport
//! - S002: `set_operational_mode` tool call dispatched via HTTP transport
//! - S003: `recover_state` tool call dispatched via HTTP transport
//! - S006: Unknown tool name returns MCP error response
//! - S007: Malformed arguments return descriptive MCP error
//!
//! FR-001 — MCP Transport Dispatch
