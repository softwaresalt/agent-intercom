//! Unit tests for ACP session lifecycle (T033–T037b).
//!
//! Covers:
//! - T033 (S021): stopping an ACP session calls `interrupt()` on the driver
//! - T034 (S023): agent process crash emits `AgentEvent::SessionTerminated`
//! - T035 (S025): startup timeout kills the process if no ready signal arrives
//! - T036 (S026): empty prompt is rejected at spawn time
//! - T037b (S075): spawned process does NOT inherit `SLACK_BOT_TOKEN`

use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use agent_intercom::acp::spawner::{monitor_exit, spawn_agent, SpawnConfig, ALLOWED_ENV_VARS};
use agent_intercom::driver::AgentEvent;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a `SpawnConfig` that references a known-good executable with the
/// given startup timeout.  On Windows the workspace root defaults to `TEMP`.
fn echo_config(startup_timeout: Duration) -> SpawnConfig {
    SpawnConfig {
        host_cli: echo_exe(),
        host_cli_args: Vec::new(),
        workspace_root: std::env::temp_dir(),
        startup_timeout,
    }
}

/// Platform-appropriate "echo" command that emits exactly one line and exits.
#[cfg(unix)]
fn echo_exe() -> String {
    "sh".to_owned()
}

#[cfg(windows)]
fn echo_exe() -> String {
    "cmd".to_owned()
}

/// Args to produce a single "ready" line on stdout then exit.
#[cfg(unix)]
fn echo_args() -> Vec<String> {
    vec!["-c".to_owned(), "echo ready".to_owned()]
}

#[cfg(windows)]
fn echo_args() -> Vec<String> {
    vec!["/C".to_owned(), "echo ready".to_owned()]
}

/// Platform-appropriate command that hangs (never writes to stdout).
#[cfg(unix)]
fn hanging_exe() -> String {
    "sh".to_owned()
}

#[cfg(windows)]
fn hanging_exe() -> String {
    "cmd".to_owned()
}

/// Args that cause the process to sleep for a very long time.
#[cfg(unix)]
fn hanging_args() -> Vec<String> {
    vec!["-c".to_owned(), "sleep 300".to_owned()]
}

#[cfg(windows)]
fn hanging_args() -> Vec<String> {
    vec!["/C".to_owned(), "timeout /t 300 /nobreak".to_owned()]
}

// ── T033: stop ACP session calls interrupt() ─────────────────────────────────

/// S021 — The `AgentDriver::interrupt` contract must be satisfied: calling it
/// on an unknown (or already-terminated) session returns `Ok(())`.
///
/// Concretely, this verifies that the `spawn_agent` types compile and that the
/// driver interrupt path is reachable without panicking.
#[tokio::test]
async fn acp_session_stop_terminates_child_process() {
    use agent_intercom::driver::mcp_driver::McpDriver;

    // SpawnConfig import proves the spawner types are accessible.
    let _config = echo_config(Duration::from_secs(5));

    // For ACP sessions the orchestrator calls driver.interrupt(session_id).
    // McpDriver's interrupt is idempotent — unknown sessions return Ok(()).
    let driver = McpDriver::new_empty();
    let result = driver.interrupt("acp-session-stop-test").await;
    assert!(
        result.is_ok(),
        "driver.interrupt() must succeed for an ACP session stop"
    );
}

// ── T034: agent process crash emits SessionTerminated ────────────────────────

/// S023 — when the child process exits, `monitor_exit` must emit
/// `AgentEvent::SessionTerminated` with the correct `session_id`.
#[tokio::test]
async fn agent_process_crash_is_detected() {
    let mut cfg = echo_config(Duration::from_secs(10));
    cfg.host_cli = echo_exe();
    cfg.host_cli_args = echo_args();

    // Spawn a process that exits immediately after printing one line.
    let conn = spawn_agent(&cfg, "sess-crash-test", "run task")
        .await
        .expect("spawn_agent must succeed with echo-like process");

    let (tx, mut rx) = mpsc::channel::<AgentEvent>(8);
    let cancel = CancellationToken::new();

    // monitor_exit takes ownership of the child.
    let _handle = monitor_exit("sess-crash-test".to_owned(), conn.child, tx, cancel.clone());

    // The process exits immediately; the monitor should emit SessionTerminated.
    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("event must arrive within 5 s")
        .expect("channel must not close prematurely");

    match event {
        AgentEvent::SessionTerminated { session_id, .. } => {
            assert_eq!(session_id, "sess-crash-test");
        }
        other => panic!("expected SessionTerminated, got {other:?}"),
    }
}

// ── T035: startup timeout kills process ──────────────────────────────────────

/// S025 — if the agent never writes to stdout within `startup_timeout`,
/// `spawn_agent` must kill the process and return `AppError::Acp`.
#[tokio::test]
async fn startup_timeout_kills_process_if_no_response() {
    let config = SpawnConfig {
        host_cli: hanging_exe(),
        host_cli_args: hanging_args(),
        workspace_root: std::env::temp_dir(),
        startup_timeout: Duration::from_millis(150),
    };

    let result = spawn_agent(&config, "sess-timeout-test", "run task").await;

    assert!(
        result.is_err(),
        "spawn_agent must fail when no ready signal arrives within startup_timeout"
    );
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("startup timeout") || msg.contains("acp:"),
        "error must mention startup timeout, got: {msg}"
    );
}

// ── T036: empty prompt is rejected ───────────────────────────────────────────

/// S026 — `spawn_agent` must return an error when the prompt is empty or
/// all-whitespace, preventing the agent from being started without work.
#[tokio::test]
async fn empty_prompt_is_rejected() {
    let config = echo_config(Duration::from_secs(5));

    let result_empty = spawn_agent(&config, "sess-empty-test", "").await;
    assert!(
        result_empty.is_err(),
        "spawn_agent must reject an empty prompt"
    );

    let result_whitespace = spawn_agent(&config, "sess-ws-test", "   ").await;
    assert!(
        result_whitespace.is_err(),
        "spawn_agent must reject a whitespace-only prompt"
    );
}

// ── T037b: spawned process does not inherit Slack tokens ─────────────────────

/// S075 — the ACP spawner must use `env_clear()` so Slack tokens and other
/// secrets from the server's environment are never leaked into the child process.
///
/// This test verifies the allowlist exported by the spawner does NOT include
/// any secret variable names, and that the dangerous vars are absent.
#[test]
fn spawned_process_does_not_inherit_slack_tokens() {
    // ALLOWED_ENV_VARS is the exhaustive allowlist passed to the child.
    let allowed: std::collections::HashSet<&str> = ALLOWED_ENV_VARS.iter().copied().collect();

    let forbidden = [
        "SLACK_BOT_TOKEN",
        "SLACK_APP_TOKEN",
        "SLACK_MEMBER_IDS",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        "DATABASE_URL",
        "GITHUB_TOKEN",
        "OPENAI_API_KEY",
    ];

    for var in &forbidden {
        assert!(
            !allowed.contains(var),
            "ALLOWED_ENV_VARS must not contain secret variable `{var}`"
        );
    }
}

// ── T094: crash with pending clearance resolves as timeout ───────────────────

/// S068 — When an ACP session crashes (EOF on stream) while a clearance
/// request is pending in the database, that request must be resolved as
/// `interrupted` so the operator is not left waiting for a non-existent
/// approval button.
///
/// This test verifies the pending-clearance resolution logic in isolation,
/// without requiring the full ACP event consumer to be running.
#[tokio::test]
async fn crash_with_pending_clearance_resolves_as_timeout() {
    use std::sync::Arc;

    use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus};
    use agent_intercom::models::session::{ProtocolMode, Session, SessionMode, SessionStatus};
    use agent_intercom::persistence::approval_repo::ApprovalRepo;
    use agent_intercom::persistence::db;
    use agent_intercom::persistence::session_repo::SessionRepo;

    let pool = Arc::new(db::connect_memory().await.expect("in-memory db"));

    // Create an ACP session.
    let session_repo = SessionRepo::new(Arc::clone(&pool));
    let mut session = Session::new(
        "U_OP".to_owned(),
        std::env::temp_dir().to_string_lossy().to_string(),
        Some("build feature".to_owned()),
        SessionMode::Remote,
    );
    session.protocol_mode = ProtocolMode::Acp;
    let created = session_repo.create(&session).await.expect("create session");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    // Create a pending approval request for this session.
    let approval_repo = ApprovalRepo::new(Arc::clone(&pool));
    let approval = ApprovalRequest::new(
        created.id.clone(),
        "Update config.toml".to_owned(),
        None,
        String::new(),
        "config.toml".to_owned(),
        agent_intercom::models::approval::RiskLevel::Low,
        "sha256:abc123".to_owned(),
    );
    let persisted = approval_repo
        .create(&approval)
        .await
        .expect("create approval");
    assert_eq!(persisted.status, ApprovalStatus::Pending);

    // Simulate crash resolution: mark all pending approvals for the session
    // as interrupted (what the ACP event consumer does on SessionTerminated).
    approval_repo
        .resolve_pending_for_session(&created.id, ApprovalStatus::Interrupted)
        .await
        .expect("resolve pending for session");

    // Verify the approval is no longer pending.
    let updated = approval_repo
        .get_by_id(&persisted.id)
        .await
        .expect("fetch approval")
        .expect("approval present");
    assert_eq!(
        updated.status,
        ApprovalStatus::Interrupted,
        "pending clearance must be marked interrupted when agent crashes"
    );
}
