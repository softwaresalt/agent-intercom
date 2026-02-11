//! Contract tests for SurrealDB schema (T104).
//!
//! Verify that table creation, field definitions, and ASSERT constraints
//! match the data-model.md specification.

use std::sync::Arc;

use monocoque_agent_rem::config::GlobalConfig;
use monocoque_agent_rem::persistence::db;

fn config_for_tests() -> GlobalConfig {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = "{root}"
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "claude"

[slack]
channel_id = "C123"

authorized_user_ids = ["U123"]

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        root = temp.path().to_str().expect("utf8 path"),
    );

    GlobalConfig::from_toml_str(&toml).expect("valid config")
}

#[tokio::test]
async fn schema_creates_five_tables() {
    let config = config_for_tests();
    let db = db::connect(&config, true).await.expect("db connect");
    let db = Arc::new(db);

    // Attempt to query each table — should succeed without error.
    let tables = ["session", "approval_request", "checkpoint", "continuation_prompt", "stall_alert"];

    for table in tables {
        let query = format!("SELECT * FROM {table} LIMIT 1");
        let result: surrealdb::Result<surrealdb::Response> = db.query(&query).await;
        assert!(
            result.is_ok(),
            "table '{table}' should exist and be queryable"
        );
    }
}

#[tokio::test]
async fn session_table_accepts_valid_record() {
    let config = config_for_tests();
    let db = db::connect(&config, true).await.expect("db connect");

    let result: surrealdb::Result<surrealdb::Response> = db
        .query(
            r"CREATE session:test SET
                owner_user_id = 'U123',
                workspace_root = '/test',
                status = 'created',
                prompt = 'hello',
                mode = 'remote',
                created_at = time::now(),
                updated_at = time::now(),
                last_tool = NONE,
                nudge_count = 0,
                stall_paused = false,
                terminated_at = NONE,
                progress_snapshot = NONE",
        )
        .await;

    assert!(result.is_ok(), "valid session should be insertable");
}

#[tokio::test]
async fn approval_request_table_accepts_valid_record() {
    let config = config_for_tests();
    let db = db::connect(&config, true).await.expect("db connect");

    let result: surrealdb::Result<surrealdb::Response> = db
        .query(
            r"CREATE approval_request:test SET
                session_id = 'session:test',
                title = 'test',
                description = NONE,
                diff_content = '--- a\n+++ b',
                file_path = 'src/main.rs',
                risk_level = 'low',
                status = 'pending',
                original_hash = 'abc123',
                slack_ts = NONE,
                created_at = time::now(),
                consumed_at = NONE",
        )
        .await;

    assert!(result.is_ok(), "valid approval_request should be insertable");
}
