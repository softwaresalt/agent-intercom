# Session Memory: 002-sqlite-migration Phase 1

**Date**: 2026-02-16
**Spec**: specs/002-sqlite-migration/
**Phase**: 1 — Setup
**Mode**: Full (phase 1 of 6)

## Task Overview

Phase 1 swaps the SurrealDB dependency for sqlx and updates configuration. Three tasks:

- **T001**: Replaced `surrealdb = "1.5"` with `sqlx = "0.8"` in both `[workspace.dependencies]` and `[dependencies]` sections of Cargo.toml
- **T002**: Added `[database]` section to config.toml with `path = "data/monocoque.db"`
- **T003**: Created `DatabaseConfig` struct in src/config.rs with a `path: PathBuf` field, added it to `GlobalConfig`, and updated `db_path()` to return `&self.database.path` instead of a hardcoded `.monocoque/db` path

## Current State

### Completed Tasks

- T001, T002, T003 — all 3 Phase 1 tasks complete

### Files Modified

- `Cargo.toml` — dependency swap (surrealdb → sqlx)
- `config.toml` — added `[database]` section
- `src/config.rs` — added `DatabaseConfig` struct, updated `GlobalConfig` and `db_path()`
- `specs/002-sqlite-migration/tasks.md` — T001–T003 marked complete

### Compilation Status

- **cargo fmt**: PASS
- **cargo check**: FAILS (26 errors) — all from SurrealDB references in files outside Phase 1 scope
- **cargo clippy**: blocked by compilation failure
- **cargo test**: blocked by compilation failure

This is expected. Phase 1 intentionally removes the SurrealDB dependency. Phase 2 replaces all SurrealDB type references with sqlx equivalents.

### Error Sources (Phase 2 will fix)

- `src/errors.rs`: `From<surrealdb::Error>` impl (T006)
- `src/persistence/db.rs`: `Surreal<Db>`, `Mem`, `RocksDb` imports (T007, T008)
- `src/persistence/mod.rs`: `Database` type alias (T007)
- `src/models/mod.rs`: `deserialize_surreal_id` helper (T010)
- `src/mcp/handler.rs`: `AppState.db` type (T016)
- `src/mcp/context.rs`: `ToolContext.db` type (T017)
- `src/slack/commands.rs`: 7 functions with `Arc<surrealdb::Surreal<Db>>` params
- `src/slack/handlers/nudge.rs`: `surrealdb::Surreal<Db>` reference

## Important Discoveries

- config.toml had NO existing `[database]` section — SurrealDB config was hardcoded in db.rs (namespace `monocoque`, database `agent_rc`)
- config.rs had no `DatabaseConfig` struct — `db_path()` was derived from `default_workspace_root`
- The `DatabaseConfig` struct uses `#[serde(default)]` so existing config files without the `[database]` section will still parse using the default path

## Next Steps

Phase 2 (Foundational — US2 + US3) will:
1. Write tests for `connect()` and `connect_memory()` (T004, T005)
2. Replace `From<surrealdb::Error>` with `From<sqlx::Error>` (T006)
3. Update the `Database` type alias to `SqlitePool` (T007)
4. Rewrite db.rs with SqlitePool connection logic (T008)
5. Rewrite schema.rs with SQLite DDL (T009)
6. Remove `deserialize_surreal_id` helper (T010)
7. Update all model field types (T011–T014)
8. Update main.rs connect call (T015)
9. Update MCP handler/context types (T016, T017)

## Context to Preserve

- `sqlx` features: `runtime-tokio`, `sqlite`, `json`, `chrono`, `macros`
- Default db path: `data/monocoque.db` (configurable via `[database].path`)
- `DatabaseConfig` defaults via `#[serde(default)]` on the field in `GlobalConfig`
- `db_path()` now returns `&Path` (was `PathBuf`) — callers in db.rs will need updating
