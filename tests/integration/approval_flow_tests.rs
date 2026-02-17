//! Integration tests for the approval flow (T106).
//!
//! Validates the end-to-end flow:
//! 1. Submit approval request → DB record created
//! 2. Simulate Accept → oneshot resolves with `approved`
//! 3. DB record updated to `Approved`
//!
//! Also tests Reject and timeout paths.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;

use monocoque_agent_rc::config::GlobalConfig;
use monocoque_agent_rc::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use monocoque_agent_rc::persistence::approval_repo::ApprovalRepo;
use monocoque_agent_rc::persistence::db;

/// Build a minimal test configuration with in-memory DB.
fn test_config() -> GlobalConfig {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-approval"
max_concurrent_sessions = 3
host_cli = "echo"
authorized_user_ids = ["U_OWNER"]

[slack]
channel_id = "C_TEST"

[timeouts]
approval_seconds = 2
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

/// Create a sample approval request for testing.
fn sample_request(session_id: &str) -> ApprovalRequest {
    ApprovalRequest::new(
        session_id.to_owned(),
        "Add auth middleware".to_owned(),
        Some("Detailed description".to_owned()),
        "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new".to_owned(),
        "src/main.rs".to_owned(),
        RiskLevel::Low,
        "abc123def456".to_owned(),
    )
}

#[tokio::test]
async fn approval_flow_creates_db_record() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = ApprovalRepo::new(database);

    let request = sample_request("session-1");
    let request_id = request.id.clone();

    let created = repo.create(&request).await.expect("create should succeed");
    assert_eq!(created.id, request_id);
    assert_eq!(created.status, ApprovalStatus::Pending);
    assert_eq!(created.title, "Add auth middleware");
    assert_eq!(created.file_path, "src/main.rs");
    assert_eq!(created.risk_level, RiskLevel::Low);
}

#[tokio::test]
async fn approval_flow_accept_updates_status() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = ApprovalRepo::new(database);

    let request = sample_request("session-2");
    let request_id = request.id.clone();
    repo.create(&request).await.expect("create");

    // Simulate Accept — update status to Approved.
    repo.update_status(&request_id, ApprovalStatus::Approved)
        .await
        .expect("approve");

    // Verify DB state.
    let fetched = repo
        .get_by_id(&request_id)
        .await
        .expect("fetch")
        .expect("approval should exist");
    assert_eq!(fetched.status, ApprovalStatus::Approved);
}

#[tokio::test]
async fn approval_flow_reject_updates_status() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = ApprovalRepo::new(database);

    let request = sample_request("session-3");
    let request_id = request.id.clone();
    repo.create(&request).await.expect("create");

    repo.update_status(&request_id, ApprovalStatus::Rejected)
        .await
        .expect("reject");

    let rejected = repo
        .get_by_id(&request_id)
        .await
        .expect("fetch")
        .expect("approval should exist");
    assert_eq!(rejected.status, ApprovalStatus::Rejected);
}

#[tokio::test]
async fn approval_flow_oneshot_resolves_on_accept() {
    // Simulate the blocking pattern: ask_approval blocks on a oneshot,
    // and the interaction callback resolves it.
    let (tx, rx) = oneshot::channel::<(&str, Option<String>)>();

    // Simulate the interaction callback resolving with "approved".
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(("approved", None));
    });

    let (status, _reason) = rx.await.expect("oneshot should resolve");
    assert_eq!(status, "approved");
}

#[tokio::test]
async fn approval_flow_oneshot_resolves_on_reject_with_reason() {
    let (tx, rx) = oneshot::channel::<(&str, Option<String>)>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(("rejected", Some("Needs more tests".to_owned())));
    });

    let (status, reason) = rx.await.expect("oneshot should resolve");
    assert_eq!(status, "rejected");
    assert_eq!(reason.as_deref(), Some("Needs more tests"));
}

#[tokio::test]
async fn approval_flow_timeout_expires_request() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = ApprovalRepo::new(database);

    let request = sample_request("session-timeout");
    let request_id = request.id.clone();
    repo.create(&request).await.expect("create");

    let (tx, rx) = oneshot::channel::<(&str, Option<String>)>();

    // Simulate timeout — no one resolves the sender.
    let timeout_result = tokio::time::timeout(Duration::from_millis(200), rx).await;
    assert!(timeout_result.is_err(), "should timeout without response");

    // On timeout, mark the request as expired.
    repo.update_status(&request_id, ApprovalStatus::Expired)
        .await
        .expect("expire");

    let expired = repo
        .get_by_id(&request_id)
        .await
        .expect("fetch")
        .expect("approval should exist");
    assert_eq!(expired.status, ApprovalStatus::Expired);

    // Sender dropped without sending.
    drop(tx);
}

#[tokio::test]
async fn approval_flow_pending_for_session_query() {
    let _config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = ApprovalRepo::new(database);

    let request = sample_request("session-pending");
    repo.create(&request).await.expect("create");

    let pending = repo
        .get_pending_for_session("session-pending")
        .await
        .expect("query pending");
    assert!(pending.is_some());
    assert_eq!(
        pending.as_ref().map(|r| r.status),
        Some(ApprovalStatus::Pending)
    );
}
