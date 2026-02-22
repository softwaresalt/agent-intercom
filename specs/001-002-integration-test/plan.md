# Implementation Plan: Integration Test Full Coverage

**Branch**: `001-002-integration-test` | **Date**: 2026-02-22 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-002-integration-test/spec.md`

## Summary

Add integration tests to fill three coverage gaps in the existing test suite: (1) policy hot-reload via `PolicyWatcher`, (2) IPC server command dispatch and auth enforcement, and (3) full MCP tool dispatch through the HTTP/SSE transport. The existing 23 integration test modules (150+ test functions) already cover FR-002 through FR-006 and FR-009 through FR-011. This plan focuses exclusively on the gaps identified in [research.md](research.md): FR-001 (MCP dispatch), FR-007 (policy hot-reload), and FR-008 (IPC server). No new production code is required — only new test modules.

## Technical Context

**Language/Version**: Rust stable, edition 2021
**Primary Dependencies**: `rmcp` 0.5, `axum` 0.8, `tokio` 1.37, `sqlx` 0.8, `interprocess` 2.0, `notify` 6.1, `tempfile` 3.10
**Storage**: SQLite in-memory via sqlx (test-only)
**Testing**: `cargo test` — unit, contract, integration tiers in `tests/` directory
**Target Platform**: Windows workstations (primary), Linux servers (secondary)
**Project Type**: Single workspace, two binaries
**Performance Goals**: N/A — tests only; all timeouts ≤ 5s per assertion
**Constraints**: No external services (no real Slack, no real IPC connections unless testing IPC specifically); zero regressions in existing tests
**Scale/Scope**: 3 new integration test modules, ~19 new test functions, 0 production code changes

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Safety-First Rust | PASS | Tests use `#![allow(clippy::expect_used, clippy::unwrap_used)]` per existing harness convention. No production code changes. |
| II. MCP Protocol Fidelity | PASS | No change to tool surface. Tests verify existing tool dispatch correctness. |
| III. Test-First Development | PASS | This feature **is** the tests. All new code is test code. |
| IV. Security Boundary Enforcement | PASS | IPC auth tests verify security enforcement. No relaxation of boundaries. |
| V. Structured Observability | PASS | No change — tests don't emit production traces. |
| VI. Single-Binary Simplicity | PASS | No new dependencies. All test deps already in `Cargo.toml`. |

### Post-Design Re-evaluation (Phase 1 complete)

No production code changes and no new dependencies. All constitution principles pass without violation. No complexity tracking needed.

## Project Structure

### Documentation (this feature)

```text
specs/001-002-integration-test/
├── spec.md              # Feature specification
├── plan.md              # This file
├── research.md          # Gap analysis and test strategy research
├── quickstart.md        # Build/test instructions
├── checklists/
│   └── requirements.md  # Specification quality checklist
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
tests/
├── integration.rs       # UPDATE: Register 3 new modules
├── integration/
│   ├── test_helpers.rs  # EXISTING: Shared test infrastructure (no changes expected)
│   ├── ...              # EXISTING: 23 integration test modules (no changes)
│   ├── policy_watcher_tests.rs   # NEW: FR-007 — Policy hot-reload integration
│   ├── ipc_server_tests.rs       # NEW: FR-008 — IPC server command dispatch + auth
│   └── mcp_dispatch_tests.rs     # NEW: FR-001 — Full MCP transport dispatch
```

**Structure Decision**: Single project structure. All new code is test modules in the existing `tests/integration/` directory. No new directories, no production code changes, no new dependencies. The 3 new modules are registered in `tests/integration.rs` via `mod` declarations, following the existing pattern.

## Existing Coverage Summary

The following spec requirements are **already fully covered** by existing integration tests and require no additional work:

| FR | Existing Coverage | Test Module(s) |
|---|---|---|
| FR-002 | Health endpoint 200 OK, 404 for unknown routes | `health_endpoint_tests` |
| FR-003 | Session pause/resume/terminate, invalid transitions, max concurrent | `session_manager_tests`, `session_lifecycle_tests` |
| FR-004 | Checkpoint file hashes, Modified/Deleted/Added divergences | `checkpoint_manager_tests`, `session_lifecycle_tests` |
| FR-005 | Stalled → AutoNudge → Escalated sequence | `stall_escalation_tests` |
| FR-006 | Stall detector reset, pause, resume, cancel | `stall_escalation_tests` |
| FR-009 | Shutdown marks pending entities Interrupted | `shutdown_recovery_tests` |
| FR-010 | Startup recovery finds interrupted sessions | `shutdown_recovery_tests`, `crash_recovery_tests` |
| FR-011 | In-memory SQLite, no external services | All tests |

## Gap Coverage Plan

### Gap 1: Policy Hot-Reload (FR-007) — `policy_watcher_tests.rs`

**What's missing**: No test exercises `PolicyWatcher::register()` → file modification → `get_policy()` reflecting updated policy. Unit tests cover `PolicyLoader::load()` and `PolicyEvaluator::check()` statically, but the `notify`-based file watcher is untested.

**Test plan** (6 tests):
1. `register_loads_initial_policy` — Register watcher on workspace with valid policy file → `get_policy()` returns parsed policy
2. `policy_file_modification_detected` — Modify `.monocoque/settings.json` after register → poll `get_policy()` until updated policy reflected (50ms poll, 2s timeout)
3. `policy_file_deletion_falls_back_to_deny_all` — Delete policy file after register → poll until `get_policy()` returns deny-all default
4. `malformed_policy_file_uses_deny_all` — Write malformed JSON → verify evaluator uses deny-all
5. `unregister_stops_watching` — Call `unregister()` → modify file → verify policy does NOT update
6. `multiple_workspaces_independent_policies` — Register two workspaces → modify one → verify only that workspace's policy changes

**Infrastructure**: `tempfile::tempdir()` per test. Create `.monocoque/` subdirectory. Write initial `settings.json`. Use `PolicyWatcher::new()` with empty global commands (or test-specific allowlist).

### Gap 2: IPC Server Command Dispatch (FR-008) — `ipc_server_tests.rs`

**What's missing**: No test starts `spawn_ipc_server()`, connects a client, sends JSON-line commands, or verifies auth enforcement. The only IPC-related tests check `AppState.ipc_auth_token` field existence.

**Test plan** (8 tests):
1. `ipc_valid_auth_token_accepted` — Start IPC server with auth token → send `list` command with valid token → success response
2. `ipc_invalid_auth_token_rejected` — Send command with wrong token → unauthorized error
3. `ipc_missing_auth_token_rejected` — Send command without token → unauthorized error
4. `ipc_list_returns_active_sessions` — Create sessions in DB → `list` → response contains session IDs
5. `ipc_approve_resolves_pending_approval` — Create pending approval with oneshot sender → send `approve` → sender resolves with Approved status
6. `ipc_reject_resolves_with_reason` — Create pending approval → send `reject {reason}` → sender resolves with Rejected + reason
7. `ipc_resume_resolves_pending_wait` — Create pending wait → send `resume` → sender resolves
8. `ipc_mode_changes_session_mode` — Create active session → send `mode {new_mode}` → verify DB updated

**Infrastructure**: Each test creates a `GlobalConfig` with unique `ipc_name` (e.g., `monocoque-test-{uuid}`). Opens an `interprocess::local_socket::LocalSocketStream` client. Sends/receives JSON lines delimited by `\n`. Uses `tokio::time::timeout(Duration::from_secs(5), ...)` for all async assertions.

### Gap 3: MCP Transport Dispatch (FR-001) — `mcp_dispatch_tests.rs`

**What's missing**: Tests exercise tool logic at the repository/orchestrator layer but bypass the actual `ServerHandler::call_tool()` dispatch path. `RequestContext<RoleServer>` (from rmcp) has no public constructor, so direct `call_tool()` invocation is not possible.

**Approach**: Test via the HTTP/SSE transport — start `serve_sse()` on an ephemeral port, then send raw JSON-RPC tool-call requests to the `/message` endpoint and verify responses.

**Test plan** (5 tests):
1. `transport_heartbeat_dispatch` — Send heartbeat tool call via HTTP → verify `acknowledged: true` in response
2. `transport_set_mode_dispatch` — Send set_operational_mode tool call → verify mode change response
3. `transport_recover_state_dispatch` — Send recover_state → verify clean state response
4. `transport_unknown_tool_returns_error` — Send unknown tool name → verify error response
5. `transport_malformed_args_returns_error` — Send tool call with invalid JSON arguments → verify descriptive error

**Infrastructure**: Start `serve_sse()` using the same pattern as `health_endpoint_tests.rs` (ephemeral port, `CancellationToken`). The challenge is that SSE transport requires an MCP session initialization handshake before tool calls can be sent. If the rmcp client SDK provides a programmatic API (`rmcp::transport::sse::SseClientTransport`), use it. Otherwise, implement the JSON-RPC envelope manually over HTTP.

**Risk mitigation**: If the MCP handshake is too complex to set up in tests (SSE streams, session negotiation), reduce this module to a transport-level smoke test: verify that the `/mcp` endpoint accepts POST requests and returns valid JSON-RPC responses, without completing the full MCP session lifecycle. The per-tool logic is already thoroughly tested at the repository layer.

## Dependency Map

```
policy_watcher_tests ──→ PolicyWatcher, PolicyLoader (src/policy/)
                     ──→ tempfile, notify (existing deps)

ipc_server_tests     ──→ spawn_ipc_server (src/ipc/server.rs)
                     ──→ interprocess (existing dep)
                     ──→ test_helpers (tests/integration/test_helpers.rs)

mcp_dispatch_tests   ──→ serve_sse (src/mcp/sse.rs)
                     ──→ rmcp client SDK or raw HTTP (existing deps)
                     ──→ test_helpers (tests/integration/test_helpers.rs)
```

All three modules are independent — no ordering dependencies between them. They can be implemented in any order or in parallel.

## Implementation Order Recommendation

1. **`policy_watcher_tests`** — Lowest risk, well-understood API surface (`PolicyWatcher::register/get_policy`). Uses only filesystem + in-memory state.
2. **`ipc_server_tests`** — Medium risk. Uses real named pipes but unique names per test. Auth testing is straightforward.
3. **`mcp_dispatch_tests`** — Highest risk. Depends on rmcp client SDK capabilities which may require experimentation. May need to fall back to smoke-test scope.

## Complexity Tracking

No constitution violations. No complexity entries required.

## Quality Gates Verification

After implementation, all gates must pass:

1. `cargo check` — zero errors
2. `cargo clippy -- -D warnings` — zero warnings
3. `cargo fmt --all -- --check` — no violations
4. `cargo test` — all existing + new tests pass (zero failures)
