---
id: TASK-002
title: "SQLite Migration"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - feature
dependencies: []
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
# Feature Specification: SQLite Migration

**Feature Branch**: `002-sqlite-migration`
**Created**: 2026-02-16
**Status**: Draft
**Input**: User description: "Strip the use of SurrealDB from the current code base and replace with SQLite via sqlx (sqlite feature). SurrealDB is massive overkill for what this MCP server needs a database. SQLite via sqlx (sqlite feature) is a much better fit for what a database is needed in this situation."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Database operations behave identically after migration (Priority: P1)

As a developer running the MCP server, I need all existing database operations (session management, approval requests, checkpoints, continuation prompts, stall alerts) to work exactly as they do today so that the migration is invisible to every consumer of the persistence layer.

**Why this priority**: This is the foundational requirement. If CRUD operations do not behave identically, every downstream feature (Slack handlers, MCP tools, orchestrator) breaks.

**Independent Test**: Run the full existing test suite (unit, contract, integration) against the new SQLite persistence layer and confirm 100% pass rate with no behavioral regressions.

**Acceptance Scenarios**:

1. **Given** the server starts with the new SQLite backend, **When** a session is created, read, updated, and terminated, **Then** the session lifecycle behaves identically to the prior implementation.
2. **Given** a running server with SQLite, **When** an approval request is created and its status transitions through pending → approved → consumed, **Then** the state machine operates correctly with all fields persisted.
3. **Given** the server with SQLite, **When** checkpoints, continuation prompts, and stall alerts are created and queried, **Then** all CRUD operations return correct results.

---

### User Story 2 — Schema bootstraps automatically on first run (Priority: P1)

As the server operator, I need the SQLite database file and schema to be created automatically on first startup so that no manual database setup is required.

**Why this priority**: The current SurrealDB backend creates its schema idempotently on connect. The SQLite replacement must preserve this zero-configuration startup experience.

**Independent Test**: Delete any existing database file, start the server, confirm the database file is created and all tables exist with correct column definitions.

**Acceptance Scenarios**:

1. **Given** no database file exists at the configured path, **When** the server starts, **Then** a new SQLite database file is created and all tables are provisioned.
2. **Given** a database file already exists with the correct schema, **When** the server starts, **Then** the schema bootstrap runs without error and does not destroy existing data.

---

### User Story 3 — In-memory database for tests (Priority: P1)

As a developer running tests, I need the persistence layer to support an in-memory database mode so that tests run fast, remain isolated, and require no filesystem cleanup.

**Why this priority**: The existing test suite relies on `kv-mem` for in-memory SurrealDB. Without an equivalent in-memory SQLite mode, all persistence tests break.

**Independent Test**: Run any persistence test and confirm it uses an in-memory database, completes in under one second, and leaves no database files on disk.

**Acceptance Scenarios**:

1. **Given** the `connect` function is called with the in-memory flag, **When** a database handle is returned, **Then** it uses an in-memory SQLite database with no file created on disk.
2. **Given** an in-memory database, **When** schema bootstrap runs, **Then** all tables are created and CRUD operations succeed.

---

### User Story 4 — Data retention purge continues to work (Priority: P2)

As the server operator, I need the background retention purge task to delete old sessions and their child records after the configured retention period so that the database does not grow unbounded.

**Why this priority**: Retention is a background housekeeping concern. It is important but not required for core functionality to work.

**Independent Test**: Create sessions with termination timestamps older than the retention period, run the purge task, and confirm the sessions and all related child records are deleted.

**Acceptance Scenarios**:

1. **Given** sessions terminated more than 30 days ago exist in the database, **When** the retention purge task runs, **Then** those sessions and their child records (approvals, checkpoints, prompts, stall alerts) are deleted.
2. **Given** sessions that are still active or were terminated within the retention period, **When** the retention purge task runs, **Then** those sessions are not deleted.

---

### User Story 5 — Reduced binary size and build time (Priority: P3)

As a developer, I benefit from a smaller compiled binary and faster build times now that the heavy SurrealDB dependency (and its transitive RocksDB native compilation) is removed.

**Why this priority**: This is a quality-of-life improvement. It is a motivating reason for the migration but not a functional requirement.

**Independent Test**: Compare the release binary size and clean build time before and after the migration.

**Acceptance Scenarios**:

1. **Given** the SurrealDB dependency is fully removed from Cargo.toml, **When** a release build completes, **Then** the resulting binary is smaller than the current binary.
2. **Given** a clean build environment, **When** `cargo build --release` runs, **Then** the build completes faster than the current SurrealDB-based build.

---

### Edge Cases

- **EC-001**: What happens when the database file is locked by another process at startup? → The `sqlx::SqlitePool::connect_with()` call returns `sqlx::Error`, mapped to `AppError::Db`. The server fails to start with a descriptive error message. No retry logic is needed — only one server instance should access the database.
- **EC-002**: What happens when a write fails mid-transaction (crash, disk full)? → SQLite's WAL mode provides crash-safe writes. Incomplete transactions are automatically rolled back on recovery. Disk-full errors return `sqlx::Error` → `AppError::Db`, propagated to the caller.
- **EC-003**: How does the system handle concurrent read/write access from multiple async tasks? → FR-014 specifies a single-writer pool (`max_connections = 1`) with WAL mode. Writes are serialized by the pool. Concurrent reads are handled natively by WAL. No application-level locking is required.
- **EC-004**: What happens when the schema bootstrap encounters a database file with a different (older or corrupted) schema version? → `CREATE TABLE IF NOT EXISTS` (FR-003) succeeds silently for existing tables. A corrupted SQLite file causes `sqlx::Error` on the first query, mapped to `AppError::Db`. No automatic repair — the operator must delete the corrupt file and restart.
- **EC-005**: What happens when the configured database path does not exist or is not writable? → FR-018 requires auto-creating parent directories. If directory creation fails (permissions), the `connect()` function returns `AppError::Db`. If the path exists but is not writable, `sqlx` returns an error on connect.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST replace SurrealDB with SQLite (via `sqlx` with the `sqlite` feature) as the embedded database for all persistence operations.
- **FR-002**: System MUST preserve the existing `Database` type alias pattern so that a single type change in `persistence/db.rs` propagates to all consumers without requiring changes to their function signatures beyond the alias.
- **FR-003**: System MUST provide an idempotent schema bootstrap using `CREATE TABLE IF NOT EXISTS` that creates all tables and columns on first connect and is safe to run on every startup. No versioned migration system is required at this time.
- **FR-004**: System MUST support two connection modes via separate functions: `connect(path)` for file-backed production databases and `connect_memory()` for in-memory test databases (see contracts/repository-api.md Connection Contract).
- **FR-005**: System MUST preserve the same repository API surface (method names, parameter types, return types) for all five repository modules: `SessionRepo`, `ApprovalRepo`, `CheckpointRepo`, `PromptRepo`, and `StallAlertRepo`.
- **FR-006**: System MUST translate all existing SurrealQL queries into equivalent SQLite-compatible SQL executed through `sqlx`.
- **FR-007**: System MUST preserve the existing `AppError::Db` error variant and provide a `From<sqlx::Error>` conversion to replace the current `From<surrealdb::Error>` conversion.
- **FR-008**: System MUST remove the SurrealDB `deserialize_surreal_id` helper and replace it with standard string-based ID handling compatible with SQLite rows.
- **FR-009**: System MUST replace the read-modify-write update pattern with targeted SQL `UPDATE ... SET field = ? WHERE id = ?` statements for each repository mutation method.
- **FR-010**: System MUST store `FLEXIBLE TYPE object` fields (checkpoint `session_state` and `file_hashes`) as JSON text columns in SQLite.
- **FR-011**: System MUST store `progress_snapshot` arrays as JSON text columns in SQLite.
- **FR-012**: System MUST preserve timestamp handling using RFC 3339 string format for all date/time columns.
- **FR-013**: System MUST preserve the retention purge logic, translating SurrealQL `DELETE ... WHERE session_id IN (SELECT ...)` subqueries to equivalent SQLite SQL.
- **FR-014**: System MUST enable SQLite WAL (Write-Ahead Logging) mode for production databases and use a single-writer connection pool to serialize writes while allowing concurrent reads from async tasks.
- **FR-015**: System MUST remove SurrealDB from `Cargo.toml` (both workspace and package dependency sections) after the migration is complete.
- **FR-016**: System MUST update or rewrite all existing tests (unit, contract, integration) that reference SurrealDB types or APIs to use the new SQLite/sqlx equivalents.
- **FR-017**: System MUST preserve the `Arc<Database>` propagation pattern used by `AppState` and ad-hoc repository construction in tool handlers and Slack commands.
- **FR-018**: System MUST store the SQLite database file at a configurable path (default: `data/agent-rc.db`). The connect function MUST auto-create parent directories if they do not exist.
- **FR-019**: System MUST enforce enum constraints (session status, risk level, approval status, prompt type, stall status) using defense-in-depth: (a) `CHECK` constraints in the SQLite DDL catch invalid values at the database level (see contracts/schema.sql.md), and (b) repository methods validate enum values before executing SQL to provide descriptive `AppError::Db` messages. Both layers are required.
- **FR-020**: System MUST use parameterized queries (bind variables) for all dynamic values to prevent SQL injection.

### Key Entities

All five entities are carried forward with identical domain semantics. Only the persistence implementation changes.

- **Session**: Tracks an agent work session. Key attributes: owner, workspace root, status (created/active/paused/terminated/interrupted), operational mode, progress snapshot, activity timestamps.
- **Approval Request**: A diff-based change requiring human approval. Key attributes: session link, file path, diff content, risk level, status lifecycle (pending → approved/rejected → consumed), content hash.
- **Checkpoint**: A point-in-time snapshot of session state and file hashes for crash recovery. Key attributes: session link, session state (arbitrary JSON), file hashes, progress snapshot.
- **Continuation Prompt**: A message forwarded to the human operator for decision. Key attributes: session link, prompt text, type, elapsed time, decision, instruction.
- **Stall Alert**: Tracks idle-session detection and nudge escalation. Key attributes: session link, idle duration, nudge count, status lifecycle, progress snapshot.

## Clarifications

### Session 2026-02-16

- Q: Should the schema bootstrap include a version-tracking mechanism for future schema changes? → A: No. Use `CREATE TABLE IF NOT EXISTS` only; defer migration tooling to a future feature. The server has no released user base with persistent data to migrate.
- Q: How should the persistence layer handle concurrent write contention in SQLite? → A: Single-writer pool (`max_connections = 1`). WAL mode handles concurrent reads naturally. No retry logic needed given the server's low write throughput.
- Q: Should repository mutations preserve the read-modify-write pattern or use targeted SQL UPDATE statements? → A: Replace with targeted SQL `UPDATE ... SET` for each mutation method. Eliminates redundant reads and is idiomatic for SQLite/sqlx.
- Q: Should the connect function auto-create parent directories for the database file path? → A: Yes. Auto-create parent directories if they do not exist, matching the current RocksDB behavior and preserving zero-configuration startup.
- Q: Where should enum constraint validation occur since SQLite lacks ASSERT? → A: Validate in repository methods (create/update) before executing SQL. Keeps domain models free from validation logic while guaranteeing no invalid value reaches the database. SQLite CHECK constraints provide a second safety net at the DDL level (defense-in-depth).
- Q: Why does FR-005 use `PromptRepo` when the entity is `ContinuationPrompt`? → `PromptRepo` is the existing codebase abbreviation for the repository managing `ContinuationPrompt` entities (table: `continuation_prompt`, module: `prompt_repo.rs`). This naming convention is inherited from the current implementation and is not a new inconsistency.

## Assumptions

- The existing domain model structs (`Session`, `ApprovalRequest`, `Checkpoint`, `ContinuationPrompt`, `StallAlert`) retain their field names and domain semantics. Numeric fields that are unsigned in the current code (`u32`, `u64`) are widened to `i64` to align with SQLite's `INTEGER` affinity (see data-model.md Type Changes Summary). Only serde attributes related to SurrealDB ID handling are removed or adjusted.
- The `Arc<Database>` pattern is preserved. The `Database` type alias changes from `Surreal<Db>` to a `sqlx::SqlitePool` (or equivalent).
- No data migration from an existing SurrealDB database is required. This is a clean replacement — any existing SurrealDB data is discarded.
- The `GROUP ALL` aggregation used by `count_active()` translates directly to a standard SQL `SELECT COUNT(*)` query.
- The `config.db_path()` method is updated to return a file path (default: `data/agent-rc.db`) instead of a directory path.
- `sqlx` runtime is set to `tokio` to match the existing async runtime.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All existing tests (unit, contract, integration) pass against the SQLite backend with zero behavioral regressions.
- **SC-002**: The SurrealDB crate is completely absent from `Cargo.toml` and `Cargo.lock` after migration.
- **SC-003**: Server startup with a fresh database completes schema bootstrap and becomes operational without manual database setup.
- **SC-004**: The release binary size is measurably smaller than the pre-migration binary.
- **SC-005**: Clean build time (`cargo build --release`) is measurably faster than the pre-migration build.
- **SC-006**: All five repository modules pass their CRUD and query tests using an in-memory SQLite database.
- **SC-007**: The retention purge task successfully deletes expired sessions and all child records in a single test run.
- **SC-008**: No `surrealdb` import, type reference, or API call remains anywhere in the codebase.


---
title: SQLite Migration Data Model
description: Entity definitions, SQLite schema, and repository API surface for the SurrealDB-to-SQLite migration
ms.date: 2026-02-16
---

# Data Model: SQLite Migration

## Entities

All five entities carry forward with identical domain semantics. Changes are limited to persistence annotations and ID handling.

### Session

Tracks an agent work session from creation through termination.

| Field | Rust Type | SQLite Column | SQLite Type | Constraints |
|---|---|---|---|---|
| id | `String` | `id` | `TEXT PRIMARY KEY NOT NULL` | UUID string |
| owner_user_id | `String` | `owner_user_id` | `TEXT NOT NULL` | Slack user ID |
| workspace_root | `String` | `workspace_root` | `TEXT NOT NULL` | Absolute path |
| status | `String` | `status` | `TEXT NOT NULL` | `CHECK(status IN ('created','active','paused','terminated','interrupted'))` |
| prompt | `Option<String>` | `prompt` | `TEXT` | Nullable |
| mode | `String` | `mode` | `TEXT NOT NULL` | `CHECK(mode IN ('remote','local','hybrid'))` |
| created_at | `DateTime<Utc>` | `created_at` | `TEXT NOT NULL` | RFC 3339 |
| updated_at | `DateTime<Utc>` | `updated_at` | `TEXT NOT NULL` | RFC 3339 |
| terminated_at | `Option<DateTime<Utc>>` | `terminated_at` | `TEXT` | RFC 3339 or NULL |
| last_tool | `Option<String>` | `last_tool` | `TEXT` | Nullable |
| nudge_count | `i64` | `nudge_count` | `INTEGER NOT NULL` | Default 0 |
| stall_paused | `bool` | `stall_paused` | `INTEGER NOT NULL` | 0 or 1, default 0 |
| progress_snapshot | `Option<Vec<ProgressItem>>` | `progress_snapshot` | `TEXT` | JSON array or NULL |

**State transitions**: created → active → paused ↔ active → terminated / interrupted

### Approval Request

A diff-based change requiring human approval via Slack.

| Field | Rust Type | SQLite Column | SQLite Type | Constraints |
|---|---|---|---|---|
| id | `String` | `id` | `TEXT PRIMARY KEY NOT NULL` | UUID string |
| session_id | `String` | `session_id` | `TEXT NOT NULL` | FK to session |
| title | `String` | `title` | `TEXT NOT NULL` | |
| description | `Option<String>` | `description` | `TEXT` | Nullable |
| diff_content | `String` | `diff_content` | `TEXT NOT NULL` | Unified diff |
| file_path | `String` | `file_path` | `TEXT NOT NULL` | Relative to workspace |
| risk_level | `String` | `risk_level` | `TEXT NOT NULL` | `CHECK(risk_level IN ('low','high','critical'))` |
| status | `String` | `status` | `TEXT NOT NULL` | `CHECK(status IN ('pending','approved','rejected','expired','consumed','interrupted'))` |
| original_hash | `String` | `original_hash` | `TEXT NOT NULL` | SHA-256 hex |
| slack_ts | `Option<String>` | `slack_ts` | `TEXT` | Slack message timestamp |
| created_at | `DateTime<Utc>` | `created_at` | `TEXT NOT NULL` | RFC 3339 |
| consumed_at | `Option<DateTime<Utc>>` | `consumed_at` | `TEXT` | RFC 3339 or NULL |

**State transitions**: pending → approved/rejected/expired → consumed / interrupted

### Checkpoint

Point-in-time snapshot for crash recovery.

| Field | Rust Type | SQLite Column | SQLite Type | Constraints |
|---|---|---|---|---|
| id | `String` | `id` | `TEXT PRIMARY KEY NOT NULL` | UUID string |
| session_id | `String` | `session_id` | `TEXT NOT NULL` | FK to session |
| label | `Option<String>` | `label` | `TEXT` | Nullable |
| session_state | `serde_json::Value` | `session_state` | `TEXT NOT NULL` | JSON object |
| file_hashes | `HashMap<String, String>` | `file_hashes` | `TEXT NOT NULL` | JSON object |
| workspace_root | `String` | `workspace_root` | `TEXT NOT NULL` | Absolute path |
| progress_snapshot | `Option<Vec<ProgressItem>>` | `progress_snapshot` | `TEXT` | JSON array or NULL |
| created_at | `DateTime<Utc>` | `created_at` | `TEXT NOT NULL` | RFC 3339 |

### Continuation Prompt

Message forwarded to the human operator for decision.

| Field | Rust Type | SQLite Column | SQLite Type | Constraints |
|---|---|---|---|---|
| id | `String` | `id` | `TEXT PRIMARY KEY NOT NULL` | UUID string |
| session_id | `String` | `session_id` | `TEXT NOT NULL` | FK to session |
| prompt_text | `String` | `prompt_text` | `TEXT NOT NULL` | |
| prompt_type | `String` | `prompt_type` | `TEXT NOT NULL` | `CHECK(prompt_type IN ('continuation','clarification','error_recovery','resource_warning'))` |
| elapsed_seconds | `Option<i64>` | `elapsed_seconds` | `INTEGER` | Nullable |
| actions_taken | `Option<i64>` | `actions_taken` | `INTEGER` | Nullable |
| decision | `Option<String>` | `decision` | `TEXT` | Nullable |
| instruction | `Option<String>` | `instruction` | `TEXT` | Nullable |
| slack_ts | `Option<String>` | `slack_ts` | `TEXT` | Slack message timestamp |
| created_at | `DateTime<Utc>` | `created_at` | `TEXT NOT NULL` | RFC 3339 |

### Stall Alert

Idle-session detection and nudge escalation tracking.

| Field | Rust Type | SQLite Column | SQLite Type | Constraints |
|---|---|---|---|---|
| id | `String` | `id` | `TEXT PRIMARY KEY NOT NULL` | UUID string |
| session_id | `String` | `session_id` | `TEXT NOT NULL` | FK to session |
| last_tool | `Option<String>` | `last_tool` | `TEXT` | Nullable |
| last_activity_at | `DateTime<Utc>` | `last_activity_at` | `TEXT NOT NULL` | RFC 3339 |
| idle_seconds | `i64` | `idle_seconds` | `INTEGER NOT NULL` | |
| nudge_count | `i64` | `nudge_count` | `INTEGER NOT NULL` | Default 0 |
| status | `String` | `status` | `TEXT NOT NULL` | `CHECK(status IN ('pending','nudged','self_recovered','escalated','dismissed'))` |
| nudge_message | `Option<String>` | `nudge_message` | `TEXT` | Nullable |
| progress_snapshot | `Option<Vec<ProgressItem>>` | `progress_snapshot` | `TEXT` | JSON array or NULL |
| slack_ts | `Option<String>` | `slack_ts` | `TEXT` | Slack message timestamp |
| created_at | `DateTime<Utc>` | `created_at` | `TEXT NOT NULL` | RFC 3339 |

**State transitions**: pending → nudged → self_recovered / escalated / dismissed

### ProgressItem (embedded value type)

Serialized as JSON within `progress_snapshot` TEXT columns.

| Field | Rust Type | JSON Key |
|---|---|---|
| label | `String` | `label` |
| status | `String` | `status` |

## Relationships

```text
Session 1──* ApprovalRequest   (via session_id)
Session 1──* Checkpoint         (via session_id)
Session 1──* ContinuationPrompt (via session_id)
Session 1──* StallAlert         (via session_id)
```

All relationships are enforced at the application layer via `session_id` string foreign keys. No SQLite `FOREIGN KEY` constraints are used (consistent with the current SurrealDB implementation that uses string-typed foreign keys with no record links).

## Repository API Surface

All repository method signatures are preserved. Internal implementations change from SurrealDB SDK calls to sqlx queries.

### SessionRepo (12 methods)

| Method | SQL Pattern | Return Type |
|---|---|---|
| `create(session)` | `INSERT INTO session (...) VALUES (...)` | `Result<Session>` |
| `get_by_id(id)` | `SELECT * FROM session WHERE id = ?` | `Result<Option<Session>>` |
| `update_status(id, status)` | `UPDATE session SET status = ?, updated_at = ? WHERE id = ?` | `Result<()>` |
| `update_last_activity(id, tool)` | `UPDATE session SET last_tool = ?, updated_at = ? WHERE id = ?` | `Result<()>` |
| `list_active()` | `SELECT * FROM session WHERE status = 'active'` | `Result<Vec<Session>>` |
| `update_progress_snapshot(id, snapshot)` | `UPDATE session SET progress_snapshot = ?, updated_at = ? WHERE id = ?` | `Result<()>` |
| `set_terminated(id)` | `UPDATE session SET status = 'terminated', terminated_at = ?, updated_at = ? WHERE id = ?` | `Result<()>` |
| `count_active()` | `SELECT COUNT(*) FROM session WHERE status = 'active'` | `Result<i64>` |
| `get_most_recent_interrupted()` | `SELECT * FROM session WHERE status = 'interrupted' ORDER BY updated_at DESC LIMIT 1` | `Result<Option<Session>>` |
| `list_interrupted()` | `SELECT * FROM session WHERE status = 'interrupted'` | `Result<Vec<Session>>` |
| `list_active_or_paused()` | `SELECT * FROM session WHERE status IN ('active', 'paused')` | `Result<Vec<Session>>` |
| `update_mode(id, mode)` | `UPDATE session SET mode = ?, updated_at = ? WHERE id = ?` | `Result<()>` |

### ApprovalRepo (6 methods)

| Method | SQL Pattern | Return Type |
|---|---|---|
| `create(approval)` | `INSERT INTO approval_request (...) VALUES (...)` | `Result<ApprovalRequest>` |
| `get_by_id(id)` | `SELECT * FROM approval_request WHERE id = ?` | `Result<Option<ApprovalRequest>>` |
| `get_pending_for_session(session_id)` | `SELECT * FROM approval_request WHERE session_id = ? AND status = 'pending'` | `Result<Vec<ApprovalRequest>>` |
| `update_status(id, status)` | `UPDATE approval_request SET status = ? WHERE id = ?` | `Result<()>` |
| `mark_consumed(id)` | `UPDATE approval_request SET status = 'consumed', consumed_at = ? WHERE id = ?` | `Result<()>` |
| `list_pending()` | `SELECT * FROM approval_request WHERE status = 'pending'` | `Result<Vec<ApprovalRequest>>` |

### CheckpointRepo (4 methods)

| Method | SQL Pattern | Return Type |
|---|---|---|
| `create(checkpoint)` | `INSERT INTO checkpoint (...) VALUES (...)` | `Result<Checkpoint>` |
| `get_by_id(id)` | `SELECT * FROM checkpoint WHERE id = ?` | `Result<Option<Checkpoint>>` |
| `list_for_session(session_id)` | `SELECT * FROM checkpoint WHERE session_id = ? ORDER BY created_at DESC` | `Result<Vec<Checkpoint>>` |
| `delete_for_session(session_id)` | `DELETE FROM checkpoint WHERE session_id = ?` | `Result<()>` |

### PromptRepo (5 methods)

| Method | SQL Pattern | Return Type |
|---|---|---|
| `create(prompt)` | `INSERT INTO continuation_prompt (...) VALUES (...)` | `Result<ContinuationPrompt>` |
| `get_by_id(id)` | `SELECT * FROM continuation_prompt WHERE id = ?` | `Result<Option<ContinuationPrompt>>` |
| `get_pending_for_session(session_id)` | `SELECT * FROM continuation_prompt WHERE session_id = ? AND decision IS NULL LIMIT 1` | `Result<Option<ContinuationPrompt>>` |
| `update_decision(id, decision, instruction)` | `UPDATE continuation_prompt SET decision = ?, instruction = ? WHERE id = ?` | `Result<()>` |
| `list_pending()` | `SELECT * FROM continuation_prompt WHERE decision IS NULL` | `Result<Vec<ContinuationPrompt>>` |

### StallAlertRepo (5 methods)

| Method | SQL Pattern | Return Type |
|---|---|---|
| `create(alert)` | `INSERT INTO stall_alert (...) VALUES (...)` | `Result<StallAlert>` |
| `get_active_for_session(session_id)` | `SELECT * FROM stall_alert WHERE session_id = ? AND status IN ('pending','nudged') LIMIT 1` | `Result<Option<StallAlert>>` |
| `update_status(id, status)` | `UPDATE stall_alert SET status = ? WHERE id = ?` | `Result<()>` |
| `increment_nudge_count(id)` | `UPDATE stall_alert SET nudge_count = nudge_count + 1 WHERE id = ?` | `Result<()>` |
| `dismiss(id)` | `UPDATE stall_alert SET status = 'dismissed' WHERE id = ?` | `Result<()>` |

## Type Changes Summary

| Location | Before (SurrealDB) | After (SQLite/sqlx) |
|---|---|---|
| `Database` type alias | `Surreal<Db>` | `sqlx::SqlitePool` |
| `AppState.db` | `Arc<Surreal<Db>>` | `Arc<sqlx::SqlitePool>` |
| `ToolContext.db` | `Arc<Surreal<Db>>` | `Arc<sqlx::SqlitePool>` |
| Model `id` serde attrs | `skip_serializing` + `deserialize_surreal_id` | Plain `String` field, no special attrs |
| `nudge_count` type | `u32` | `i64` (SQLite INTEGER) |
| `idle_seconds` type | `u64` | `i64` (SQLite INTEGER) |
| `elapsed_seconds` type | `Option<u64>` | `Option<i64>` |
| `actions_taken` type | `Option<u32>` | `Option<i64>` |
| `From` impl | `From<surrealdb::Error>` | `From<sqlx::Error>` |


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




---

## Checklists

---
title: Specification Quality Checklist
description: Validates specification completeness and quality for the SQLite Migration feature before proceeding to planning
ms.date: 2026-02-16
---

# Specification Quality Checklist: SQLite Migration

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-02-16
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- All items passed on the first validation iteration.
- The spec references `sqlx` and `SQLite` by name because the feature IS a technology swap. The spec constrains WHAT is replaced and WHY, not HOW the implementation is structured.
- No [NEEDS CLARIFICATION] markers were needed. The feature scope is well-defined: a 1:1 persistence layer replacement with no data migration.




---

## Contracts

---
title: Repository API Contract
description: Method signatures and behavioral contracts for all five repository modules
ms.date: 2026-02-16
---

# Contract: Repository API Surface

All repository methods accept `&SqlitePool` as first parameter and return `Result<T, AppError>`.

## Connection Contract

```rust
/// Production: file-backed SQLite
pub async fn connect(path: &str) -> Result<SqlitePool> {
    // SqliteConnectOptions::from_str(path)
    //   .create_if_missing(true)
    //   .journal_mode(SqliteJournalMode::Wal)
    // SqlitePoolOptions::new().max_connections(1).connect_with(opts)
}

/// Test: in-memory SQLite
pub async fn connect_memory() -> Result<SqlitePool> {
    // SqliteConnectOptions::from_str("sqlite::memory:")
    // SqlitePoolOptions::new().max_connections(1).min_connections(1)
}

/// Schema bootstrap (called once after connect)
pub async fn bootstrap_schema(pool: &SqlitePool) -> Result<()> {
    // sqlx::raw_sql(DDL_STRING).execute(pool).await
}
```

## Error Mapping Contract

```rust
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Db(err.to_string())
    }
}
```

All `sqlx::Error` variants map to `AppError::Db`. No special-casing of `RowNotFound` — methods returning `Option<T>` use `fetch_optional` which returns `None` (not an error).

## Repository Method Contracts

### Behavioral guarantees (all repos)

1. **Create** methods generate the `id` field (UUID) before insertion
2. **Get-by-ID** methods return `Ok(None)` for missing records, never `Err`
3. **Update** methods return `Ok(())` even if the target row does not exist (affected_rows may be 0)
4. **List** methods return `Ok(Vec::new())` for empty result sets
5. **Delete** methods return `Ok(())` even if no rows are deleted
6. **JSON fields** are serialized with `serde_json::to_string()` before INSERT and deserialized with `serde_json::from_str()` after SELECT
7. **Enum validation** is performed in repository methods before SQL execution; invalid values return `AppError::Db`

### SessionRepo

```rust
pub async fn create(pool: &SqlitePool, session: &Session) -> Result<Session>;
pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Session>>;
pub async fn update_status(pool: &SqlitePool, id: &str, status: &str) -> Result<()>;
pub async fn update_last_activity(pool: &SqlitePool, id: &str, tool: &str) -> Result<()>;
pub async fn list_active(pool: &SqlitePool) -> Result<Vec<Session>>;
pub async fn update_progress_snapshot(pool: &SqlitePool, id: &str, snapshot: &[ProgressItem]) -> Result<()>;
pub async fn set_terminated(pool: &SqlitePool, id: &str) -> Result<()>;
pub async fn count_active(pool: &SqlitePool) -> Result<i64>;
pub async fn get_most_recent_interrupted(pool: &SqlitePool) -> Result<Option<Session>>;
pub async fn list_interrupted(pool: &SqlitePool) -> Result<Vec<Session>>;
pub async fn list_active_or_paused(pool: &SqlitePool) -> Result<Vec<Session>>;
pub async fn update_mode(pool: &SqlitePool, id: &str, mode: &str) -> Result<()>;
```

### ApprovalRepo

```rust
pub async fn create(pool: &SqlitePool, approval: &ApprovalRequest) -> Result<ApprovalRequest>;
pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<ApprovalRequest>>;
pub async fn get_pending_for_session(pool: &SqlitePool, session_id: &str) -> Result<Vec<ApprovalRequest>>;
pub async fn update_status(pool: &SqlitePool, id: &str, status: &str) -> Result<()>;
pub async fn mark_consumed(pool: &SqlitePool, id: &str) -> Result<()>;
pub async fn list_pending(pool: &SqlitePool) -> Result<Vec<ApprovalRequest>>;
```

### CheckpointRepo

```rust
pub async fn create(pool: &SqlitePool, checkpoint: &Checkpoint) -> Result<Checkpoint>;
pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Checkpoint>>;
pub async fn list_for_session(pool: &SqlitePool, session_id: &str) -> Result<Vec<Checkpoint>>;
pub async fn delete_for_session(pool: &SqlitePool, session_id: &str) -> Result<()>;
```

### PromptRepo

```rust
pub async fn create(pool: &SqlitePool, prompt: &ContinuationPrompt) -> Result<ContinuationPrompt>;
pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<ContinuationPrompt>>;
pub async fn get_pending_for_session(pool: &SqlitePool, session_id: &str) -> Result<Option<ContinuationPrompt>>;
pub async fn update_decision(pool: &SqlitePool, id: &str, decision: &str, instruction: Option<&str>) -> Result<()>;
pub async fn list_pending(pool: &SqlitePool) -> Result<Vec<ContinuationPrompt>>;
```

### StallAlertRepo

```rust
pub async fn create(pool: &SqlitePool, alert: &StallAlert) -> Result<StallAlert>;
pub async fn get_active_for_session(pool: &SqlitePool, session_id: &str) -> Result<Option<StallAlert>>;
pub async fn update_status(pool: &SqlitePool, id: &str, status: &str) -> Result<()>;
pub async fn increment_nudge_count(pool: &SqlitePool, id: &str) -> Result<()>;
pub async fn dismiss(pool: &SqlitePool, id: &str) -> Result<()>;
```

### RetentionService

```rust
/// Purge sessions (and all children) terminated more than retention_days ago.
/// Deletion order: stall_alerts → checkpoints → continuation_prompts → approval_requests → sessions
pub async fn purge_expired(pool: &SqlitePool, retention_days: u32) -> Result<u64>;
```

## Row Struct Contract

Each entity has a corresponding `sqlx::FromRow` struct for deserialization:

```rust
#[derive(sqlx::FromRow)]
struct SessionRow {
    id: String,
    owner_user_id: String,
    workspace_root: String,
    status: String,
    prompt: Option<String>,
    mode: String,
    created_at: String,
    updated_at: String,
    terminated_at: Option<String>,
    last_tool: Option<String>,
    nudge_count: i64,
    stall_paused: i64,          // 0 or 1, converted to bool
    progress_snapshot: Option<String>,  // JSON, deserialized to Vec<ProgressItem>
}
```

Row structs are private to the `persistence` module. The public API uses domain model structs from `src/models/`.


---

---
title: SQLite Schema DDL Contract
description: Complete CREATE TABLE IF NOT EXISTS statements for all five tables
ms.date: 2026-02-16
---

# Contract: SQLite Schema DDL

All DDL statements executed via `sqlx::raw_sql()` during `bootstrap_schema(pool)`.

```sql
-- Executed as a single raw_sql() call with all statements

CREATE TABLE IF NOT EXISTS session (
    id              TEXT PRIMARY KEY NOT NULL,
    owner_user_id   TEXT NOT NULL,
    workspace_root  TEXT NOT NULL,
    status          TEXT NOT NULL CHECK(status IN ('created','active','paused','terminated','interrupted')),
    prompt          TEXT,
    mode            TEXT NOT NULL CHECK(mode IN ('remote','local','hybrid')),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    terminated_at   TEXT,
    last_tool       TEXT,
    nudge_count     INTEGER NOT NULL DEFAULT 0,
    stall_paused    INTEGER NOT NULL DEFAULT 0,
    progress_snapshot TEXT
);

CREATE TABLE IF NOT EXISTS approval_request (
    id              TEXT PRIMARY KEY NOT NULL,
    session_id      TEXT NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT,
    diff_content    TEXT NOT NULL,
    file_path       TEXT NOT NULL,
    risk_level      TEXT NOT NULL CHECK(risk_level IN ('low','high','critical')),
    status          TEXT NOT NULL CHECK(status IN ('pending','approved','rejected','expired','consumed','interrupted')),
    original_hash   TEXT NOT NULL,
    slack_ts        TEXT,
    created_at      TEXT NOT NULL,
    consumed_at     TEXT
);

CREATE TABLE IF NOT EXISTS checkpoint (
    id              TEXT PRIMARY KEY NOT NULL,
    session_id      TEXT NOT NULL,
    label           TEXT,
    session_state   TEXT NOT NULL,
    file_hashes     TEXT NOT NULL,
    workspace_root  TEXT NOT NULL,
    progress_snapshot TEXT,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS continuation_prompt (
    id              TEXT PRIMARY KEY NOT NULL,
    session_id      TEXT NOT NULL,
    prompt_text     TEXT NOT NULL,
    prompt_type     TEXT NOT NULL CHECK(prompt_type IN ('continuation','clarification','error_recovery','resource_warning')),
    elapsed_seconds INTEGER,
    actions_taken   INTEGER,
    decision        TEXT,
    instruction     TEXT,
    slack_ts        TEXT,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS stall_alert (
    id              TEXT PRIMARY KEY NOT NULL,
    session_id      TEXT NOT NULL,
    last_tool       TEXT,
    last_activity_at TEXT NOT NULL,
    idle_seconds    INTEGER NOT NULL,
    nudge_count     INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL CHECK(status IN ('pending','nudged','self_recovered','escalated','dismissed')),
    nudge_message   TEXT,
    progress_snapshot TEXT,
    slack_ts        TEXT,
    created_at      TEXT NOT NULL
);
```

## Schema Invariants

1. No `FOREIGN KEY` constraints — referential integrity enforced at application layer
2. No `AUTOINCREMENT` — all IDs are application-generated UUIDs
3. Datetime columns store RFC 3339 strings (e.g., `2026-02-16T12:00:00Z`)
4. JSON columns (`progress_snapshot`, `session_state`, `file_hashes`) store serialized JSON as TEXT
5. Boolean columns (`stall_paused`) stored as INTEGER (0/1)
6. Enum columns use CHECK constraints matching the exact variants
7. All tables use `CREATE TABLE IF NOT EXISTS` — idempotent, safe to call on every startup

<!-- SECTION:DESCRIPTION:END -->
