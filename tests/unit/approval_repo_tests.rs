//! Unit tests for `ApprovalRepo` CRUD operations (T019).
//!
//! Validates:
//! - Create approval request and verify all fields persisted
//! - `get_by_id` returns `None` for missing records
//! - `update_status` transitions and `get_pending_for_session`
//! - `mark_consumed` sets `consumed_at` and enforces single-use
//! - Double-consume returns `AlreadyConsumed` error

use std::sync::Arc;

use monocoque_agent_rc::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use monocoque_agent_rc::persistence::{approval_repo::ApprovalRepo, db};

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
