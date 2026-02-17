# Implementation Plan: SQLite Migration

**Branch**: `002-sqlite-migration` | **Date**: 2026-02-16 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/002-sqlite-migration/spec.md`

## Summary

Replace SurrealDB 1.5 (kv-rocksdb embedded, ~100 transitive dependencies) with SQLite via sqlx 0.8 (bundled libsqlite3, ~20 transitive dependencies). The migration is a 1:1 persistence layer swap: five SCHEMAFULL SurrealDB tables become five `CREATE TABLE IF NOT EXISTS` SQLite tables, five repository modules rewrite internal queries from SurrealDB SDK calls to sqlx parameterized SQL, and the `Database` type alias changes from `Surreal<Db>` to `SqlitePool`. All public repository method signatures are preserved. No schema migration system — idempotent DDL on every startup.

## Technical Context

**Language/Version**: Rust stable, edition 2021
**Primary Dependencies**: sqlx 0.8 (runtime-tokio, sqlite, json, chrono, macros), axum 0.8, rmcp 0.5, slack-morphism 2.17, tokio 1.37
**Storage**: SQLite (file-backed via sqlx bundled libsqlite3; WAL journal mode; single-writer pool max_connections=1)
**Testing**: `cargo test` — unit, contract, integration tiers in `tests/` directory; SQLite in-memory backend (`sqlite::memory:` with min_connections=1)
**Target Platform**: Windows workstations (primary), Linux servers (secondary)
**Project Type**: Single workspace, two binaries (`monocoque-agent-rc`, `monocoque-ctl`)
**Performance Goals**: N/A — persistence layer is not a bottleneck; single-writer pool sufficient for sequential MCP tool calls
**Constraints**: Zero SurrealDB references remaining after migration; all quality gates pass (check, clippy, fmt, test)
**Scale/Scope**: 8 persistence modules rewritten, 5 model modules updated, 2 MCP modules updated (type changes), ~30 repository methods migrated, ~25 test modules updated

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Safety-First Rust | PASS | No change — `#![forbid(unsafe_code)]`, clippy pedantic, no unwrap/expect |
| II. MCP Protocol Fidelity | PASS | No change — tool surface unchanged, only internal persistence calls change |
| III. Test-First Development | PASS | TDD required — write failing test, then implement. All three test tiers updated |
| IV. Security Boundary Enforcement | PASS | No change — path safety, keychain credentials unaffected |
| V. Structured Observability | PASS | No change — tracing spans unaffected (persistence calls already traced at call sites) |
| VI. Single-Binary Simplicity | **VIOLATION** | Constitution states "SurrealDB in embedded mode is the sole persistence layer; do not introduce additional databases." This migration replaces SurrealDB with SQLite. See Complexity Tracking |

### Post-Design Re-evaluation (Phase 1 complete)

All design artifacts confirm this is a strict 1:1 replacement:
- Same 5 tables, same column semantics, same repository method signatures
- Same test backend pattern (in-memory for tests, file-backed for production)
- Net dependency reduction (~80 fewer transitive crates)
- No architectural changes beyond the persistence layer internals

Constitution Principle VI violation remains justified. Post-merge amendment required to update the constitution text from "SurrealDB in embedded mode" to "SQLite via sqlx".

## Project Structure

### Documentation (this feature)

```text
specs/002-sqlite-migration/
├── plan.md              # This file
├── research.md          # Phase 0 output (sqlx patterns, SQLite config, migration strategy)
├── data-model.md        # Phase 1 output (entity definitions, SQLite schema, type changes)
├── quickstart.md        # Phase 1 output (build/test/run after migration)
├── contracts/           # Phase 1 output
│   ├── schema.sql.md    # Complete DDL for all 5 tables
│   └── repository-api.md # Method signatures and behavioral contracts
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
src/
├── config.rs            # Update: db config section (path instead of ns/db/engine)
├── errors.rs            # Update: From<sqlx::Error> replaces From<surrealdb::Error>
├── lib.rs               # Update: re-export changes if any
├── main.rs              # Update: connect() call signature
├── models/
│   ├── mod.rs           # Update: remove deserialize_surreal_id, add FromRow derives
│   ├── approval.rs      # Update: field types (u32→i64, u64→i64)
│   ├── checkpoint.rs    # No change expected
│   ├── progress.rs      # No change expected
│   ├── prompt.rs        # Update: field types (u64→i64, u32→i64)
│   ├── session.rs       # Update: field types, remove surrealdb serde attrs
│   └── stall.rs         # Update: field types (u64→i64, u32→i64)
├── persistence/
│   ├── mod.rs           # Update: Database type alias, re-exports
│   ├── db.rs            # Rewrite: SqlitePool connect/connect_memory
│   ├── schema.rs        # Rewrite: SQLite DDL via raw_sql
│   ├── session_repo.rs  # Rewrite: sqlx queries
│   ├── approval_repo.rs # Rewrite: sqlx queries
│   ├── checkpoint_repo.rs # Rewrite: sqlx queries
│   ├── prompt_repo.rs   # Rewrite: sqlx queries
│   ├── stall_repo.rs    # Rewrite: sqlx queries
│   └── retention.rs     # Rewrite: sqlx purge queries
├── mcp/
│   ├── handler.rs       # Update: AppState.db type Arc<SqlitePool>
│   └── context.rs       # Update: ToolContext.db type Arc<SqlitePool>
└── slack/               # Update: any direct db type references

tests/
├── contract/            # Update: connect_memory() + bootstrap_schema()
├── integration/         # Update: connect_memory() + bootstrap_schema()
└── unit/                # Update: connect_memory() + bootstrap_schema()

Cargo.toml               # Update: remove surrealdb, add sqlx
```

**Structure Decision**: Single project structure. All changes are within the existing directory layout. No new modules or directories created (except `contracts/` in the spec directory). The `persistence/` module is the primary change surface; all other modules receive type-signature updates only.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Principle VI: Replacing SurrealDB with SQLite | SurrealDB is massive overkill (~100 transitive deps, 15+ MB binary bloat) for what this MCP server needs: five simple tables with CRUD operations. SQLite via sqlx provides identical functionality with ~80 fewer dependencies. | Keeping SurrealDB: rejected because it violates the spirit of Principle VI ("do not add libraries speculatively") — SurrealDB's graph database, record links, SurrealQL, and multi-model capabilities are entirely unused. SQLite is the minimal viable persistence layer. |
