---
id: TASK-001.10.05
title: "001-002 - User Story 1 — MCP Transport Dispatch (Priority: P1, FR-001)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001.10
dependencies: []
ordinal: 1150
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Verify full MCP tool dispatch through the HTTP/SSE transport — JSON argument parsing, tool-router matching, handler execution, and response construction. Covers SCENARIOS.md S001–S010.

**Independent Test**: `cargo test --test integration mcp_dispatch_tests` passes with 5+ tests covering tool dispatch via HTTP transport.

**Risk**: rmcp `RequestContext` has no public constructor. Tests must go through the SSE transport layer. If the MCP handshake is too complex, fall back to transport-level smoke tests verifying endpoint accepts requests and returns valid JSON-RPC responses.

### Tests (TDD — write first, verify compilation, then run)

- [X] T022 [P] [US1] Write `transport_heartbeat_dispatch` test (S001) in `tests/integration/mcp_dispatch_tests.rs` — Start `serve_sse()` on ephemeral port, create active session, send heartbeat tool call via MCP client/HTTP, verify `acknowledged: true` in response
- [X] T023 [P] [US1] Write `transport_set_mode_dispatch` test (S002) in `tests/integration/mcp_dispatch_tests.rs` — Send set_operational_mode via transport, verify mode change response
- [X] T024 [P] [US1] Write `transport_recover_state_dispatch` test (S003) in `tests/integration/mcp_dispatch_tests.rs` — Send recover_state via transport, verify clean state response
- [X] T025 [P] [US1] Write `transport_unknown_tool_returns_error` test (S006) in `tests/integration/mcp_dispatch_tests.rs` — Send unknown tool name, verify error response
- [X] T026 [P] [US1] Write `transport_malformed_args_returns_error` test (S007) in `tests/integration/mcp_dispatch_tests.rs` — Send tool call with invalid JSON arguments, verify descriptive error

### Verification

- [X] T027 [US1] Run `cargo test --test integration mcp_dispatch_tests` and verify all tests pass
- [X] T028 [US1] Run `cargo clippy -- -D warnings` and verify zero warnings from new test code

**Checkpoint**: MCP transport dispatch tests complete. FR-001 satisfied. `cargo test --test integration mcp_dispatch_tests` passes.

---

<!-- SECTION:DESCRIPTION:END -->
