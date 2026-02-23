# Phase 3 Memory: IPC Server Tests (US7/FR-008)

**Feature Spec**: 001-002-integration-test  
**Phase**: 3 — IPC Server Command Dispatch  
**Completed**: 2026-02-22  
**Commit**: (pending — to be recorded after git commit)  
**Tasks**: T012–T021

---

## Work Summary

Created 8 integration tests in `tests/integration/ipc_server_tests.rs` verifying:
- Auth token enforcement (valid, invalid, missing — S053/S054/S055)
- `list` command returns active sessions from DB (S057)
- `approve` resolves a pending oneshot approval (S059)
- `reject` resolves with reason string (S060)
- `resume` resolves pending wait oneshot (S062)
- `mode` updates session mode in DB (S064)

---

## Key Discoveries

1. **`ipc_app_state()` must be sync**: The function builds `AppState` but has no `.await` points — `clippy::unused_async` will flag it if declared async. Made sync; removed `.await` from all 8 call sites.

2. **`SessionRepo::get_by_id()` not `get()`**: The repository method for fetching a session by ID is `get_by_id(&id)`, not `get(&id)`.

3. **`unique_ipc_name()` pattern**: Each IPC test must use a UUID-based pipe name (`format!("ti{}", Uuid::new_v4().simple())`) to avoid cross-test named-pipe conflicts on Windows.

4. **`send_ipc()` uses `spawn_blocking`**: The interprocess client is blocking; wrapped in `tokio::task::spawn_blocking` with a 20-retry loop (50ms intervals) to handle server startup race.

5. **`unregister_stops_watching` flakiness (Phase 2 fix applied in Phase 3)**: Added 100ms drain `tokio::time::sleep` after `PolicyWatcher::unregister()` to allow OS-level watcher deregistration before writing to disk. Fixed intermittent failure in full suite.

---

## Test Count Delta

- Start of Phase 3: 487 tests
- End of Phase 3: 495 tests (+8 IPC tests)

---

## Files Modified

- `tests/integration/ipc_server_tests.rs` — created (447 lines, 8 tests)
- `tests/integration/policy_watcher_tests.rs` — patched (100ms drain sleep fix)
- `specs/001-002-integration-test/tasks.md` — T012–T021 marked [X]

---

## Phase 4 Handoff Notes

### Target: MCP Dispatch Tests (T022–T028)

File: `tests/integration/mcp_dispatch_tests.rs` (empty stub exists)

**HIGH RISK**: `rmcp::RequestContext` has no public constructor — tests must go through the full HTTP/SSE transport layer. Strategy:
1. Start `axum` server with `StreamableHttpService` on a random port
2. Send raw HTTP POST requests with JSON-RPC bodies
3. Assert well-formed JSON-RPC responses

5 target tests:
- `transport_heartbeat_dispatch` (S001)
- `transport_set_mode_dispatch` (S002)
- `transport_recover_state_dispatch` (S003)
- `transport_unknown_tool_returns_error` (S009)
- `transport_malformed_args_returns_error` (S010)

Read `src/mcp/sse.rs` first to understand how `StreamableHttpService` is configured.
