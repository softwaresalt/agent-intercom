---
id: TASK-005.07
title: "005 - User Story 4 — Workspace-to-Channel Mapping (Priority: P2)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5070
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Centralized workspace-to-channel mapping in `config.toml` replaces per-workspace `channel_id` query params

**Independent Test**: Configure workspace mapping, connect with `workspace_id`, verify messages route to correct channel

### Tests (S027–S035)

- [x] T042 [P] [US4] Write unit test for workspace mapping config parsing in `tests/unit/workspace_mapping_tests.rs` — covers S027, S032, S033
- [x] T043 [P] [US4] Write unit test for workspace_id resolution (known, unknown, both params) in `tests/unit/workspace_mapping_tests.rs` — covers S027, S028, S029, S030, S031
- [x] T044 [P] [US4] Write integration test for hot-reload of workspace mappings in `tests/integration/workspace_routing_tests.rs` — covers S034, S035

### Implementation

- [x] T045 [US4] Add `WorkspaceMapping` struct and `[[workspace]]` TOML parsing to `src/config.rs`
- [x] T046 [US4] Add `workspace_mappings: HashMap<String, String>` field to `GlobalConfig` in `src/config.rs`
- [x] T047 [US4] Add workspace_id validation rules (non-empty, valid characters, no duplicates) to `src/config.rs`
- [x] T048 [US4] Parse `workspace_id` query parameter in SSE middleware in `src/mcp/sse.rs`
- [x] T049 [US4] Implement resolution logic in `src/mcp/sse.rs` — workspace_id lookup → channel_id, with channel_id fallback and deprecation warning
- [x] T050 [US4] Extend `PolicyWatcher` (or create new watcher) to hot-reload workspace mappings from `config.toml` via `notify` in `src/policy/watcher.rs`
- [x] T051 [US4] Update `config.toml.example` with `[[workspace]]` example entries

**Checkpoint**: Workspace mappings resolve correctly; legacy channel_id still works; hot-reload functional

---

<!-- SECTION:DESCRIPTION:END -->
