//! `SQLite` schema bootstrap logic.
//!
//! All table definitions use `CREATE TABLE IF NOT EXISTS` — safe to
//! re-run on every server startup. Produces a convergent result.

use sqlx::SqlitePool;

use crate::Result;

/// Apply all table definitions to the connected `SQLite` database.
///
/// Creates all five tables idempotently. Safe to call on every startup.
///
/// # Errors
///
/// Returns `AppError::Db` if any DDL statement fails.
pub async fn bootstrap_schema(pool: &SqlitePool) -> Result<()> {
    let ddl = r"
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

CREATE INDEX IF NOT EXISTS idx_approval_session ON approval_request(session_id);
CREATE INDEX IF NOT EXISTS idx_checkpoint_session ON checkpoint(session_id);
CREATE INDEX IF NOT EXISTS idx_prompt_session ON continuation_prompt(session_id);
CREATE INDEX IF NOT EXISTS idx_stall_session ON stall_alert(session_id);
";

    sqlx::raw_sql(ddl).execute(pool).await?;
    Ok(())
}
