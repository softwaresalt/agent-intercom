# Phase 13 Memory: 005-intercom-acp-server

**Date**: 2026-03-03
**Phase**: 13 — HITL Findings Remediation (Critical & High)
**Branch**: 005-intercom-acp-server
**Status**: COMPLETE (13/15 tasks, T118 and T121 deferred)

## Task Overview

Phase 13 addresses three HITL findings from adversarial testing:

| Finding | Severity | Description |
|---------|----------|-------------|
| HITL-003 | CRITICAL | MCP tools unreachable in ACP mode — HTTP transport disabled |
| HITL-005 | HIGH | session-checkpoint acts on wrong session (broken heuristic) |
| HITL-006 | HIGH | Interrupted sessions unmanageable after server restart |

## Current State

### Tasks Completed (13/15)

- [x] **T107–T109**: Integration tests for ACP HTTP bridge in `tests/integration/acp_mcp_bridge_tests.rs`
  - 4 tests: HTTP transport accessible, no-session-id → 401, invalid-session-id → 401, valid-session-id → not 401
- [x] **T110**: Enabled HTTP transport in ACP mode (`src/main.rs`)
  - Removed `args.mode == ServerMode::Mcp &&` guard from `start_sse` computation
  - Updated ACP info log to reflect transport IS starting
- [x] **T111**: Added `acp_session_guard` middleware to `src/mcp/sse.rs`
  - Rejects requests without valid `session_id` query param in ACP mode (HTTP 401)
  - Bypass for established MCP sessions (with `Mcp-Session-Id` header)
  - Wired as outermost layer on `/mcp` router
- [x] **T112**: Added `session_id_override()` accessor to `IntercomServer` in `src/mcp/handler.rs`
  - Updated `ask_approval` tool to prefer `session_id_override()` over first active session
- [x] **T113–T114**: Unit tests for `parse_checkpoint_args` in `tests/unit/command_tests.rs`
  - 5 tests: single dash arg, long dash label, no args, two args, extra args
- [x] **T115**: Fixed `parse_checkpoint_args` in `src/slack/commands.rs`
  - Removed broken heuristic (dash-containing single arg was misclassified as label)
  - 1 arg → always `session_id`; 0 args → (None, None); 2+ args → (session_id, label)
  - Made `pub` with `#[must_use]` for external test access
- [x] **T116–T117**: Unit tests for `find_interrupted_by_channel` in `tests/unit/command_tests.rs`
  - Tests that interrupted sessions are returned by channel, empty when none
- [x] **T119**: Added `find_interrupted_by_channel` to `SessionRepo` + updated `resolve_command_session`
  - Fallback: when `find_active_by_channel` returns empty, search interrupted sessions
  - `find_interrupted_by_channel` queries `status = 'interrupted'` ordered by `updated_at DESC`
- [x] **T120**: Added `handle_session_cleanup` handler and `session-cleanup` dispatch
  - Terminates all interrupted sessions in channel owned by user
  - Calls `set_terminated` + `acp_driver.deregister_session` for each
  - ACP-mode only (returns helpful error in MCP mode)

### Tasks Deferred

- [ ] **T118**: Integration test for `check_interrupted_on_startup` notification (S087, S088)
  - Deferred: requires mocking/intercepting Slack message posting in tests — complex setup
  - Current `check_interrupted_on_startup` behavior is tested implicitly via unit path
- [ ] **T121**: Update `check_interrupted_on_startup` to post Block Kit "Clear All" button
  - Deferred: requires new Block Kit action handler and modal/callback wiring
  - Low operational impact: operators can use `/arc session-cleanup` instead

## Quality Gates Passed

| Gate | Status |
|------|--------|
| `cargo check` | ✅ PASS |
| `cargo test --tests` (via CARGO_TARGET_DIR) | ✅ PASS — 326 unit + 250 integration + 33 contract + others |
| `cargo clippy -- -D warnings` | ✅ PASS (zero warnings) |
| `cargo fmt --all -- --check` | ✅ PASS |

**Note**: `cargo test` in the default target dir fails with "access denied" on `agent-intercom.exe` because two server processes from `run-debug.ps1` are holding the binary. Tests run cleanly with `CARGO_TARGET_DIR=target\test-build`.

## Important Discoveries

### HITL-003 Root Cause
- `src/main.rs` had `let start_sse = args.mode == ServerMode::Mcp && matches!(...)` — gating HTTP transport on MCP mode only
- ACP subprocesses spawn as `copilot --acp` and connect to the local MCP HTTP endpoint to call tools like `check_clearance`
- Without the HTTP transport, all MCP tools are unreachable from ACP sessions

### ACP Session Guard Design
- The guard runs as the outermost axum middleware layer (added last via `.layer()`)
- Bypass condition: `Mcp-Session-Id` header present (established MCP session after handshake)
- New connections require `?session_id=<id>` query param matching an `Active` DB session
- Health endpoint (`/health`) has no guard — it's on a different route before the guard layer

### parse_checkpoint_args Bug (HITL-005)
- Old code: if first arg contains `-` and length > 10, treat as `session_id`, else treat as `label`
- Bug: `"phase-13-checkpoint"` contains dashes and is > 10 chars → treated as `session_id`; `"phase-1"` → treated as `label` (WRONG)
- Fix: positional semantics only — no content inspection

### External Test Access to pub(crate) Functions
- `pub(crate)` functions are NOT accessible from `tests/` crates (external test crates are separate crates)
- `parse_checkpoint_args` needed to be `pub` (with `#[must_use]`) for unit tests in `tests/unit/`
- Alternative would be inline `#[cfg(test)]` modules in `src/slack/commands.rs`, but external tests are preferred by the project constitution

### Middleware Layer Order in Axum
- In axum, `.layer()` calls are applied in reverse order: last `.layer()` = outermost = runs first
- `acp_session_guard` must run BEFORE `ensure_accept_header` (accept header normalization is useless if we reject the request)
- BUT: `acp_session_guard` reads `session_id` from the raw URI BEFORE `ensure_accept_header` stores it in `PendingParams`
- This is fine because the guard extracts the param directly from the URI, not from `PendingParams`

## Files Modified

| File | Change |
|------|--------|
| `src/main.rs` | T110: Removed MCP-only guard on HTTP transport start |
| `src/mcp/sse.rs` | T111: Added `acp_session_guard` middleware + `ServerMode` + `SessionRepo` imports |
| `src/mcp/handler.rs` | T112: Added `session_id_override()` accessor |
| `src/mcp/tools/ask_approval.rs` | T112: Uses `session_id_override()` when present for session resolution |
| `src/slack/commands.rs` | T115: Fixed `parse_checkpoint_args` (pub + #[must_use] + doc); T119: resolve_command_session fallback; T120: session-cleanup dispatch + handler |
| `src/persistence/session_repo.rs` | T119: Added `find_interrupted_by_channel` method |
| `tests/unit/command_tests.rs` | NEW: T113/T114/T116/T117 unit tests |
| `tests/integration/acp_mcp_bridge_tests.rs` | NEW: T107/T108/T109 integration tests |
| `tests/unit.rs` | Added `mod command_tests;` |
| `tests/integration.rs` | Added `mod acp_mcp_bridge_tests;` |
| `specs/005-intercom-acp-server/tasks.md` | Marked completed tasks [x] |

## Next Steps

### Phase 14: Security Hardening (ES-004, ES-010, ES-008)
- T122–T130: Process tree kill (Job Objects/setsid), host_cli path validation, partial write protection
- Start with T122 (process tree kill tests) — highest priority security finding

### Deferred from Phase 13
- T118: Integration test for startup notification — needs Slack mock
- T121: "Clear All" Block Kit button in startup notification — needs new Slack handler

### Known Issues
- Socket Mode WebSocket drops still produce `dispatch_failed` for ~2 minutes (F-001/F-006)
- This is separate from Phase 13 scope (HITL-001 — tracked in Phase 15)

## Context to Preserve

- `SessionRepo::find_interrupted_by_channel` returns sessions ordered by `updated_at DESC` (most recent first)
- `set_terminated` bypasses `is_valid_transition` so `Interrupted → Terminated` is valid
- The `acp_driver.deregister_session` call is gated on `state.acp_driver.is_some()` — graceful degradation in MCP mode
- `serve_with_listener` is `pub` and takes a pre-bound `TcpListener` — use this in integration tests for deterministic ports
