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
