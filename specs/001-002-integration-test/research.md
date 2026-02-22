# Research: Integration Test Full Coverage

**Feature**: 001-002-integration-test | **Date**: 2026-02-22 | **Spec**: [spec.md](spec.md)

## Research Tasks

### RT-001: Coverage Gap Analysis — Existing vs. Specified

**Decision**: Extensive integration test infrastructure already exists (23 test modules, 150+ test functions). Three coverage gaps remain.

**Findings**:

The existing test suite covers most of the spec requirements. The gap analysis reveals:

| FR | Status | Gap |
|---|---|---|
| FR-001 | PARTIAL | Tests exercise logic layer, not `ServerHandler::call_tool()` trait dispatch. `RequestContext<RoleServer>` from rmcp has no public constructor — tests cannot invoke `call_tool()` directly in integration tests. All 9 tools have logic-layer coverage via repos/orchestrator. |
| FR-002 | COVERED | `health_returns_ok`, `non_existent_route_returns_404`, `health_without_trailing_slash_works` |
| FR-003 | COVERED | `session_manager_tests.rs` (12 tests), `session_lifecycle_tests.rs` (7 tests) |
| FR-004 | COVERED | `checkpoint_manager_tests.rs` (9 tests) |
| FR-005 | COVERED | `stall_escalation_tests.rs::full_escalation_chain` |
| FR-006 | COVERED | `stall_escalation_tests.rs` (6 reset/pause/resume tests) |
| FR-007 | MISSING | `PolicyWatcher::register()` → file change → `get_policy()` is untested. Hot-reload integration test needed. |
| FR-008 | MISSING | No IPC server integration tests exist. `spawn_ipc_server`, `dispatch_command`, auth enforcement are untested. |
| FR-009 | COVERED | `shutdown_recovery_tests.rs` (5 tests) |
| FR-010 | COVERED | `shutdown_recovery_tests.rs` (3 tests) + `crash_recovery_tests.rs` (6 tests) |
| FR-011 | COVERED | All tests use `db::connect_memory()` |
| FR-012/013 | CONDITIONAL | Pass when `cargo test` / `cargo clippy` succeeds |

**Alternatives considered**: None — gap analysis is factual.

---

### RT-002: Testing `ServerHandler::call_tool()` via Transport (FR-001)

**Decision**: Test `call_tool()` dispatch by starting the HTTP/SSE transport, initializing an rmcp client, and sending tool requests through the full MCP protocol stack.

**Rationale**: The rmcp `RequestContext<RoleServer>` type is constructed internally by the rmcp transport layer and has no public constructor. Integration tests cannot call `ServerHandler::call_tool()` directly. However, we can test the full dispatch path by connecting an MCP client to the SSE transport.

**Approach**:
1. Start `serve_sse()` on an ephemeral port with a fresh in-memory AppState.
2. Create an active session in the database.
3. Use `rmcp` client SDK to connect to the SSE endpoint.
4. Send `call_tool` requests through the client and verify responses.
5. This tests: JSON argument parsing, tool-router matching, handler dispatch, stall detector reset, and response construction.

**Risk**: The rmcp client SDK may not expose a simple programmatic API for sending tool calls. If so, fall back to raw HTTP POST to the `/message` endpoint with the JSON-RPC payload, which still exercises the full dispatch path.

**Alternatives considered**:
- Making `RequestContext` construction public: Rejected — it's owned by the rmcp crate, not our code.
- Testing only at the logic layer: Rejected — while all 9 tools have logic-layer tests, the actual dispatch path (JSON parsing, router matching, response construction) would remain untested.
- `#[cfg(test)]` helper in handler.rs: Rejected — adds test-only code to production module. The transport-based approach tests the real path.

---

### RT-003: Policy Hot-Reload Test Reliability (FR-007)

**Decision**: Use `tempdir` + `PolicyWatcher::register()` + direct file modification + polling loop for convergence.

**Rationale**: The `notify` crate's file watcher relies on OS-level filesystem events which can be delayed, batched, or missed in test environments. Tests need deterministic verification.

**Approach**:
1. Create a `tempdir` with `.monocoque/settings.json` containing initial policy.
2. Call `PolicyWatcher::register(workspace_root)` to start the watcher.
3. Verify initial policy via `get_policy(workspace_root)`.
4. Modify the policy file (overwrite with new content).
5. Poll `get_policy()` with a short interval (50ms) up to a timeout (2s) until the policy reflects the change, or fail.
6. Test deletion: remove the file, poll until `get_policy()` returns deny-all default.

**Risk**: Filesystem event delivery timing varies across platforms. Using a poll-with-timeout pattern (rather than fixed sleep) makes tests robust without being slow.

**Alternatives considered**:
- Fixed `tokio::time::sleep` after file modification: Rejected — brittle on slow CI runners.
- Injecting a test notification channel: Rejected — requires modifying production `PolicyWatcher` code. The poll pattern works without production code changes.

---

### RT-004: IPC Named Pipe Testing Strategy (FR-008)

**Decision**: Start the real IPC server and connect a real `interprocess` client in tests. Use unique pipe names per test to avoid conflicts.

**Rationale**: `interprocess` named pipes (Windows) / Unix domain sockets work reliably in userspace without elevated permissions. Each test generates a unique IPC name to prevent collisions.

**Approach**:
1. Create config with a unique `ipc_name` (e.g., `monocoque-test-{uuid}`).
2. Set `ipc_auth_token = Some("test-token".to_owned())` on AppState.
3. Call `spawn_ipc_server(state, ct)` to start the server.
4. Connect a `interprocess::local_socket::LocalSocketStream` client.
5. Send JSON-line commands and verify responses.
6. Test valid auth token → success, invalid auth token → unauthorized, missing auth token → unauthorized.
7. Test commands: `list` (returns sessions), `approve` (resolves pending), `mode` (changes mode).

**Risk**: Named pipe creation may fail if a previous test left a stale pipe. Using UUIDs in pipe names eliminates this.

**Alternatives considered**:
- Unit testing dispatch functions only: Rejected — the spec requires auth enforcement testing via the IPC server, not just the dispatch functions.
- Mocking the transport: Rejected — no mock exists for `interprocess`, and the real transport is fast and reliable in tests.

---

### RT-005: SSE Query Parameter Extraction End-to-End (FR-002 supplement)

**Decision**: The existing `channel_override_tests.rs` validates channel/session ID propagation at the `AgentRcServer` level. The `health_endpoint_tests.rs` validates HTTP routing. These together provide sufficient coverage.

**Rationale**: Testing SSE query-parameter extraction via actual SSE connections would require maintaining a long-lived SSE stream, sending MCP messages through it, and verifying the `AgentRcServer` received the correct overrides. The existing tests cover the same logic paths with lower complexity.

**Alternatives considered**:
- Full SSE stream test: Rejected — high complexity for marginal additional coverage. The `extract_channel_id`/`extract_session_id` functions have 5 unit tests in `src/mcp/sse.rs`, and `channel_override_tests.rs` verifies the values propagate correctly.

---

### RT-006: Test Isolation and Parallelism

**Decision**: All new tests use independent in-memory SQLite databases, unique tempdir paths, and unique IPC pipe names. No `serial_test` required for new tests.

**Rationale**: Each test creates its own `db::connect_memory()` pool and `tempfile::tempdir()`. This ensures complete isolation without requiring sequential execution.

**Alternatives considered**:
- Shared test database with cleanup: Rejected — in-memory per-test is simpler and eliminates cleanup failures.
- `serial_test` for IPC tests: Considered but rejected — unique pipe names per test eliminate the need.
