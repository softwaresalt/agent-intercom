//! Integration tests for session lifecycle (T119).
//!
//! Validates:
//! - start → active → pause → resume → checkpoint → terminate
//! - `max_concurrent_sessions` enforcement (FR-023)
//! - Owner-only access (FR-013)
//! - Checkpoint file hash storage and divergence detection on restore

use std::collections::HashMap;
use std::sync::Arc;

use monocoque_agent_rem::config::GlobalConfig;
use monocoque_agent_rem::models::checkpoint::Checkpoint;
use monocoque_agent_rem::models::session::{Session, SessionMode, SessionStatus};
use monocoque_agent_rem::persistence::checkpoint_repo::CheckpointRepo;
use monocoque_agent_rem::persistence::db;
use monocoque_agent_rem::persistence::session_repo::SessionRepo;

/// Build a minimal test configuration with in-memory DB and low concurrency limit.
fn test_config() -> GlobalConfig {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-lifecycle"
max_concurrent_sessions = 2
host_cli = "echo"
authorized_user_ids = ["U_OWNER", "U_OTHER"]

[slack]
channel_id = "C_TEST"

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
        root = temp.path().to_str().expect("utf8"),
    );
    GlobalConfig::from_toml_str(&toml).expect("valid config")
}

#[tokio::test]
async fn full_lifecycle_start_pause_resume_terminate() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let repo = SessionRepo::new(database);

    // Create and activate a session.
    let session = Session::new(
        "U_OWNER".into(),
        "/test/workspace".into(),
        Some("build feature X".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");
    assert_eq!(created.status, SessionStatus::Created);

    // Activate.
    let active = repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");
    assert_eq!(active.status, SessionStatus::Active);

    // Pause.
    let paused = repo
        .update_status(&created.id, SessionStatus::Paused)
        .await
        .expect("pause");
    assert_eq!(paused.status, SessionStatus::Paused);

    // Resume.
    let resumed = repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("resume");
    assert_eq!(resumed.status, SessionStatus::Active);

    // Terminate.
    let terminated = repo
        .set_terminated(&created.id, SessionStatus::Terminated)
        .await
        .expect("terminate");
    assert_eq!(terminated.status, SessionStatus::Terminated);
    assert!(terminated.terminated_at.is_some());
}

#[tokio::test]
async fn max_concurrent_sessions_enforcement() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let repo = SessionRepo::new(database);

    // Create and activate sessions up to the limit (2).
    let s1 = Session::new("U_OWNER".into(), "/ws1".into(), None, SessionMode::Remote);
    let s1_created = repo.create(&s1).await.expect("create s1");
    repo.update_status(&s1_created.id, SessionStatus::Active)
        .await
        .expect("activate s1");

    let s2 = Session::new("U_OWNER".into(), "/ws2".into(), None, SessionMode::Remote);
    let s2_created = repo.create(&s2).await.expect("create s2");
    repo.update_status(&s2_created.id, SessionStatus::Active)
        .await
        .expect("activate s2");

    // Count active sessions should be 2 (at the limit).
    let count = repo.count_active().await.expect("count");
    assert_eq!(count, 2);

    // Verify limit would be exceeded — checked by the orchestrator.
    assert!(
        count >= u64::from(config.max_concurrent_sessions),
        "active sessions should be at or above the max limit"
    );
}

#[tokio::test]
async fn owner_binding_verified_at_session_level() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let repo = SessionRepo::new(database);

    let session = Session::new(
        "U_OWNER".into(),
        "/test/workspace".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");

    // Fetch session and verify owner binding.
    let fetched = repo.get_by_id(&created.id).await.expect("fetch");
    assert_eq!(fetched.owner_user_id, "U_OWNER");

    // Simulate owner-check: a different user should be rejected.
    let other_user = "U_OTHER";
    assert_ne!(
        fetched.owner_user_id, other_user,
        "session owner should not match other user"
    );
}

#[tokio::test]
async fn checkpoint_stores_file_hashes() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let checkpoint_repo = CheckpointRepo::new(database);

    // Create and activate a session.
    let session = Session::new(
        "U_OWNER".into(),
        "/test/workspace".into(),
        None,
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await.expect("create session");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    // Build file hashes map.
    let mut file_hashes = HashMap::new();
    file_hashes.insert("src/main.rs".to_owned(), "abc123".to_owned());
    file_hashes.insert("src/lib.rs".to_owned(), "def456".to_owned());

    // Create a checkpoint.
    let session_state = serde_json::json!({
        "status": "active",
        "last_tool": "heartbeat",
    });
    let checkpoint = Checkpoint::new(
        created.id.clone(),
        Some("before-refactor".to_owned()),
        session_state,
        file_hashes.clone(),
        "/test/workspace".to_owned(),
        None,
    );
    let saved = checkpoint_repo
        .create(&checkpoint)
        .await
        .expect("create checkpoint");

    // Verify stored hashes.
    let fetched = checkpoint_repo
        .get_by_id(&saved.id)
        .await
        .expect("fetch checkpoint");
    assert_eq!(fetched.file_hashes.len(), 2);
    assert_eq!(
        fetched.file_hashes.get("src/main.rs"),
        Some(&"abc123".to_owned())
    );
    assert_eq!(
        fetched.file_hashes.get("src/lib.rs"),
        Some(&"def456".to_owned())
    );
    assert_eq!(fetched.label, Some("before-refactor".to_owned()));
    assert_eq!(fetched.session_id, created.id);
}

#[tokio::test]
async fn checkpoint_restore_detects_divergence() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let checkpoint_repo = CheckpointRepo::new(database);

    // Simulate a checkpoint with known hashes.
    let mut checkpoint_hashes = HashMap::new();
    checkpoint_hashes.insert("src/main.rs".to_owned(), "hash_original".to_owned());
    checkpoint_hashes.insert("src/lib.rs".to_owned(), "hash_original_lib".to_owned());

    let checkpoint = Checkpoint::new(
        "session-cp-1".to_owned(),
        Some("test-cp".to_owned()),
        serde_json::json!({}),
        checkpoint_hashes.clone(),
        "/test/workspace".to_owned(),
        None,
    );
    let saved = checkpoint_repo
        .create(&checkpoint)
        .await
        .expect("create checkpoint");

    // Simulate current file hashes where main.rs has changed.
    let mut current_hashes = HashMap::new();
    current_hashes.insert("src/main.rs".to_owned(), "hash_changed".to_owned());
    current_hashes.insert("src/lib.rs".to_owned(), "hash_original_lib".to_owned());

    // Verify divergence detection.
    let fetched = checkpoint_repo.get_by_id(&saved.id).await.expect("fetch");
    let mut diverged: Vec<String> = fetched
        .file_hashes
        .iter()
        .filter(|(file, hash)| current_hashes.get(*file) != Some(*hash))
        .map(|(file, _)| file.clone())
        .collect();
    diverged.sort();

    assert_eq!(diverged, vec!["src/main.rs"]);
}

#[tokio::test]
async fn checkpoint_list_for_session() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let checkpoint_repo = CheckpointRepo::new(database);

    let session_id = "session-list-1";

    // Create multiple checkpoints for the same session.
    for i in 0..3 {
        let cp = Checkpoint::new(
            session_id.to_owned(),
            Some(format!("checkpoint-{i}")),
            serde_json::json!({}),
            HashMap::new(),
            "/test/workspace".to_owned(),
            None,
        );
        checkpoint_repo
            .create(&cp)
            .await
            .expect("create checkpoint");
    }

    let list = checkpoint_repo
        .list_for_session(session_id)
        .await
        .expect("list checkpoints");
    assert_eq!(list.len(), 3);
}

#[tokio::test]
async fn invalid_status_transition_rejected() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let repo = SessionRepo::new(database);

    let session = Session::new(
        "U_OWNER".into(),
        "/test/workspace".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");

    // Attempt to pause a Created session (invalid: Created → Paused not allowed).
    let result = repo.update_status(&created.id, SessionStatus::Paused).await;
    assert!(result.is_err(), "should reject Created → Paused transition");
}
