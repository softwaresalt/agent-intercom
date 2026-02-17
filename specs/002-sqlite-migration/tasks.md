# Tasks: SQLite Migration

**Input**: Design documents from `/specs/002-sqlite-migration/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Tests are included — the spec requires TDD (Constitution Principle III) and all existing tests must be migrated.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story. US2 and US3 are folded into the Foundational phase because they produce the infrastructure that US1 and US4 depend on.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Phase 1: Setup

**Purpose**: Swap dependencies and update configuration

- [X] T001 Replace `surrealdb` with `sqlx` in Cargo.toml workspace dependencies: remove `surrealdb = "1.5"`, add `sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "json", "chrono", "macros"] }`
- [X] T002 Update `[database]` section in config.toml: replace SurrealDB engine/namespace/database fields with `path = "data/monocoque.db"`
- [X] T003 Update `DatabaseConfig` struct and `db_path()` method in src/config.rs to parse the new `path` field instead of SurrealDB-specific fields

---

## Phase 2: Foundational — Connection, Schema, Error Handling, Models (US2 + US3)

**Purpose**: Core persistence infrastructure that MUST be complete before any repository work can begin. This phase delivers User Stories 2 (auto-bootstrap) and 3 (in-memory tests).

**CRITICAL**: No user story implementation can begin until this phase is complete.

### Tests (write first, confirm they fail)

- [X] T004 [US2] Write test for file-backed `connect()` and `bootstrap_schema()` in tests/contract/schema_tests.rs — verify all 5 tables are created with correct columns
- [X] T005 [US3] Write test for `connect_memory()` and `bootstrap_schema()` in tests/unit/session_repo_tests.rs — verify in-memory pool works and tables exist

### Implementation

- [X] T006 [P] [US3] Replace `From<surrealdb::Error>` with `From<sqlx::Error>` in src/errors.rs
- [X] T007 [P] [US2] Update `Database` type alias from `Surreal<Db>` to `SqlitePool` and update re-exports in src/persistence/mod.rs
- [X] T008 [US2] Rewrite src/persistence/db.rs: implement `connect(path)` (file-backed, WAL, max_connections=1, create_if_missing, auto-create parent dirs) and `connect_memory()` (sqlite::memory:, min_connections=1). Design satisfies EC-003 (concurrent access via single-writer pool + WAL) and EC-005 (disk full/permissions → AppError::Db). EC-001 (locked file) and EC-004 (corrupt DB) produce AppError::Db on connect or first query — no auto-repair
- [X] T009 [US2] Rewrite src/persistence/schema.rs: replace SurrealQL DEFINE TABLE/FIELD with SQLite DDL via `sqlx::raw_sql()` per contracts/schema.sql.md
- [X] T010 [P] [US2] Remove `deserialize_surreal_id` helper and all `surrealdb::sql::Thing` references from src/models/mod.rs
- [X] T011 [P] [US2] Update Session model field types in src/models/session.rs: `nudge_count` u32→i64, `stall_paused` serde attrs, remove SurrealDB ID serde attributes
- [X] T012 [P] [US2] Update ApprovalRequest model field types in src/models/approval.rs: remove SurrealDB ID serde attributes
- [X] T013 [P] [US2] Update ContinuationPrompt model field types in src/models/prompt.rs: `elapsed_seconds` Option<u64>→Option<i64>, `actions_taken` Option<u32>→Option<i64>
- [X] T014 [P] [US2] Update StallAlert model field types in src/models/stall.rs: `idle_seconds` u64→i64, `nudge_count` u32→i64
- [X] T015 [US2] Update src/main.rs: change `connect()` call to use new signature with config path
- [X] T016 [P] [US2] Update `AppState.db` type from `Arc<Surreal<Db>>` to `Arc<SqlitePool>` in src/mcp/handler.rs
- [X] T017 [P] [US2] Update `ToolContext.db` type from `Arc<Surreal<Db>>` to `Arc<SqlitePool>` in src/mcp/context.rs

**Checkpoint**: `cargo check` passes. Schema bootstrap creates all 5 tables. In-memory connect works. T004 and T005 pass. US2 and US3 are complete.

---

## Phase 3: User Story 1 — Database Operations Behave Identically (Priority: P1) MVP

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

## Phase 4: User Story 4 — Data Retention Purge (Priority: P2)

**Goal**: Background retention task deletes expired sessions and all child records using SQLite SQL.

**Independent Test**: Create sessions with old termination timestamps, run purge, confirm cascading deletion in correct order.

### Tests (write first, confirm they fail)

- [x] T042 [US4] Write retention purge test in tests/integration/retention_tests.rs: create expired sessions with children, run `purge_expired()`, verify cascading deletion order (stall_alerts → checkpoints → prompts → approvals → sessions)

### Implementation

- [x] T043 [US4] Rewrite src/persistence/retention.rs: replace SurrealQL DELETE with SQLite `DELETE FROM ... WHERE session_id IN (SELECT id FROM session WHERE terminated_at < ? AND terminated_at IS NOT NULL)` — cascade order: stall_alert, checkpoint, continuation_prompt, approval_request, session

**Checkpoint**: Retention purge test passes. Expired sessions and all children deleted. Active/recent sessions untouched. US4 is complete.

---

## Phase 5: User Story 5 — SurrealDB Removal & Binary Reduction (Priority: P3)

**Goal**: Confirm SurrealDB is fully removed. Verify binary size and build time improvements.

**Independent Test**: Grep codebase for `surrealdb` — zero matches. Compare release binary size.

- [ ] T044 [US5] Remove any remaining `surrealdb` references from Cargo.toml (verify workspace deps, package deps, features sections are clean)
- [ ] T045 [US5] Run `cargo build --release` and record both binary size and wall-clock build time for comparison with pre-migration baseline (validates SC-004 and SC-005)
- [ ] T046 [US5] Search entire codebase for residual `surrealdb` references: `grep -r "surrealdb" src/ tests/ Cargo.toml ctl/` — must return zero results

**Checkpoint**: Zero SurrealDB references. Binary smaller than pre-migration. US5 is complete.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Quality gates, documentation, constitution amendment

- [ ] T047 Run `cargo check` — zero errors
- [ ] T048 Run `cargo clippy -- -D warnings` — zero warnings
- [ ] T049 Run `cargo fmt --all -- --check` — no violations
- [ ] T050 Run `cargo test` — all tests green (full suite)
- [ ] T051 [P] Run quickstart.md validation: delete DB file, start server, verify auto-bootstrap
- [ ] T052 [P] Update .specify/memory/constitution.md: amend Principle VI text from "SurrealDB in embedded mode" to "SQLite via sqlx" with version bump and sync impact report
- [ ] T053 [P] Update Technical Constraints section in .specify/memory/constitution.md: change "Persistence: SurrealDB embedded" to "Persistence: SQLite via sqlx (bundled)"

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user stories
- **Phase 3 (US1)**: Depends on Phase 2 — primary implementation phase
- **Phase 4 (US4)**: Depends on Phase 2 — can run in parallel with Phase 3
- **Phase 5 (US5)**: Depends on Phase 3 + Phase 4 — verification after all code changes
- **Phase 6 (Polish)**: Depends on Phase 5 — final validation

### User Story Dependencies

- **US2 + US3 (Foundation)**: No dependencies on other stories — delivers connect, schema, in-memory
- **US1 (P1)**: Depends on US2 + US3 (needs working connection + schema to rewrite repos)
- **US4 (P2)**: Depends on US2 + US3 (needs working connection + schema for retention queries). Independent of US1 — can be done in parallel if repo modules are available
- **US5 (P3)**: Depends on US1 + US4 (verification that all code changes are complete)

### Within Each Phase

- Tests MUST be written and observed to FAIL before implementation
- Foundation (db.rs, schema.rs) before repository modules
- Repository modules before test migration
- All quality gates (check, clippy, fmt, test) must pass at each checkpoint

### Parallel Opportunities

- T006, T007 (error handling + type alias) can run in parallel
- T010–T014 (model updates) can run in parallel with each other
- T016, T017 (MCP type updates) can run in parallel
- T018–T022 (repo unit tests) can run in parallel
- T031–T041 (test migration) can run in parallel with each other (T030 excluded — shares file with T004)
- T042 and T043 (retention) can run in parallel with Phase 3 test migration

---

## Parallel Example: Phase 3 (US1)

```
# Launch all repo test stubs together:
T018: SessionRepo unit tests
T019: ApprovalRepo unit tests
T020: CheckpointRepo unit tests
T021: PromptRepo unit tests
T022: StallAlertRepo unit tests

# Then implement repos (sequential — same module patterns, shared learning):
T023: session_repo.rs
T024: approval_repo.rs
T025: checkpoint_repo.rs
T026: prompt_repo.rs
T027: stall_repo.rs

# Then migrate all test files in parallel:
T030–T041: Each test file is independent
```

---

## Implementation Strategy

### MVP First (Setup + Foundation + US1)

1. Complete Phase 1: Setup (Cargo.toml, config)
2. Complete Phase 2: Foundation (db.rs, schema.rs, models, errors) → US2 + US3 done
3. Complete Phase 3: US1 (all 5 repos + test migration)
4. **STOP and VALIDATE**: `cargo test` — full suite green
5. This delivers a fully functional SQLite backend

### Incremental Delivery

1. Setup + Foundation → Connection and schema work, in-memory tests work (US2 + US3)
2. Add US1 → All CRUD operations work, all tests pass (MVP!)
3. Add US4 → Retention purge works with SQLite
4. Add US5 → SurrealDB fully removed, binary size confirmed
5. Polish → Quality gates, constitution amendment

### Task Count Summary

| Phase | Tasks | Parallel |
|---|---|---|
| Phase 1: Setup | 3 | 0 |
| Phase 2: Foundational (US2 + US3) | 14 | 9 |
| Phase 3: US1 | 24 | 15 |
| Phase 4: US4 | 2 | 0 |
| Phase 5: US5 | 3 | 0 |
| Phase 6: Polish | 7 | 3 |
| **Total** | **53** | **27** |
