---
id: TASK-001.10.04
title: "001-002 - User Story 7 — IPC Server Command Dispatch (Priority: P3, FR-008)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001.10
dependencies: []
ordinal: 1140
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

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

<!-- SECTION:DESCRIPTION:END -->
