# Tasks: Integration Test Full Coverage

**Input**: Design documents from `/specs/001-002-integration-test/`
**Prerequisites**: plan.md (required), spec.md (required), SCENARIOS.md (behavioral matrix)

**Tests**: This feature IS tests. All tasks produce test code. TDD applies — tests are written, verified to compile, then executed against existing production code.

**Organization**: Tasks are grouped by coverage gap (mapped to spec user stories). User stories with existing full coverage (US2–US5, US8–US9) require no new work.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US6, US7, US1)
- Include exact file paths in descriptions

## Phase 1: Setup (Test Module Registration)

**Purpose**: Register new integration test modules in the test harness entry point

- [X] T001 Register `policy_watcher_tests` module in `tests/integration.rs`
- [X] T002 Register `ipc_server_tests` module in `tests/integration.rs`
- [X] T003 Register `mcp_dispatch_tests` module in `tests/integration.rs`

**Checkpoint**: `cargo check --test integration` compiles with empty new modules

---

## Phase 2: User Story 6 — Policy Hot-Reload (Priority: P2, FR-007) 🎯 MVP

**Goal**: Verify that `PolicyWatcher::register()` detects filesystem changes and updates the policy cache. Covers SCENARIOS.md S045–S052.

**Independent Test**: `cargo test --test integration policy_watcher_tests` passes with 6+ tests covering register, modification, deletion, malformed, unregister, and multi-workspace scenarios.

### Tests (TDD — write first, verify compilation, then run)

- [X] T004 [P] [US6] Write `register_loads_initial_policy` test (S045) in `tests/integration/policy_watcher_tests.rs` — Create tempdir with `.agentrc/settings.json`, call `PolicyWatcher::register()`, assert `get_policy()` returns parsed policy
- [X] T005 [P] [US6] Write `policy_file_modification_detected` test (S046) in `tests/integration/policy_watcher_tests.rs` — Modify settings.json after register, poll `get_policy()` with 50ms interval / 2s timeout until updated policy reflected
- [X] T006 [P] [US6] Write `policy_file_deletion_falls_back_to_deny_all` test (S047) in `tests/integration/policy_watcher_tests.rs` — Delete settings.json, poll until `get_policy()` returns `WorkspacePolicy::default()`
- [X] T007 [P] [US6] Write `malformed_policy_file_uses_deny_all` test (S048) in `tests/integration/policy_watcher_tests.rs` — Write invalid JSON to settings.json, verify deny-all default
- [X] T008 [P] [US6] Write `unregister_stops_watching` test (S049) in `tests/integration/policy_watcher_tests.rs` — Unregister workspace, modify file, verify policy does NOT update
- [X] T009 [P] [US6] Write `multiple_workspaces_independent_policies` test (S050) in `tests/integration/policy_watcher_tests.rs` — Register two workspaces, modify one, verify only modified workspace's policy changes

### Verification

- [X] T010 [US6] Run `cargo test --test integration policy_watcher_tests` and verify all 6 tests pass
- [X] T011 [US6] Run `cargo clippy -- -D warnings` and verify zero warnings from new test code

**Checkpoint**: Policy hot-reload tests complete. FR-007 satisfied. `cargo test --test integration policy_watcher_tests` passes.

---

## Phase 3: User Story 7 — IPC Server Command Dispatch (Priority: P3, FR-008)

**Goal**: Verify IPC named pipe server starts, accepts connections, enforces auth tokens, and dispatches all commands correctly. Covers SCENARIOS.md S053–S066.

**Independent Test**: `cargo test --test integration ipc_server_tests` passes with 8+ tests covering auth enforcement and all command types.

### Tests (TDD — write first, verify compilation, then run)

- [X] T012 [P] [US7] Write `ipc_valid_auth_token_accepted` test (S053) in `tests/integration/ipc_server_tests.rs` — Start `spawn_ipc_server()` with unique pipe name, connect client with valid token, send `list` command, assert `{ok: true}`
- [X] T013 [P] [US7] Write `ipc_invalid_auth_token_rejected` test (S054) in `tests/integration/ipc_server_tests.rs` — Connect with wrong token, assert `{ok: false, error: "unauthorized"}`
- [X] T014 [P] [US7] Write `ipc_missing_auth_token_rejected` test (S055) in `tests/integration/ipc_server_tests.rs` — Connect without token, assert unauthorized error
- [X] T015 [P] [US7] Write `ipc_list_returns_active_sessions` test (S057) in `tests/integration/ipc_server_tests.rs` — Create 2 active + 1 terminated session in DB, send `list`, assert response contains 2 session IDs
- [X] T016 [P] [US7] Write `ipc_approve_resolves_pending_approval` test (S059) in `tests/integration/ipc_server_tests.rs` — Create pending approval with oneshot sender in `pending_approvals` map, send `approve` via IPC, assert oneshot fires with Approved status
- [X] T017 [P] [US7] Write `ipc_reject_resolves_with_reason` test (S060) in `tests/integration/ipc_server_tests.rs` — Create pending approval, send `reject` with reason, assert oneshot fires with Rejected + reason
- [X] T018 [P] [US7] Write `ipc_resume_resolves_pending_wait` test (S062) in `tests/integration/ipc_server_tests.rs` — Create pending wait, send `resume`, assert oneshot resolves
- [X] T019 [P] [US7] Write `ipc_mode_changes_session_mode` test (S064) in `tests/integration/ipc_server_tests.rs` — Create active session, send `mode hybrid`, verify session mode updated in DB

### Verification

- [X] T020 [US7] Run `cargo test --test integration ipc_server_tests` and verify all 8 tests pass
- [X] T021 [US7] Run `cargo clippy -- -D warnings` and verify zero warnings from new test code

**Checkpoint**: IPC server tests complete. FR-008 satisfied. `cargo test --test integration ipc_server_tests` passes.

---

## Phase 4: User Story 1 — MCP Transport Dispatch (Priority: P1, FR-001)

**Goal**: Verify full MCP tool dispatch through the HTTP/SSE transport — JSON argument parsing, tool-router matching, handler execution, and response construction. Covers SCENARIOS.md S001–S010.

**Independent Test**: `cargo test --test integration mcp_dispatch_tests` passes with 5+ tests covering tool dispatch via HTTP transport.

**Risk**: rmcp `RequestContext` has no public constructor. Tests must go through the SSE transport layer. If the MCP handshake is too complex, fall back to transport-level smoke tests verifying endpoint accepts requests and returns valid JSON-RPC responses.

### Tests (TDD — write first, verify compilation, then run)

- [ ] T022 [P] [US1] Write `transport_heartbeat_dispatch` test (S001) in `tests/integration/mcp_dispatch_tests.rs` — Start `serve_sse()` on ephemeral port, create active session, send heartbeat tool call via MCP client/HTTP, verify `acknowledged: true` in response
- [ ] T023 [P] [US1] Write `transport_set_mode_dispatch` test (S002) in `tests/integration/mcp_dispatch_tests.rs` — Send set_operational_mode via transport, verify mode change response
- [ ] T024 [P] [US1] Write `transport_recover_state_dispatch` test (S003) in `tests/integration/mcp_dispatch_tests.rs` — Send recover_state via transport, verify clean state response
- [ ] T025 [P] [US1] Write `transport_unknown_tool_returns_error` test (S006) in `tests/integration/mcp_dispatch_tests.rs` — Send unknown tool name, verify error response
- [ ] T026 [P] [US1] Write `transport_malformed_args_returns_error` test (S007) in `tests/integration/mcp_dispatch_tests.rs` — Send tool call with invalid JSON arguments, verify descriptive error

### Verification

- [ ] T027 [US1] Run `cargo test --test integration mcp_dispatch_tests` and verify all tests pass
- [ ] T028 [US1] Run `cargo clippy -- -D warnings` and verify zero warnings from new test code

**Checkpoint**: MCP transport dispatch tests complete. FR-001 satisfied. `cargo test --test integration mcp_dispatch_tests` passes.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Final validation across all test tiers

- [ ] T029 Run full `cargo test` and verify zero failures across all tiers (unit + contract + integration)
- [ ] T030 Run `cargo clippy -- -D warnings` and verify zero warnings across entire workspace
- [ ] T031 Run `cargo fmt --all -- --check` and verify no formatting violations
- [ ] T032 Verify existing tests (23 integration modules) are unaffected — zero regressions

**Checkpoint**: All quality gates pass. FR-012 and FR-013 satisfied.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **User Story Phases (2–4)**: All depend on Phase 1 (module registration)
  - US6, US7, US1 are mutually independent — can proceed in parallel or any order
  - Recommended order: US6 → US7 → US1 (ascending risk)
- **Polish (Phase 5)**: Depends on all user story phases completing

### Within Each User Story Phase

1. Write all test functions (T004–T009, T012–T019, T022–T026) — all marked [P] within their phase
2. Run phase verification (T010–T011, T020–T021, T027–T028)
3. Phase checkpoint must pass before moving to next phase

### Parallel Opportunities

- T001, T002, T003 affect the same file (`tests/integration.rs`) — execute sequentially
- T004–T009 are all [P] — different test functions in the same file, can be written together
- T012–T019 are all [P] — different test functions in the same file, can be written together
- T022–T026 are all [P] — different test functions in the same file, can be written together
- Phase 2, 3, 4 are independent — can run in parallel on different branches

---

## Parallel Example: Phase 2 (US6)

```text
# All policy watcher tests can be written in one pass:
T004: register_loads_initial_policy
T005: policy_file_modification_detected
T006: policy_file_deletion_falls_back_to_deny_all
T007: malformed_policy_file_uses_deny_all
T008: unregister_stops_watching
T009: multiple_workspaces_independent_policies
```

---

## Implementation Strategy

### MVP First (Phase 2 Only — Policy Watcher Tests)

1. Complete Phase 1: Setup (register modules)
2. Complete Phase 2: US6 — Policy watcher tests (lowest risk, highest FR-007 gap severity)
3. **STOP and VALIDATE**: `cargo test --test integration policy_watcher_tests` passes
4. All quality gates pass

### Incremental Delivery

1. Phase 1 → Module registration → `cargo check` passes
2. Phase 2 → Policy watcher tests → FR-007 covered → Checkpoint
3. Phase 3 → IPC server tests → FR-008 covered → Checkpoint
4. Phase 4 → MCP dispatch tests → FR-001 covered → Checkpoint
5. Phase 5 → Full validation → All FRs satisfied

### Task Summary

| Phase | Story | Tasks | New Tests |
|---|---|---|---|
| Phase 1: Setup | — | T001–T003 (3) | 0 |
| Phase 2: US6 | Policy Hot-Reload | T004–T011 (8) | 6 |
| Phase 3: US7 | IPC Server | T012–T021 (10) | 8 |
| Phase 4: US1 | MCP Dispatch | T022–T028 (7) | 5 |
| Phase 5: Polish | — | T029–T032 (4) | 0 |
| **Total** | | **32 tasks** | **19 tests** |

---

## Notes

- [P] tasks = can be written in parallel (same file, different functions, no data dependencies)
- All 19 new tests target the 3 coverage gaps identified in plan.md
- Existing 150+ tests remain untouched — zero regressions required
- Each phase produces a self-contained, independently runnable test module
- Scenario IDs (S001–S066) reference SCENARIOS.md behavioral matrix
