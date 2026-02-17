---
title: SQLite Migration Research
description: Research findings for replacing SurrealDB with SQLite via sqlx in the monocoque-agent-rc persistence layer
ms.date: 2026-02-16
---

# SQLite Migration Research

## 1. sqlx Cargo.toml Configuration

**Decision**: Use `sqlx 0.8` with `runtime-tokio`, `sqlite`, `json`, `chrono`, and `macros` features.
**Rationale**: sqlx 0.8 is the latest stable release. The `sqlite` feature bundles libsqlite3 (no system dependency needed). `json` enables `sqlx::types::Json<T>` for JSON TEXT columns. `chrono` enables direct `DateTime<Utc>` encoding/decoding. `macros` enables `#[derive(sqlx::FromRow)]`.
**Alternatives considered**: sqlx 0.9 (pre-release, too risky); rusqlite (synchronous, incompatible with tokio async); sled (not SQL-based).

```toml
[workspace.dependencies]
sqlx = { version = "0.8", features = [
    "runtime-tokio",
    "sqlite",
    "json",
    "chrono",
    "macros",
] }
```

## 2. Connection Pool Strategy

**Decision**: Use `SqlitePool` with `max_connections = 1`, WAL journal mode, and `create_if_missing(true)`. For in-memory test pools, set `min_connections = 1` to keep the database alive.
**Rationale**: SQLite is single-writer by nature. A single-connection pool serializes writes without requiring retry logic. WAL mode allows concurrent reads. The `min_connections = 1` setting for in-memory mode prevents the pool from closing all idle connections and destroying the database.
**Alternatives considered**: Multi-connection pool with `busy_timeout` (unnecessary complexity for low write throughput); shared-cache URI (more complex, not needed with single-writer pool).

## 3. Schema Bootstrap Strategy

**Decision**: Use `sqlx::raw_sql()` with `CREATE TABLE IF NOT EXISTS` statements. No versioned migration system.
**Rationale**: `raw_sql()` supports multiple semicolon-separated statements in a single call, matching the current SurrealDB batch `DEFINE` approach. `CREATE TABLE IF NOT EXISTS` is idempotent, matching SurrealDB's convergent `DEFINE TABLE` behavior. The server has no released user base with persistent data to migrate.
**Alternatives considered**: `sqlx::migrate!()` embedded migrations (premature complexity); manual version tracking table (deferred to future feature).

**Key findings**:

- `sqlx::raw_sql()` supports multi-statement DDL batches (semicolon separated). `sqlx::query()` does NOT.
- SQLite supports `CHECK` constraints natively (e.g., `CHECK(status IN ('active','paused',...))`) as a supplement to application-layer validation.
- Boolean fields map to `INTEGER` (0/1). sqlx handles `bool` ↔ `INTEGER` conversion.
- JSON data stored as `TEXT` columns. SQLite's bundled JSON1 extension provides JSON functions if needed.

## 4. Update Pattern

**Decision**: Replace the SurrealDB read-modify-write pattern with targeted `UPDATE ... SET field = ? WHERE id = ?` statements.
**Rationale**: Eliminates redundant reads, is idiomatic for SQL databases, and reduces the risk of lost updates. The single-writer pool ensures no concurrent write conflicts.
**Alternatives considered**: Preserving read-modify-write (wasteful round trip); partial updates via JSON_PATCH (over-engineered).

## 5. ID Handling

**Decision**: Remove `deserialize_surreal_id` and `surrealdb::sql::Thing` entirely. Use plain `String` IDs with `TEXT PRIMARY KEY` columns. Remove `#[serde(skip_serializing, deserialize_with)]` attributes from model `id` fields.
**Rationale**: SurrealDB's `Thing` type wrapped IDs with a table prefix. SQLite has no such concept. The existing UUID-based string IDs map directly to `TEXT PRIMARY KEY`.
**Alternatives considered**: None. This is a straightforward simplification.

## 6. Row Deserialization

**Decision**: Use intermediate `*Row` structs with `#[derive(sqlx::FromRow)]` for database rows, then convert to domain model structs.
**Rationale**: Domain model structs use `chrono::DateTime<Utc>`, `serde_json::Value`, `Vec<ProgressItem>`, and enum-like status fields that require type conversion from SQLite wire types. Intermediate row structs keep the conversion explicit and avoid coupling domain models to sqlx traits.

**Key patterns**:

- `#[sqlx(json)]` attribute auto-deserializes JSON TEXT into typed fields. Supports `#[sqlx(json(nullable))]` for `Option<T>`.
- SQLite `INTEGER` maps to `i64` in sqlx (not `u64`). The `nudge_count: u32` field needs adjustment.
- With the `chrono` feature, `DateTime<Utc>` can be used directly in `FromRow` structs (stored as TEXT, auto-converted).
- `fetch_one()`, `fetch_optional()`, `fetch_all()` map to "exactly one", "zero or one", "many" result expectations.

## 7. Error Handling

**Decision**: Replace `From<surrealdb::Error>` with `From<sqlx::Error>`. Map `RowNotFound` to `AppError::NotFound`. All other variants map to `AppError::Db`.
**Rationale**: Direct 1:1 replacement. The `RowNotFound` variant is semantically equivalent to `AppError::NotFound`, enabling cleaner error handling in repository methods that use `fetch_one()`.
**Alternatives considered**: Matching additional variants like `PoolTimedOut` (unnecessary given single-connection pool).

## 8. Retention Purge Translation

**Decision**: Translate SurrealQL `DELETE FROM {table} WHERE session_id IN (SELECT VALUE id FROM session WHERE ...)` directly to `DELETE FROM {table} WHERE session_id IN (SELECT id FROM session WHERE ...)`.
**Rationale**: SQLite fully supports correlated subqueries in DELETE. The only SurrealDB-specific syntax is `SELECT VALUE` (which becomes `SELECT id`). The format-interpolated table names are compile-time literals, so no injection risk.
**Alternatives considered**: None. This is a direct translation.

## 9. Constitution Impact

**Violation found**: Principle VI (Single-Binary Simplicity) states "SurrealDB in embedded mode is the sole persistence layer; do not introduce additional databases or caches."

**Justification**: This migration replaces SurrealDB 1:1 with SQLite. It does not introduce an additional database. The intent of Principle VI is maintained (single embedded persistence layer, no proliferation). The constitution text needs a minor amendment to say "SQLite" instead of "SurrealDB" after this feature merges. The violation is cosmetic, not architectural.

## 10. Dependency Impact

| Dependency | Before | After |
|---|---|---|
| `surrealdb` | 1.5 (kv-rocksdb, kv-mem) | Removed |
| `sqlx` | Not present | 0.8 (runtime-tokio, sqlite, json, chrono, macros) |
| RocksDB (transitive) | Pulled by surrealdb | Removed |
| libsqlite3 (transitive) | Not present | Bundled by sqlx-sqlite |

**Expected impact**: Significant reduction in dependency tree size, compile time, and binary size. SurrealDB pulled in dozens of transitive dependencies including RocksDB (C++ build). SQLite is a single C file bundled by sqlx.
