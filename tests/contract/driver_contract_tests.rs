//! Contract tests for `McpDriver` — verifies resolve/reject semantics and
//! `NotFound` error for unknown request IDs (T022–T023).

use std::collections::HashMap;
use std::sync::Arc;

use agent_intercom::driver::mcp_driver::McpDriver;
use agent_intercom::driver::AgentDriver;
use agent_intercom::mcp::handler::{ApprovalResponse, PendingApprovals};
use agent_intercom::AppError;
use tokio::sync::{oneshot, Mutex};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Build a `PendingApprovals` map pre-seeded with one entry.
///
/// Returns `(map, receiver)` so the test can await the decision.
async fn seed_approval(
    request_id: &str,
) -> (PendingApprovals, oneshot::Receiver<ApprovalResponse>) {
    let (tx, rx) = oneshot::channel();
    let map: PendingApprovals = Arc::new(Mutex::new(std::collections::HashMap::new()));
    map.lock().await.insert(request_id.to_owned(), tx);
    (map, rx)
}

// ── T022: resolve_clearance — approved path ───────────────────────────────────

/// T022a — `resolve_clearance` with `approved = true` delivers an
/// `ApprovalResponse { status: "approved", reason: None }` through the oneshot.
#[tokio::test]
async fn mcp_driver_resolve_clearance_approved() {
    let (pending, rx) = seed_approval("req-approved-001").await;
    let driver = McpDriver::new(
        pending,
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(Mutex::new(HashMap::new())),
    );

    driver
        .resolve_clearance("req-approved-001", true, None)
        .await
        .expect("resolve_clearance should succeed");

    let response = rx.await.expect("oneshot must be delivered");
    assert_eq!(response.status, "approved");
    assert!(response.reason.is_none());
}

// ── T022b: resolve_clearance — rejected path ─────────────────────────────────

/// T022b — `resolve_clearance` with `approved = false` delivers an
/// `ApprovalResponse { status: "rejected", reason: Some(...) }`.
#[tokio::test]
async fn mcp_driver_resolve_clearance_rejected() {
    let (pending, rx) = seed_approval("req-rejected-002").await;
    let driver = McpDriver::new(
        pending,
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(Mutex::new(HashMap::new())),
    );

    driver
        .resolve_clearance("req-rejected-002", false, Some("diff too risky".to_owned()))
        .await
        .expect("resolve_clearance should succeed");

    let response = rx.await.expect("oneshot must be delivered");
    assert_eq!(response.status, "rejected");
    assert_eq!(response.reason.as_deref(), Some("diff too risky"));
}

// ── T023: resolve_clearance — unknown ID returns NotFound ────────────────────

/// T023 — `resolve_clearance` with an unknown `request_id` returns
/// `Err(AppError::NotFound(...))` without panicking.
#[tokio::test]
async fn mcp_driver_resolve_clearance_unknown_id_returns_not_found() {
    let driver = McpDriver::new_empty();

    let result = driver
        .resolve_clearance("req-does-not-exist", true, None)
        .await;

    assert!(result.is_err(), "unknown request_id must return Err");
    let err = result.unwrap_err();
    assert!(
        matches!(err, AppError::NotFound(_)),
        "error must be NotFound, got: {err}"
    );
}
