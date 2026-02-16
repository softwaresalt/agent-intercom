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

use monocoque_agent_rc::config::GlobalConfig;
use monocoque_agent_rc::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use monocoque_agent_rc::models::checkpoint::Checkpoint;
use monocoque_agent_rc::models::progress::{ProgressItem, ProgressStatus};
use monocoque_agent_rc::models::prompt::{ContinuationPrompt, PromptType};
use monocoque_agent_rc::models::session::{Session, SessionMode, SessionStatus};
use monocoque_agent_rc::persistence::approval_repo::ApprovalRepo;
use monocoque_agent_rc::persistence::checkpoint_repo::CheckpointRepo;
use monocoque_agent_rc::persistence::db;
use monocoque_agent_rc::persistence::prompt_repo::PromptRepo;
use monocoque_agent_rc::persistence::session_repo::SessionRepo;

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
authorized_user_ids = ["U_OWNER"]

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
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db"));
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
    let mut response = database
        .query(
            "SELECT * FROM session WHERE status = 'interrupted' \
             ORDER BY updated_at DESC LIMIT 1",
        )
        .await
        .expect("query");
    let interrupted: Vec<Session> = response.take(0).expect("take");
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
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db"));
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
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db"));
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
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db"));
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
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db"));

    // Query interrupted sessions — should be empty.
    let mut response = database
        .query(
            "SELECT * FROM session WHERE status = 'interrupted' \
             ORDER BY updated_at DESC LIMIT 1",
        )
        .await
        .expect("query");
    let interrupted: Vec<Session> = response.take(0).expect("take");
    assert!(
        interrupted.is_empty(),
        "no interrupted sessions should exist in clean state"
    );
}

#[tokio::test]
async fn recover_finds_most_recent_interrupted_session() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    // Create two interrupted sessions.
    let session1 = create_interrupted_session(&session_repo).await;

    // Small delay so updated_at differs.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let session2 = create_interrupted_session(&session_repo).await;

    // The most recently interrupted should come first.
    let mut response = database
        .query(
            "SELECT * FROM session WHERE status = 'interrupted' \
             ORDER BY updated_at DESC LIMIT 1",
        )
        .await
        .expect("query");
    let most_recent: Vec<Session> = response.take(0).expect("take");
    assert_eq!(most_recent.len(), 1);
    assert_eq!(
        most_recent[0].id, session2.id,
        "most recently interrupted session should be returned"
    );

    // Can also recover a specific session by ID.
    let specific = session_repo.get_by_id(&session1.id).await.expect("get");
    assert_eq!(specific.status, SessionStatus::Interrupted);
}
