# Data Model: Intercom Advanced Features

**Feature**: 004-intercom-advanced-features
**Date**: 2026-02-26

## New Entities

### SteeringMessage

Operator-to-agent message queued for delivery via `ping`.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | TEXT | PK, NOT NULL | UUID v4 prefixed `steer:` |
| session_id | TEXT | NOT NULL, FK → session.id | Target session |
| channel_id | TEXT | | Slack channel the message originated from |
| message | TEXT | NOT NULL | Free-text instruction from operator |
| source | TEXT | NOT NULL, CHECK IN ('slack','ipc') | Ingestion path |
| created_at | TEXT | NOT NULL | ISO 8601 timestamp |
| consumed | INTEGER | NOT NULL, DEFAULT 0 | 0 = pending, 1 = delivered via ping |

**Indexes**: `idx_steering_session_consumed ON steering_message(session_id, consumed)`

**Lifecycle**:
- Created: Slack app mention, `/intercom steer`, or `intercom-ctl steer`
- Consumed: Marked 1 when delivered in `ping` response
- Purged: By retention policy (same as session data, configurable days)

---

### TaskInboxItem

Work item queued for agent cold-start delivery via `reboot`.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | TEXT | PK, NOT NULL | UUID v4 prefixed `task:` |
| channel_id | TEXT | | Channel scope for delivery matching |
| message | TEXT | NOT NULL | Work item text |
| source | TEXT | NOT NULL, CHECK IN ('slack','ipc') | Ingestion path |
| created_at | TEXT | NOT NULL | ISO 8601 timestamp |
| consumed | INTEGER | NOT NULL, DEFAULT 0 | 0 = pending, 1 = delivered at startup |

**Indexes**: `idx_inbox_channel_consumed ON task_inbox(channel_id, consumed)`

**Lifecycle**:
- Created: `/intercom task` slash command or `intercom-ctl task`
- Consumed: Marked 1 when delivered in `reboot` response
- Purged: By retention policy

**Note**: No `session_id` column — inbox items exist before any session. Channel-scoped: delivered only to sessions matching the item's `channel_id`.

---

### AuditLogEntry (filesystem, not DB)

Structured record of an agent interaction event, written to `.intercom/logs/audit-YYYY-MM-DD.jsonl`.

| Field | Type | Description |
|-------|------|-------------|
| timestamp | string | ISO 8601 with timezone |
| session_id | string | Associated session (nullable for server-level events) |
| event_type | string | One of: `tool_call`, `approval`, `rejection`, `command_approval`, `command_rejection`, `session_start`, `session_terminate`, `session_interrupt` |
| tool_name | string | MCP tool name (for tool_call events) |
| parameters | object | Tool call parameters (for tool_call events) |
| result_summary | string | Brief result description |
| operator_id | string | Slack user ID (for approval/rejection events) |
| reason | string | Rejection reason (for rejection events) |
| request_id | string | Approval request ID (for approval/rejection events) |
| command | string | Terminal command (for command_approval/rejection events) |

**Rotation**: One file per calendar day. New file opened when date changes.

---

### CompiledWorkspacePolicy (in-memory only)

Pre-compiled form of `WorkspacePolicy` with regex patterns compiled into a `RegexSet`.

| Field | Type | Description |
|-------|------|-------------|
| raw | WorkspacePolicy | Original policy data |
| command_set | RegexSet | Pre-compiled command patterns |
| command_patterns | Vec\<String\> | Original pattern strings (for matched_rule reporting) |

**Lifecycle**:
- Created: When `PolicyLoader::load()` is called
- Replaced: On hot-reload when `PolicyWatcher` detects file change
- Cached: In `PolicyCache` (wired into `AppState`)

## Modified Entities

### Session (existing)

No schema changes. Behavioral change: sessions are marked `terminated` when SSE/HTTP stream disconnects (currently they remain `active` indefinitely).

### GlobalConfig (existing)

New field:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| slack_detail_level | string | "standard" | One of: minimal, standard, verbose |

### AppState (existing, in-memory)

New fields:

| Field | Type | Description |
|-------|------|-------------|
| policy_cache | Arc\<PolicyCache\> | Shared policy cache for hot-reload |
| audit_logger | Arc\<dyn AuditLogger\> | Audit log writer |

## DDL Additions (idempotent)

```sql
CREATE TABLE IF NOT EXISTS steering_message (
    id              TEXT PRIMARY KEY NOT NULL,
    session_id      TEXT NOT NULL,
    channel_id      TEXT,
    message         TEXT NOT NULL,
    source          TEXT NOT NULL CHECK(source IN ('slack','ipc')),
    created_at      TEXT NOT NULL,
    consumed        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS task_inbox (
    id              TEXT PRIMARY KEY NOT NULL,
    channel_id      TEXT,
    message         TEXT NOT NULL,
    source          TEXT NOT NULL CHECK(source IN ('slack','ipc')),
    created_at      TEXT NOT NULL,
    consumed        INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_steering_session_consumed
    ON steering_message(session_id, consumed);
CREATE INDEX IF NOT EXISTS idx_inbox_channel_consumed
    ON task_inbox(channel_id, consumed);
```

## Entity Relationships

```text
session 1──∞ steering_message    (session_id FK)
channel 1──∞ task_inbox          (channel_id scoping, no FK)
session 1──∞ audit_log_entry     (session_id in JSONL, no FK)
policy_cache 1──1 compiled_policy (in-memory)
```