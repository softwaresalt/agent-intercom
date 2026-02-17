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
- **FR-018**: System MUST store the SQLite database file at a configurable path (default: `data/monocoque.db`). The connect function MUST auto-create parent directories if they do not exist.
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
- The `config.db_path()` method is updated to return a file path (default: `data/monocoque.db`) instead of a directory path.
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
