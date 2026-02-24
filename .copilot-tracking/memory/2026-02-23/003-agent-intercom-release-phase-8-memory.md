# Phase 8 Memory — 003-agent-intercom-release

**Date**: 2026-02-23  
**Spec**: 003-agent-intercom-release  
**Phase**: 8 — US5: rmcp 0.13 Upgrade  
**Tasks**: T094–T109  
**Commit**: b6f5898  
**Tests**: 553 passing (0 failures)  
**Branch**: 003-agent-intercom-release

---

## What Was Built

Upgraded rmcp from 0.5 to 0.13.0 and replaced the SSE-based transport with
the new StreamableHttpService (Streamable HTTP) transport. This is a protocol-
level breaking change: the two-endpoint model (GET /sse + POST /message) is
replaced by a single POST /mcp endpoint with Mcp-Session-Id header tracking.

---

## Files Modified

| File | Change |
|------|--------|
| `Cargo.toml` | rmcp `0.5` → `0.13.0`; feature `transport-sse-server` → `transport-streamable-http-server` |
| `src/mcp/sse.rs` | Complete rewrite: `SseServer` → `StreamableHttpService` + `LocalSessionManager`; new `serve_http()` fn; `/sse` → 410 Gone; `serve_sse()` deprecated alias |
| `src/mcp/handler.rs` | Added `title: None, icons: None, meta: None` to all 9 `Tool` structs; `..Default::default()` on `Implementation` and `ListResourcesResult` |
| `src/mcp/resources/slack_channel.rs` | Added new optional fields to `RawResource`, `RawResourceTemplate`, `ListResourcesResult`, `ListResourceTemplatesResult` |
| `src/main.rs` | `serve_sse` → `serve_http` |
| `tests/integration/mcp_dispatch_tests.rs` | Complete rewrite: `SseConnection` → `McpConnection` using Streamable HTTP protocol |
| `tests/integration/health_endpoint_tests.rs` | `serve_sse` → `serve_http` |
| `tests/integration/streamable_http_tests.rs` | NEW — 4 tests gated behind `#[cfg(feature = "rmcp-upgrade")]` |
| `tests/integration/stdio_transport_tests.rs` | NEW — 1 compile-time stability test for `serve_stdio` |
| `tests/integration.rs` | Added `mod stdio_transport_tests;` and `mod streamable_http_tests;` |

---

## Key Technical Facts

### rmcp 0.13 Breaking Changes Resolved

1. **`sse_server` module removed** → `streamable_http_server::{StreamableHttpService, StreamableHttpServerConfig, session::local::LocalSessionManager}`
2. **`Tool` struct** — does NOT derive `Default`; must set `title: None, icons: None, meta: None` explicitly
3. **`Implementation` struct** — HAS custom `Default` impl; use `..Default::default()` for new optional fields
4. **`RawResource`** — does NOT derive `Default`; explicit `None`s for `icons`, `meta`, `title`
5. **`RawResourceTemplate`** — does NOT derive `Default`; explicit `None`s for `icons`, `title`
6. **`ListResourcesResult` / `ListResourceTemplatesResult`** — DO derive `Default` via `paginated_result!` macro; use `..Default::default()`

### Protocol Change: SSE → Streamable HTTP

**Old (rmcp 0.5)**:
- `GET /sse` → receives `endpoint` event with session URL
- `POST /message?sessionId=<id>` → send requests

**New (rmcp 0.13)**:
- `POST /mcp` with headers `Content-Type: application/json`, `Accept: application/json, text/event-stream`
- Response header `Mcp-Session-Id: <session-id>` from initialization
- Subsequent requests: include `Mcp-Session-Id: <session-id>` header

### McpConnection Implementation Pattern

```rust
struct McpConnection {
    base_url: String,
    client: Client,
    session_id: Option<String>,
}

impl McpConnection {
    fn new(base_url: &str) -> Self { ... }
    async fn handshake(&mut self) -> Value { ... }  // returns init response
    async fn request(&mut self, method: &str, params: Value) -> Value { ... }
}
```

The `request()` method detects `text/event-stream` content-type and extracts
`data:` lines from SSE chunks when needed for SSE responses.

### Backward Compat

- `/sse` returns `410 Gone` with message: "This endpoint has been removed. Use POST /mcp with the Streamable HTTP protocol."
- `serve_sse()` is kept as a `#[deprecated(since = "0.2.0")]` alias for `serve_http()`

---

## Quality Gates (all passed)

- `cargo check` ✅
- `cargo test` ✅ — 553 tests, 0 failures
- `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` ✅ — 0 issues
- `cargo fmt --all -- --check` ✅ — clean

---

## Phase 9 Preview (T110–T116)

T110: Name reference audit — verify zero "monocoque" references remain outside docs/history  
T111: Update `.github/copilot-instructions.md` — final sweep  
T112: Audit `.github/agents/` for stale references  
T113: Verify SC-001 through SC-008 automated success criteria  
T114: Full quality gate (final)  
T115: Execute quickstart.md validation steps  
T116: Draft constitution amendment for rmcp 0.13 + binary name changes  
