# Phase 4 Memory — Retention Purge (002-sqlite-migration)

**Date**: 2026-02-16
**Phase**: 4 of 6
**Tasks**: T042, T043
**Commit**: (pending)

## What Was Done

Implemented the retention purge service (`src/persistence/retention.rs`) with SQLite
queries replacing the previous `todo!()` stub. Wrote 3 integration tests in TDD style.

### Key Implementation Details

- `purge()` takes `db: &Database` (type alias for `SqlitePool`) and `retention_days: u32`
- Cutoff calculated as `chrono::Utc::now() - chrono::Duration::days(i64::from(retention_days))`
- Cutoff serialized to RFC3339 string for SQLite comparison
- 5 cascading DELETE queries using subquery pattern:
  `DELETE FROM {table} WHERE session_id IN (SELECT id FROM session WHERE terminated_at IS NOT NULL AND terminated_at < ?1)`
- Deletion order: stall_alert → checkpoint → continuation_prompt → approval_request → session
- `db` passed directly to `.execute(db)` since `&SqlitePool` implements `Executor` — no `.as_ref()` needed (unlike repos which unwrap `Arc<Database>`)
- `spawn_retention_task()` preserved unchanged — runs every 3600s via `tokio::time::interval_at`

### Integration Tests (retention_tests.rs)

1. `purge_deletes_expired_sessions_and_children` — creates expired/recent/active sessions with full child records, verifies only expired deleted
2. `purge_with_no_expired_sessions_is_noop` — verifies no deletion when all sessions recent
3. `purge_respects_retention_days_config` — boundary test with different retention_days values

### Test Counts

339 total: 139 unit + 138 contract + 45 integration + 17 lib

### Patterns Learned

- `Database = SqlitePool` (type alias in `persistence/db.rs`) — when function takes `&Database`, it's already `&SqlitePool`, so pass directly to `.execute()` without `.as_ref()`
- Repo structs hold `Arc<Database>` and use `self.db.as_ref()` (Arc deref), but standalone functions taking `&Database` skip that layer

## Files Modified

- `src/persistence/retention.rs` — replaced `todo!()` with 5 cascading DELETE queries
- `tests/integration/retention_tests.rs` — new file, 3 integration tests
- `tests/integration.rs` — registered `retention_tests` module
- `specs/002-sqlite-migration/tasks.md` — marked T042, T043 done
