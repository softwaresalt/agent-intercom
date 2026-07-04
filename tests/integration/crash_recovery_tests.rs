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
use agent_intercom::models::steering::{SteeringMessage, SteeringSource};
use agent_intercom::persistence::approval_repo::ApprovalRepo;
use agent_intercom::persistence::checkpoint_repo::CheckpointRepo;
use agent_intercom::persistence::db;
use agent_intercom::persistence::prompt_repo::PromptRepo;
use agent_intercom::persistence::session_repo::SessionRepo;
use agent_intercom::persistence::steering_repo::SteeringRepo;

use agent_intercom::orchestrator::spawner;

/// Build a config with a cross-platform `host_cli` that spawns and exits
/// immediately (`cmd /c echo` on Windows, `echo` on Unix). `root` must be an
/// existing directory because `from_toml_str` canonicalizes it.
fn respawn_config(root: &str) -> GlobalConfig {
    let host_cli_line = if cfg!(windows) {
        "host_cli = \"cmd\"\nhost_cli_args = [\"/c\", \"echo\"]"
    } else {
        "host_cli = \"echo\"\nhost_cli_args = []"
    };
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-respawn"
max_concurrent_sessions = 3
{host_cli_line}

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
    );
    GlobalConfig::from_toml_str(&toml).expect("valid config")
}

/// Build a config whose `host_cli` points at a binary that does not exist, so
/// `respawn_session`'s child spawn deterministically fails.
fn respawn_config_bad_cli(root: &str) -> GlobalConfig {
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-respawn-bad"
max_concurrent_sessions = 3
host_cli = "agent-intercom-nonexistent-binary-xyz"
host_cli_args = []

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
    );
    GlobalConfig::from_toml_str(&toml).expect("valid config")
}

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

/// F.3-T1: an induced agent crash triggers respawn + session rebind. The
/// crashed session becomes `Interrupted`, and a new `Active` session is created
/// that is linked to the predecessor via `restart_of`, carrying the workspace,
/// owner, prompt, Slack thread, and ACP `agent_session_id` forward (rebind).
#[tokio::test]
async fn respawn_creates_resumed_session_linked_to_crashed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().to_str().expect("utf8").to_string();
    let config = respawn_config(&workspace);

    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    // Create an active session that will "crash".
    let mut session = Session::new(
        "U_OWNER".into(),
        workspace.clone(),
        Some("build feature X".into()),
        SessionMode::Remote,
    );
    session.channel_id = Some("C_TEST".into());
    session.thread_ts = Some("1700000000.000100".into());
    session.agent_session_id = Some("acp-sess-123".into());
    let created = session_repo.create(&session).await.expect("create");
    let active = session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    // Induce crash recovery.
    let (resumed, _child) =
        spawner::respawn_session(&active, &config, &session_repo, &database, config.http_port)
            .await
            .expect("respawn");

    // The crashed session is now Interrupted.
    let crashed = session_repo
        .get_by_id(&active.id)
        .await
        .expect("get")
        .expect("crashed session exists");
    assert_eq!(crashed.status, SessionStatus::Interrupted);
    assert!(crashed.terminated_at.is_some());

    // The resumed session is a distinct, Active restart linked to the crashed one.
    assert_ne!(resumed.id, active.id, "resume must be a new session record");
    assert_eq!(resumed.status, SessionStatus::Active);
    assert_eq!(resumed.restart_of.as_deref(), Some(active.id.as_str()));

    // Session rebind: identity + workspace + prompt + ACP session id carried forward.
    assert_eq!(resumed.owner_user_id, active.owner_user_id);
    assert_eq!(resumed.workspace_root, active.workspace_root);
    assert_eq!(resumed.prompt, active.prompt);
    assert_eq!(resumed.agent_session_id.as_deref(), Some("acp-sess-123"));
    assert_eq!(resumed.channel_id.as_deref(), Some("C_TEST"));
    assert_eq!(resumed.thread_ts.as_deref(), Some("1700000000.000100"));

    // The resumed session is persisted and retrievable.
    let persisted = session_repo
        .get_by_id(&resumed.id)
        .await
        .expect("get")
        .expect("resumed session exists");
    assert_eq!(persisted.status, SessionStatus::Active);
    assert_eq!(persisted.restart_of.as_deref(), Some(active.id.as_str()));
}

/// F.3-T4 (characterization): the resume path consumes F.3-T2 + F.3-T3
/// persistence. After a respawn, the crashed session's pending steering
/// messages, clearances, and prompts are rebound to the resumed session with
/// correlation ids preserved, and none remain on the crashed session.
#[tokio::test]
async fn respawn_rebinds_pending_steering_clearance_and_prompt() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().to_str().expect("utf8").to_string();
    let config = respawn_config(&workspace);

    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let steering_repo = SteeringRepo::new(Arc::clone(&database));
    let approval_repo = ApprovalRepo::new(Arc::clone(&database));
    let prompt_repo = PromptRepo::new(Arc::clone(&database));

    // An active session mid-task with pending steering, clearance, and prompt.
    let mut session = Session::new(
        "U_OWNER".into(),
        workspace.clone(),
        Some("mid-task work".into()),
        SessionMode::Remote,
    );
    session.channel_id = Some("C_TEST".into());
    let created = session_repo.create(&session).await.expect("create");
    let active = session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let steer = SteeringMessage::new(
        active.id.clone(),
        Some("C_TEST".into()),
        "refocus on the failing test".into(),
        SteeringSource::Slack,
    );
    steering_repo.insert(&steer).await.expect("insert steering");

    let approval = ApprovalRequest::new(
        active.id.clone(),
        "Apply refactor".into(),
        None,
        "--- a/src/x.rs\n+++ b/src/x.rs".into(),
        "src/x.rs".into(),
        RiskLevel::Low,
        "hash-x".into(),
    );
    let approval_id = approval.id.clone();
    approval_repo
        .create(&approval)
        .await
        .expect("create approval");

    let prompt = ContinuationPrompt::new(
        active.id.clone(),
        "Continue with the plan?".into(),
        PromptType::Continuation,
        Some(60),
        Some(2),
    );
    let prompt_id = prompt.id.clone();
    prompt_repo.create(&prompt).await.expect("create prompt");

    // Induce crash recovery.
    let (resumed, _child) =
        spawner::respawn_session(&active, &config, &session_repo, &database, config.http_port)
            .await
            .expect("respawn");

    // Steering message is now owned by the resumed session (origin preserved).
    assert!(steering_repo
        .fetch_unconsumed(&active.id)
        .await
        .expect("fetch crashed steering")
        .is_empty());
    let resumed_steering = steering_repo
        .fetch_unconsumed(&resumed.id)
        .await
        .expect("fetch resumed steering");
    assert_eq!(resumed_steering.len(), 1);
    assert_eq!(resumed_steering[0].message, "refocus on the failing test");
    assert_eq!(
        resumed_steering[0].origin_session_id.as_deref(),
        Some(active.id.as_str())
    );

    // Pending clearance is rebound with its correlation id preserved.
    assert!(approval_repo
        .get_pending_for_session(&active.id)
        .await
        .expect("fetch crashed approval")
        .is_none());
    let resumed_approval = approval_repo
        .get_pending_for_session(&resumed.id)
        .await
        .expect("fetch resumed approval")
        .expect("pending approval present");
    assert_eq!(resumed_approval.id, approval_id);

    // Pending prompt is rebound with its correlation id preserved.
    assert!(prompt_repo
        .get_pending_for_session(&active.id)
        .await
        .expect("fetch crashed prompt")
        .is_none());
    let resumed_prompt = prompt_repo
        .get_pending_for_session(&resumed.id)
        .await
        .expect("fetch resumed prompt")
        .expect("pending prompt present");
    assert_eq!(resumed_prompt.id, prompt_id);
}

/// F.3-T4 (regression): if the replacement process fails to spawn, the crashed
/// session's pending state is NOT moved — it stays on the crashed session so it
/// can be retried or recovered, rather than being orphaned onto a session whose
/// process never started.
#[tokio::test]
async fn respawn_spawn_failure_leaves_pending_state_on_crashed_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().to_str().expect("utf8").to_string();
    let config = respawn_config_bad_cli(&workspace);

    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let steering_repo = SteeringRepo::new(Arc::clone(&database));
    let approval_repo = ApprovalRepo::new(Arc::clone(&database));
    let prompt_repo = PromptRepo::new(Arc::clone(&database));

    let mut session = Session::new(
        "U_OWNER".into(),
        workspace.clone(),
        Some("mid-task work".into()),
        SessionMode::Remote,
    );
    session.channel_id = Some("C_TEST".into());
    let created = session_repo.create(&session).await.expect("create");
    let active = session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    steering_repo
        .insert(&SteeringMessage::new(
            active.id.clone(),
            Some("C_TEST".into()),
            "pending steer".into(),
            SteeringSource::Slack,
        ))
        .await
        .expect("insert steering");
    approval_repo
        .create(&ApprovalRequest::new(
            active.id.clone(),
            "Apply refactor".into(),
            None,
            "--- a/x\n+++ b/x".into(),
            "x".into(),
            RiskLevel::Low,
            "hash".into(),
        ))
        .await
        .expect("create approval");
    prompt_repo
        .create(&ContinuationPrompt::new(
            active.id.clone(),
            "Continue?".into(),
            PromptType::Continuation,
            Some(60),
            Some(2),
        ))
        .await
        .expect("create prompt");

    // Respawn fails because the host CLI binary does not exist.
    let result =
        spawner::respawn_session(&active, &config, &session_repo, &database, config.http_port)
            .await;
    assert!(
        result.is_err(),
        "respawn must fail when the host CLI is missing"
    );

    // The crashed session's pending state was NOT moved — it remains recoverable.
    let steering = steering_repo
        .fetch_unconsumed(&active.id)
        .await
        .expect("fetch steering");
    assert_eq!(
        steering.len(),
        1,
        "steering must stay on the crashed session"
    );
    assert!(approval_repo
        .get_pending_for_session(&active.id)
        .await
        .expect("fetch approval")
        .is_some());
    assert!(prompt_repo
        .get_pending_for_session(&active.id)
        .await
        .expect("fetch prompt")
        .is_some());
}
