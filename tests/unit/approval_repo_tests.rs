//! Unit tests for `ApprovalRepo` CRUD operations (T019).
//!
//! Validates:
//! - Create approval request and verify all fields persisted
//! - `get_by_id` returns `None` for missing records
//! - `update_status` transitions and `get_pending_for_session`
//! - `mark_consumed` sets `consumed_at` and enforces single-use
//! - Double-consume returns `AlreadyConsumed` error

use std::sync::Arc;

use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use agent_intercom::persistence::{approval_repo::ApprovalRepo, db};

fn sample_request(session_id: &str) -> ApprovalRequest {
    ApprovalRequest::new(
        session_id.to_owned(),
        "Add endpoint".to_owned(),
        Some("Adds a REST endpoint".to_owned()),
        "--- a/src/main.rs\n+++ b/src/main.rs\n".to_owned(),
        "src/main.rs".to_owned(),
        RiskLevel::Low,
        "abc123".to_owned(),
    )
}

#[tokio::test]
async fn create_persists_all_fields() {
    let db = db::connect_memory().await.expect("db");
    let repo = ApprovalRepo::new(Arc::new(db));

    let req = sample_request("sess-1");
    let id = req.id.clone();
    let created = repo.create(&req).await.expect("create");

    assert_eq!(created.id, id);
    assert_eq!(created.session_id, "sess-1");
    assert_eq!(created.title, "Add endpoint");
    assert_eq!(created.description, Some("Adds a REST endpoint".to_owned()));
    assert_eq!(created.risk_level, RiskLevel::Low);
    assert_eq!(created.status, ApprovalStatus::Pending);
    assert!(created.consumed_at.is_none());
}

#[tokio::test]
async fn get_by_id_returns_none_for_missing() {
    let db = db::connect_memory().await.expect("db");
    let repo = ApprovalRepo::new(Arc::new(db));

    let result = repo.get_by_id("nonexistent").await.expect("query");
    assert!(result.is_none());
}

#[tokio::test]
async fn get_by_id_returns_persisted_record() {
    let db = db::connect_memory().await.expect("db");
    let repo = ApprovalRepo::new(Arc::new(db));

    let req = sample_request("sess-2");
    let id = req.id.clone();
    repo.create(&req).await.expect("create");

    let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.file_path, "src/main.rs");
}

#[tokio::test]
async fn update_status_changes_status() {
    let db = db::connect_memory().await.expect("db");
    let repo = ApprovalRepo::new(Arc::new(db));

    let req = sample_request("sess-3");
    let id = req.id.clone();
    repo.create(&req).await.expect("create");

    repo.update_status(&id, ApprovalStatus::Approved)
        .await
        .expect("approve");

    let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(fetched.status, ApprovalStatus::Approved);
}

#[tokio::test]
async fn get_pending_for_session_returns_pending_only() {
    let db = db::connect_memory().await.expect("db");
    let repo = ApprovalRepo::new(Arc::new(db));

    let req1 = sample_request("sess-4");
    let req2 = sample_request("sess-4");
    let id1 = req1.id.clone();
    repo.create(&req1).await.expect("create1");
    repo.create(&req2).await.expect("create2");

    // Approve req1 — only req2 should remain pending.
    repo.update_status(&id1, ApprovalStatus::Approved)
        .await
        .expect("approve");

    let pending = repo.get_pending_for_session("sess-4").await.expect("query");
    assert!(pending.is_some());
    assert_eq!(pending.as_ref().map(|p| &p.id), Some(&req2.id));
}

#[tokio::test]
async fn mark_consumed_sets_consumed_at() {
    let db = db::connect_memory().await.expect("db");
    let repo = ApprovalRepo::new(Arc::new(db));

    let req = sample_request("sess-5");
    let id = req.id.clone();
    repo.create(&req).await.expect("create");
    repo.update_status(&id, ApprovalStatus::Approved)
        .await
        .expect("approve");

    repo.mark_consumed(&id).await.expect("consume");

    let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(fetched.status, ApprovalStatus::Consumed);
    assert!(fetched.consumed_at.is_some());
}

#[tokio::test]
async fn mark_consumed_twice_returns_error() {
    let db = db::connect_memory().await.expect("db");
    let repo = ApprovalRepo::new(Arc::new(db));

    let req = sample_request("sess-6");
    let id = req.id.clone();
    repo.create(&req).await.expect("create");
    repo.update_status(&id, ApprovalStatus::Approved)
        .await
        .expect("approve");
    repo.mark_consumed(&id).await.expect("first consume");

    let result = repo.mark_consumed(&id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn mark_consumed_on_pending_returns_error() {
    let db = db::connect_memory().await.expect("db");
    let repo = ApprovalRepo::new(Arc::new(db));

    let req = sample_request("sess-7");
    let id = req.id.clone();
    repo.create(&req).await.expect("create");

    let result = repo.mark_consumed(&id).await;
    assert!(result.is_err());
}

// ─── F.3-T3: pending-clearance persistence + resume rebind ───────────

/// Reassigning carries only the *pending* clearances of a crashed session to
/// the resumed session, preserving the ACP correlation id (the request id).
#[tokio::test]
async fn reassign_pending_carries_clearance_to_resumed_session() {
    let db = db::connect_memory().await.expect("db");
    let repo = ApprovalRepo::new(Arc::new(db));

    // One pending clearance and one already-decided clearance for the crashed session.
    let pending = sample_request("sess-crashed");
    let pending_id = pending.id.clone();
    repo.create(&pending).await.expect("create pending");

    let decided = sample_request("sess-crashed");
    let decided_id = decided.id.clone();
    repo.create(&decided).await.expect("create decided");
    repo.update_status(&decided_id, ApprovalStatus::Approved)
        .await
        .expect("approve");

    let moved = repo
        .reassign_pending_to_session("sess-crashed", "sess-resumed")
        .await
        .expect("reassign");
    assert_eq!(moved, 1, "only the pending clearance moves");

    // The crashed session no longer has a pending clearance.
    assert!(repo
        .get_pending_for_session("sess-crashed")
        .await
        .expect("fetch crashed")
        .is_none());

    // The resumed session inherits the pending clearance with the same
    // correlation id (request id) restored.
    let resumed = repo
        .get_pending_for_session("sess-resumed")
        .await
        .expect("fetch resumed")
        .expect("pending present");
    assert_eq!(resumed.id, pending_id, "correlation id must be preserved");
    assert_eq!(resumed.status, ApprovalStatus::Pending);

    // The decided clearance stays with the crashed session.
    let decided_after = repo
        .get_by_id(&decided_id)
        .await
        .expect("fetch decided")
        .expect("present");
    assert_eq!(decided_after.session_id, "sess-crashed");
}

/// A pending clearance survives a full DB restart (close pool, reopen the same
/// file-backed database) with its correlation id and status intact.
#[tokio::test]
async fn pending_clearance_survives_db_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("approval-restart.db");
    let path_str = path.to_str().expect("utf8");

    let saved_id = {
        let db = db::connect(path_str).await.expect("connect");
        let repo = ApprovalRepo::new(Arc::new(db));
        let req = sample_request("sess-restart");
        let id = req.id.clone();
        repo.create(&req).await.expect("create");
        id
    }; // pool dropped == server shutdown

    let db2 = db::connect(path_str).await.expect("reconnect");
    let repo2 = ApprovalRepo::new(Arc::new(db2));
    let restored = repo2
        .get_pending_for_session("sess-restart")
        .await
        .expect("fetch after restart")
        .expect("pending present after restart");
    assert_eq!(restored.id, saved_id);
    assert_eq!(restored.status, ApprovalStatus::Pending);
}
