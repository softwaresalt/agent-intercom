# Phase 4 Memory — 001-002-integration-test

**Date**: 2026-02-22  
**Spec**: `001-002-integration-test`  
**Phase**: 4 — MCP Transport Dispatch Tests  
**Commit**: `f32a046`  
**Branch**: `001-002-integration-test`

---

## What Was Built

Added `tests/integration/mcp_dispatch_tests.rs` — 5 integration tests covering
FR-001 MCP tool dispatch via the live SSE transport layer.

### Files Added/Modified

| File | Change |
|---|---|
| `tests/integration/mcp_dispatch_tests.rs` | New — hand-rolled SSE client + 5 tests |
| `specs/001-002-integration-test/tasks.md` | T022–T028 marked `[X]` |

### Tests Implemented

| Test | Scenario | Assertion |
|---|---|---|
| `transport_heartbeat_dispatch` | S001 | `acknowledged: true` |
| `transport_list_tools_returns_nine_tools` | list_tools | exactly 9 tools |
| `transport_recover_state_dispatch` | S003 | `status: "clean"` |
| `transport_unknown_tool_returns_error` | S006 | `error` present |
| `transport_malformed_args_returns_error` | S007 | `error` present |

---

## Key Technical Decisions

### SseConnection Helper

Hand-rolled `SseConnection` struct connects end-to-end via the real SSE
transport (`serve_sse()` → `SseServer`). Uses `reqwest::Client` with:

- `GET /sse` → `event: endpoint` → message URL (one-shot delivery via
  `Option<oneshot::Sender<String>>` to avoid move-in-loop)
- `response.chunk()` streaming loop (no `bytes_stream` — no `futures` dep)
- `POST /message?sessionId=xxx` with `serde_json::to_string()` body +
  `Content-Type: application/json` header (no `json` feature in reqwest)
- Background `tokio::spawn` task reads SSE events and fans out to
  `mpsc::Sender<String>` (data) + `oneshot::Sender<String>` (endpoint)

### Critical Invariants

- **`reqwest` has no `json` feature** — always use
  `.body(serde_json::to_string(&payload)?)` with explicit `Content-Type` header
- **`response.chunk()` requires `mut response`**
- **`endpoint_tx_opt: Option<oneshot::Sender<_>>`** pattern required — Sender
  takes ownership on `send()`, can't be used again in next loop iteration
- **`on_initialized` auto-creates an active session** when SSE connects without
  `session_id_override` — heartbeat test works without pre-creating a session
- **`recover_state` returns `{"status":"clean"}`** (NOT `interrupted_sessions`
  array) when no interrupted sessions exist

### Clippy Fixes Applied

- `while_let_loop`: `loop { let Some(nl) = ... else { break }; ... }`
  → `while let Some(nl) = buf.find('\n') { ... }`
- `manual_strip`: `line["event:".len()..]` after `starts_with("event:")`
  → `if let Some(event_type) = line.strip_prefix("event:") { ... }`

---

## Quality Gates Result

| Gate | Result |
|---|---|
| `cargo check` | ✅ |
| `cargo clippy --all-targets -- -D warnings` | ✅ 0 warnings |
| `cargo fmt --all -- --check` | ✅ 0 violations |
| `cargo test` (500 total) | ✅ 0 failures |

---

## Handoff to Phase 5

Phase 5 tasks (T029–T032) are all cross-cutting polish:
- T029: Full `cargo test` — zero failures
- T030: Full clippy workspace — zero warnings  
- T031: `cargo fmt --all -- --check` — zero violations
- T032: Verify 24 integration modules unaffected

**All Phase 5 gates were already satisfied during Phase 4 completion** — the
full suite ran clean (500 tests, 0 failures) and clippy + fmt both passed.
Phase 5 should be a quick verification pass followed by a final commit.
