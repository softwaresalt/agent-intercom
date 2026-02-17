# Session Memory: 002-sqlite-migration Phase 2

**Date**: 2026-02-16
**Spec**: specs/002-sqlite-migration/
**Phase**: 2 — Foundational — Connection, Schema, Error Handling, Models (US2 + US3)
**Mode**: Full (phase 2 of 6)

## Task Overview

Phase 2 delivers the core persistence infrastructure that all repository work depends on. It implements User Stories 2 (auto-bootstrap on startup) and 3 (in-memory database for tests). Fourteen tasks covering tests (T004–T005), error handling (T006), type alias (T007), connection/schema (T008–T009), model updates (T010–T014), main.rs (T015), and MCP type updates (T016–T017).

## Current State

### Completed Tasks

All 14 Phase 2 tasks complete: T004–T017

### Files Modified

- `src/errors.rs` — `From<sqlx::Error>` replaces `From<surrealdb::Error>`
- `src/persistence/mod.rs` — `Database = SqlitePool`, `pub use sqlx::SqlitePool` re-export
- `src/persistence/db.rs` — full rewrite: `connect(path)` with WAL + single-writer pool, `connect_memory()` with min_connections=1
- `src/persistence/schema.rs` — full rewrite: SQLite DDL for 5 tables via `sqlx::raw_sql()`
- `src/persistence/session_repo.rs` — stubbed all 12 methods with `todo!()`
- `src/persistence/approval_repo.rs` — stubbed all 6 methods with `todo!()`
- `src/persistence/checkpoint_repo.rs` — stubbed all 4 methods with `todo!()`
- `src/persistence/prompt_repo.rs` — stubbed all 5 methods with `todo!()`
- `src/persistence/stall_repo.rs` — stubbed all 6 methods with `todo!()`, added new `get_by_id` method
- `src/persistence/retention.rs` — `purge()` stubbed with `todo!()`
- `src/models/mod.rs` — removed `deserialize_surreal_id` helper and `surrealdb::sql::Thing` references
- `src/models/session.rs` — `nudge_count` u32→i64, removed SurrealDB serde attrs
- `src/models/approval.rs` — removed SurrealDB ID serde attributes
- `src/models/prompt.rs` — `elapsed_seconds` Option<u64>→Option<i64>, `actions_taken` Option<u32>→Option<i64>
- `src/models/stall.rs` — `idle_seconds` u64→i64, `nudge_count` u32→i64
- `src/models/policy.rs` — doc comment SurrealDB→`SQLite`
- `src/main.rs` — updated `connect()` call to use config path
- `src/mcp/handler.rs` — `AppState.db` to `Arc<SqlitePool>`
- `src/mcp/context.rs` — `ToolContext.db` to `Arc<SqlitePool>`
- `src/slack/commands.rs` — all 7 functions updated from `Arc<surrealdb::Surreal<Db>>` to `Arc<Database>`
- `src/slack/handlers/nudge.rs` — replaced raw SurrealDB `.select()` with `stall_repo.get_by_id()`
- `tests/contract/schema_tests.rs` — full rewrite: 3 SQLite schema tests
- `tests/unit/session_repo_tests.rs` — added `in_memory_connect_creates_five_tables`, updated existing test to use `connect_memory()`
- `tests/integration/*.rs` (7 files) — all 33 `db::connect(&config, true)` → `db::connect_memory()`
- `tests/integration/crash_recovery_tests.rs` — 3 raw `database.query()` calls replaced with repo methods
- `specs/002-sqlite-migration/tasks.md` — T004–T017 marked complete

### Quality Gate Results

- **cargo check**: PASS (lib and tests)
- **cargo fmt --all -- --check**: PASS
- **cargo clippy --all-targets -- -D warnings -D clippy::pedantic**: PASS
- **cargo test**: 138 contract tests PASS, 17 unit tests PASS, 32 integration tests FAIL (expected — `todo!()` panics)

The 32 integration test failures are expected: all repo method bodies are `todo!()` stubs that panic when called. Phase 3 replaces these stubs with real sqlx implementations, which will restore integration test passes.

## Important Discoveries

### Stubbing Strategy

All 5 repo modules + retention.rs were stubbed with `todo!("rewrite with sqlx in Phase N (TXXX)")`. Method signatures, doc comments, and struct definitions were preserved. This allows `cargo check` and `cargo clippy` to pass while deferring implementation to Phase 3/4.

### New Method: `StallAlertRepo::get_by_id`

`src/slack/handlers/nudge.rs` had a raw SurrealDB `.select()` call that needed a direct ID lookup on stall alerts. The existing repo API only had `get_active_for_session(session_id)`. A new `get_by_id(alert_id: &str) -> Result<Option<StallAlert>>` method was added to `StallAlertRepo`. This is not in the original tasks.md — Phase 3 T027 must implement it along with the other 5 stall repo methods.

### clippy::unused_async Workaround

All `todo!()` stub methods are `async` but contain no `.await` points, triggering `clippy::unused_async`. Added `#[allow(clippy::unused_async)]` to all 5 repo impl blocks and the `purge()` function. These `#[allow]` attributes should be removed in Phase 3/4 when real sqlx queries (which use `.await`) replace the stubs.

### clippy::doc_markdown for `SQLite`

Clippy pedantic requires backtick-quoting technical terms in doc comments. All 24 instances of bare "SQLite" across 13 source files were wrapped as `` `SQLite` ``. Future doc comments must follow this pattern.

### Integration Test `config` Variables

After replacing `db::connect(&config, true)` with `db::connect_memory()`, the `config` variable is unused in ~33 test functions. These produce warnings but not errors. Phase 3 test migration (T030–T041) should either remove the unused `config` bindings or prefix them with `_config`.

### Schema Column Order

The session table DDL in `schema.rs` places `terminated_at` before `last_tool`. The initial T004 test had them reversed, causing an assertion failure. The test was corrected to match the DDL order: `..., updated_at, terminated_at, last_tool, nudge_count, ...`.

## Next Steps

### Phase 3 (US1 — Database Operations Behave Identically)

This is the largest phase (24 tasks). Key work:

1. **Write repo unit tests first** (T018–T022): 5 new test files for session, approval, checkpoint, prompt, and stall repos
2. **Implement repo methods** (T023–T027): Replace `todo!()` stubs with sqlx queries using `sqlx::query()` / `sqlx::query_as()`. Each repo needs intermediate `*Row` structs with `#[derive(sqlx::FromRow)]` for deserialization
3. **Remember to implement `StallAlertRepo::get_by_id`** in T027 (not in original task list)
4. **Remove `#[allow(clippy::unused_async)]`** from all repo impl blocks once real `.await` calls are present
5. **Migrate tests** (T028–T041): Update remaining test files, add new test modules to harness files
6. **Handle unused `config` warnings**: Remove or prefix unused `config` variables in integration tests

### Implementation Notes for Phase 3

- `sqlx::query_as::<_, SessionRow>()` pattern for reads
- `sqlx::query()` with `.bind()` for writes
- Enum columns (status, mode, risk_level, prompt_type) stored as TEXT with CHECK constraints — parse with `.parse::<EnumType>()` in Rust
- Boolean `stall_paused` stored as INTEGER 0/1 — convert with `row.stall_paused != 0`
- All timestamps as `TEXT` in ISO 8601 format via chrono
- JSON fields (`file_hashes`, `progress_snapshot`, `instruction`) stored as TEXT, parsed with `serde_json`

## Context to Preserve

- **Spec files**: `specs/002-sqlite-migration/spec.md`, `plan.md`, `contracts/repository-api.md`, `contracts/schema.sql.md`
- **Agent references**: `.github/agents/rust-engineer.agent.md` for coding standards
- **Constitution**: `.specify/memory/constitution.md` for principle checks
- **Phase 1 memory**: `.copilot-tracking/memory/2026-02-16/002-sqlite-migration-phase-1-memory.md`
