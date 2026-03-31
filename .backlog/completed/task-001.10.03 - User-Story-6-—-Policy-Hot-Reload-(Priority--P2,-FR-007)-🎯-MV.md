---
id: TASK-001.10.03
title: "001-002 - User Story 6 — Policy Hot-Reload (Priority: P2, FR-007) 🎯 MVP"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001.10
dependencies: []
ordinal: 1130
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

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

<!-- SECTION:DESCRIPTION:END -->
