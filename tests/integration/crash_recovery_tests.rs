//! Integration tests for crash recovery (T123).
//!
//! Validates:
//! - Create session with pending approval → simulate shutdown (mark Interrupted) →
//!   invoke `recover_state` → verify pending request returned with original data
//! - Create session with pending prompt → mark Interrupted → recover → verify
//! - Create session with progress snapshot → mark Interrupted → recover →
//!   verify progress snapshot present (SC-004)
//! - Clean state with no interrupted sessions returns `clean`

use std::sync::Arc;

use agent_intercom::config::GlobalConfig;
use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use agent_intercom::models::checkpoint::Checkpoint;
use agent_intercom::models::progress::{ProgressItem, ProgressStatus};
use agent_intercom::models::prompt::{ContinuationPrompt, PromptType};
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::approval_repo::ApprovalRepo;
use agent_intercom::persistence::checkpoint_repo::CheckpointRepo;
use agent_intercom::persistence::db;
use agent_intercom::persistence::prompt_repo::PromptRepo;
use agent_intercom::persistence::session_repo::SessionRepo;

/// Build a minimal test configuration.
fn test_config() -> GlobalConfig {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-recovery"
max_concurrent_sessions = 3
host_cli = "echo"

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

/// Helper: create an active session, mark it interrupted (simulating crash).
async fn create_interrupted_session(repo: &SessionRepo) -> Session {
    let session = Session::new(
        "U_OWNER".into(),
        "/test/workspace".into(),
        Some("build feature X".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create session");
    let active = repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");
    repo.set_terminated(&active.id, SessionStatus::Interrupted)
        .await
        .expect("interrupt")
}

#[tokio::test]
async fn recover_interrupted_session_with_pending_approval() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let approval_repo = ApprovalRepo::new(Arc::clone(&database));

    // Create interrupted session with a pending approval.
    let session = create_interrupted_session(&session_repo).await;
    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Add auth module".into(),
        Some("Implements JWT auth".into()),
        "--- a/src/auth.rs\n+++ b/src/auth.rs".into(),
        "src/auth.rs".into(),
        RiskLevel::Low,
        "abc123hash".into(),
    );
    approval_repo
        .create(&approval)
        .await
        .expect("create approval");

    // Query interrupted sessions — should find our session.
    let interrupted = session_repo
        .list_interrupted()
        .await
        .expect("list interrupted");
    assert_eq!(interrupted.len(), 1);
    assert_eq!(interrupted[0].id, session.id);

    // Get pending approvals for that session — should find Interrupted ones
    // but the one we created should still be Pending (not yet marked by shutdown).
    let pending = approval_repo
        .get_pending_for_session(&session.id)
        .await
        .expect("get pending");
    assert!(pending.is_some(), "pending approval should exist");
    let pending = pending.expect("present");
    assert_eq!(pending.title, "Add auth module");
    assert_eq!(pending.status, ApprovalStatus::Pending);
}

#[tokio::test]
async fn recover_interrupted_session_with_pending_prompt() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let prompt_repo = PromptRepo::new(Arc::clone(&database));

    let session = create_interrupted_session(&session_repo).await;

    let prompt = ContinuationPrompt::new(
        session.id.clone(),
        "Should I continue with the current task?".into(),
        PromptType::Continuation,
        Some(120),
        Some(5),
    );
    prompt_repo.create(&prompt).await.expect("create prompt");

    // Query pending prompts for interrupted session.
    let pending = prompt_repo
        .get_pending_for_session(&session.id)
        .await
        .expect("get pending");
    assert!(pending.is_some(), "pending prompt should exist");
    let pending = pending.expect("present");
    assert_eq!(
        pending.prompt_text,
        "Should I continue with the current task?"
    );
    assert!(
        pending.decision.is_none(),
        "prompt should have no decision yet"
    );
}

#[tokio::test]
async fn recover_session_includes_progress_snapshot() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    // Create a session with a progress snapshot.
    let mut session = Session::new(
        "U_OWNER".into(),
        "/test/workspace".into(),
        Some("implement feature".into()),
        SessionMode::Remote,
    );
    session.progress_snapshot = Some(vec![
        ProgressItem {
            label: "Parse config".into(),
            status: ProgressStatus::Done,
        },
        ProgressItem {
            label: "Build schema".into(),
            status: ProgressStatus::InProgress,
        },
        ProgressItem {
            label: "Run tests".into(),
            status: ProgressStatus::Pending,
        },
    ]);

    let created = session_repo.create(&session).await.expect("create");
    let active = session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");
    let interrupted = session_repo
        .set_terminated(&active.id, SessionStatus::Interrupted)
        .await
        .expect("interrupt");

    // Progress snapshot should be preserved through interruption.
    assert!(
        interrupted.progress_snapshot.is_some(),
        "progress snapshot should survive interruption"
    );
    let snapshot = interrupted.progress_snapshot.expect("present");
    assert_eq!(snapshot.len(), 3);
    assert_eq!(snapshot[0].label, "Parse config");
    assert_eq!(snapshot[0].status, ProgressStatus::Done);
    assert_eq!(snapshot[1].label, "Build schema");
    assert_eq!(snapshot[1].status, ProgressStatus::InProgress);
}

#[tokio::test]
async fn recover_session_includes_last_checkpoint() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&database));

    let session = create_interrupted_session(&session_repo).await;

    // Create a checkpoint for the session.
    let checkpoint = Checkpoint::new(
        session.id.clone(),
        Some("before-refactor".into()),
        serde_json::json!({"status": "active"}),
        std::collections::HashMap::from([
            ("src/main.rs".into(), "hash1".into()),
            ("src/lib.rs".into(), "hash2".into()),
        ]),
        "/test/workspace".into(),
        None,
    );
    checkpoint_repo
        .create(&checkpoint)
        .await
        .expect("create checkpoint");

    // Query last checkpoint for session.
    let checkpoints = checkpoint_repo
        .list_for_session(&session.id)
        .await
        .expect("list checkpoints");
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].label.as_deref(), Some("before-refactor"));
}

#[tokio::test]
async fn clean_state_when_no_interrupted_sessions() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    // Query interrupted sessions — should be empty.
    let interrupted = session_repo
        .list_interrupted()
        .await
        .expect("list interrupted");
    assert!(
        interrupted.is_empty(),
        "no interrupted sessions should exist in clean state"
    );
}

#[tokio::test]
async fn recover_finds_most_recent_interrupted_session() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    // Create two interrupted sessions.
    let session1 = create_interrupted_session(&session_repo).await;

    // Small delay so updated_at differs.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let session2 = create_interrupted_session(&session_repo).await;

    // The most recently interrupted should come first.
    let most_recent = session_repo
        .get_most_recent_interrupted()
        .await
        .expect("get most recent interrupted");
    assert!(most_recent.is_some(), "should find an interrupted session");
    assert_eq!(
        most_recent.as_ref().unwrap().id,
        session2.id,
        "most recently interrupted session should be returned"
    );

    // Can also recover a specific session by ID.
    let specific = session_repo
        .get_by_id(&session1.id)
        .await
        .expect("get")
        .expect("session should exist");
    assert_eq!(specific.status, SessionStatus::Interrupted);
}
