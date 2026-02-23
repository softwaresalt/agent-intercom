---
title: SQLite Migration Quickstart
description: Minimal steps to build, test, and run the project after the SQLite migration
ms.date: 2026-02-16
---

# Quickstart: SQLite Migration

## Prerequisites

- Rust stable (edition 2021)
- No additional system libraries required — sqlx bundles libsqlite3 via `bundled` feature

## Build

```powershell
cargo build
```

The build downloads and compiles the bundled SQLite C library on first run. Subsequent builds use cached artifacts.

## Test

```powershell
cargo test
```

All tests use in-memory SQLite (`sqlite::memory:`). No disk cleanup needed.

## Run

```powershell
cargo run
```

The database file is created automatically at the path specified by `db.path` in `config.toml` (default: `data/agent-rc.db`). Parent directories are auto-created.

## Configuration

### config.toml changes

```toml
[database]
path = "data/agent-rc.db"          # SQLite file path (relative to working dir)
retention_days = 30                 # Same retention policy as before
```

The `[database]` section replaces the previous `[database]` section that configured SurrealDB namespace/database/engine.

### Environment override

```text
MONOCOQUE_DB_PATH=data/agent-rc.db     # Override config.toml db path
```

## Key differences from SurrealDB

| Concern | Before (SurrealDB) | After (SQLite) |
|---|---|---|
| Engine | kv-rocksdb embedded | SQLite bundled via sqlx |
| Test backend | kv-mem in-process | sqlite::memory: |
| Connection | `Surreal::new::<RocksDb/Mem>(path)` | `SqlitePool::connect(url)` |
| Schema | SurrealQL DEFINE TABLE/FIELD | SQL CREATE TABLE IF NOT EXISTS |
| Queries | `db.select().query()` | `sqlx::query_as!()` / `sqlx::query!()` |
| Binary size | ~15 MB larger | ~3 MB larger |
| Dependencies | surrealdb (100+ transitive) | sqlx + bundled sqlite3 (~20 transitive) |

## Verification after migration

1. `cargo check` — compiles without errors
2. `cargo clippy -- -D warnings` — zero warnings
3. `cargo fmt --all -- --check` — formatted
4. `cargo test` — all unit, contract, and integration tests pass
5. No `surrealdb` references remain: `grep -r "surrealdb" src/ tests/ Cargo.toml` returns nothing
