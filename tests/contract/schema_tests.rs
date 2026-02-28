//! Contract tests for `SQLite` schema bootstrap (T004, T104-migrated).
//!
//! Verify that table creation and column definitions match the
//! contracts/schema.sql.md specification.

use agent_intercom::persistence::db;

/// T004: File-backed `connect()` creates all 5 tables with correct columns.
#[tokio::test]
async fn file_backed_connect_creates_five_tables() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.db");
    let pool = db::connect(db_path.to_str().expect("utf8"))
        .await
        .expect("file-backed connect should succeed");

    let tables = [
        "session",
        "approval_request",
        "checkpoint",
        "continuation_prompt",
        "stall_alert",
    ];

    for table in tables {
        let query = format!("SELECT COUNT(*) AS cnt FROM {table}");
        let row: (i64,) = sqlx::query_as(&query)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("table '{table}' should be queryable: {e}"));
        assert_eq!(row.0, 0, "table '{table}' should start empty");
    }
}

/// T004 addendum: verify session table has expected columns.
#[tokio::test]
async fn session_table_has_expected_columns() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("cols.db");
    let pool = db::connect(db_path.to_str().expect("utf8"))
        .await
        .expect("connect");

    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('session') ORDER BY cid")
            .fetch_all(&pool)
            .await
            .expect("pragma_table_info");

    let column_names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    let expected = [
        "id",
        "owner_user_id",
        "workspace_root",
        "status",
        "prompt",
        "mode",
        "created_at",
        "updated_at",
        "terminated_at",
        "last_tool",
        "nudge_count",
        "stall_paused",
        "progress_snapshot",
        "protocol_mode",
        "channel_id",
        "thread_ts",
        "connectivity_status",
        "last_activity_at",
        "restart_of",
    ];

    assert_eq!(
        column_names, expected,
        "session table columns should match schema.sql.md"
    );
}

/// T004 addendum: verify `approval_request` table has expected columns.
#[tokio::test]
async fn approval_request_table_has_expected_columns() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("ar.db");
    let pool = db::connect(db_path.to_str().expect("utf8"))
        .await
        .expect("connect");

    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('approval_request') ORDER BY cid")
            .fetch_all(&pool)
            .await
            .expect("pragma_table_info");

    let column_names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    let expected = [
        "id",
        "session_id",
        "title",
        "description",
        "diff_content",
        "file_path",
        "risk_level",
        "status",
        "original_hash",
        "slack_ts",
        "created_at",
        "consumed_at",
    ];

    assert_eq!(
        column_names, expected,
        "approval_request table columns should match schema.sql.md"
    );
}
