---
id: TASK-002.04
title: "002 - User Story 1 — Database Operations Behave Identically (Priority: P1) MVP"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-002
dependencies: []
ordinal: 2040
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: All 5 repository modules rewritten with sqlx queries. Every existing CRUD operation produces identical results.

**Independent Test**: Run the full existing test suite (unit, contract, integration) against the new SQLite persistence layer and confirm 100% pass rate.

### Tests (write first, confirm they fail)

- [X] T018 [P] [US1] Write unit tests for SessionRepo CRUD operations in tests/unit/session_repo_tests.rs: create, get_by_id, update_status, list_active, set_terminated, count_active
- [X] T019 [P] [US1] Write unit tests for ApprovalRepo CRUD operations in new tests/unit/approval_repo_tests.rs: create, get_by_id, get_pending_for_session, update_status, mark_consumed
- [X] T020 [P] [US1] Write unit tests for CheckpointRepo CRUD operations in tests/unit/checkpoint_tests.rs: create, get_by_id, list_for_session, delete_for_session
- [X] T021 [P] [US1] Write unit tests for PromptRepo CRUD operations in new tests/unit/prompt_repo_tests.rs: create, get_by_id, get_pending_for_session, update_decision
- [X] T022 [P] [US1] Write unit tests for StallAlertRepo CRUD operations in new tests/unit/stall_repo_tests.rs: create, get_active_for_session, update_status, increment_nudge_count, dismiss

### Implementation

- [X] T023 [US1] Rewrite src/persistence/session_repo.rs: replace all SurrealDB SDK calls with sqlx queries per contracts/repository-api.md SessionRepo section (12 methods). Include FR-019 enum validation for `status` and `mode` in create/update methods
- [X] T024 [US1] Rewrite src/persistence/approval_repo.rs: replace all SurrealDB SDK calls with sqlx queries per contracts/repository-api.md ApprovalRepo section (6 methods). Include FR-019 enum validation for `risk_level` and `status` in create/update methods
- [X] T025 [US1] Rewrite src/persistence/checkpoint_repo.rs: replace all SurrealDB SDK calls with sqlx queries per contracts/repository-api.md CheckpointRepo section (4 methods)
- [X] T026 [US1] Rewrite src/persistence/prompt_repo.rs: replace all SurrealDB SDK calls with sqlx queries per contracts/repository-api.md PromptRepo section (5 methods). Include FR-019 enum validation for `prompt_type` in create method
- [X] T027 [US1] Rewrite src/persistence/stall_repo.rs: replace all SurrealDB SDK calls with sqlx queries per contracts/repository-api.md StallAlertRepo section (5 methods). Include FR-019 enum validation for `status` in create/update methods
- [X] T028 [US1] Update any Slack command/handler files in src/slack/ that directly reference SurrealDB types to use SqlitePool
- [X] T029 [US1] Update all MCP tool handlers in src/mcp/tools/ that reference the old Database type

### Test Migration

- [X] T030 [US1] Migrate contract tests to use `connect_memory()` + `bootstrap_schema()` in tests/contract/schema_tests.rs (depends on T004; shares file — not parallel)
- [X] T031 [P] [US1] Migrate contract tests in tests/contract/accept_diff_tests.rs: replace SurrealDB setup with SQLite in-memory
- [X] T032 [P] [US1] Migrate contract tests in tests/contract/ask_approval_tests.rs: replace SurrealDB setup with SQLite in-memory
- [X] T033 [P] [US1] Migrate contract tests in tests/contract/check_auto_approve_tests.rs: replace SurrealDB setup with SQLite in-memory
- [X] T034 [P] [US1] Migrate contract tests in tests/contract/forward_prompt_tests.rs: replace SurrealDB setup with SQLite in-memory
- [X] T035 [P] [US1] Migrate contract tests in tests/contract/heartbeat_tests.rs, mode_tests.rs, recover_state_tests.rs, remote_log_tests.rs, resource_tests.rs: replace SurrealDB setup with SQLite in-memory
- [X] T036 [P] [US1] Migrate integration tests in tests/integration/session_lifecycle_tests.rs: replace SurrealDB setup with SQLite in-memory
- [X] T037 [P] [US1] Migrate integration tests in tests/integration/approval_flow_tests.rs: replace SurrealDB setup with SQLite in-memory
- [X] T038 [P] [US1] Migrate integration tests in tests/integration/crash_recovery_tests.rs, nudge_flow_tests.rs, prompt_flow_tests.rs: replace SurrealDB setup with SQLite in-memory
- [X] T039 [P] [US1] Migrate integration tests in tests/integration/diff_apply_tests.rs, channel_override_tests.rs: replace SurrealDB setup with SQLite in-memory
- [X] T040 [P] [US1] Migrate unit tests in tests/unit/config_tests.rs, credential_loading_tests.rs: update any DB-related config references
- [X] T041 [US1] Update test harness files (tests/unit.rs, tests/contract.rs, tests/integration.rs) to add any new test modules (approval_repo_tests, prompt_repo_tests, stall_repo_tests, retention_tests)

**Checkpoint**: `cargo test` passes with all unit, contract, and integration tests green. All 31+ repository methods work identically. US1 is complete.

---

<!-- SECTION:DESCRIPTION:END -->
