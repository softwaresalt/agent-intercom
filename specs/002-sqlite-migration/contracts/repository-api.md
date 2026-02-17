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
