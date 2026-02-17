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
