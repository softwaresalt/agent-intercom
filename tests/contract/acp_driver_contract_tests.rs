//! Contract tests for `AcpDriver` — verifies clearance resolution, prompt
//! forwarding, deregister cleanup, interrupt idempotency, and `resolve_prompt`
//! routing (RI-08).
//!
//! Mirrors the MCP driver contract tests in `driver_contract_tests.rs`.

use serde_json::Value;
use tokio::sync::mpsc;

use agent_intercom::driver::acp_driver::AcpDriver;
use agent_intercom::driver::AgentDriver;
use agent_intercom::AppError;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Set up an `AcpDriver` with a registered session and writer channel.
///
/// Returns `(driver, receiver)` so the test can inspect outbound messages.
async fn setup_driver_with_session(session_id: &str) -> (AcpDriver, mpsc::Receiver<Value>) {
    let driver = AcpDriver::new();
    let (tx, rx) = mpsc::channel(16);
    driver.register_session(session_id, tx).await;
    (driver, rx)
}

// ── resolve_clearance — approved path ───────────────────────────────────────

/// Resolving a registered clearance with `approved = true` delivers a
/// `clearance/response` message with `status: "approved"` to the correct
/// session's writer channel.
#[tokio::test]
async fn acp_driver_resolve_clearance_approved() {
    let (driver, mut rx) = setup_driver_with_session("sess-001").await;

    // Register a pending clearance
    driver.register_clearance("sess-001", "req-approve-1").await;

    driver
        .resolve_clearance("req-approve-1", true, None)
        .await
        .expect("resolve_clearance should succeed");

    let msg = rx.recv().await.expect("must receive a message");
    assert_eq!(msg["method"], "clearance/response");
    assert_eq!(msg["id"], "req-approve-1");
    assert_eq!(msg["params"]["status"], "approved");
    assert!(
        msg["params"]["reason"].is_null(),
        "approved clearance has no reason"
    );
}

// ── resolve_clearance — rejected path ───────────────────────────────────────

/// Resolving with `approved = false` delivers `status: "rejected"` with
/// the operator's reason.
#[tokio::test]
async fn acp_driver_resolve_clearance_rejected() {
    let (driver, mut rx) = setup_driver_with_session("sess-002").await;

    driver.register_clearance("sess-002", "req-reject-1").await;

    driver
        .resolve_clearance("req-reject-1", false, Some("too risky".to_owned()))
        .await
        .expect("resolve_clearance should succeed");

    let msg = rx.recv().await.expect("must receive a message");
    assert_eq!(msg["params"]["status"], "rejected");
    assert_eq!(msg["params"]["reason"], "too risky");
}

// ── resolve_clearance — unknown request_id ──────────────────────────────────

/// An unknown `request_id` returns `AppError::NotFound`.
#[tokio::test]
async fn acp_driver_resolve_clearance_unknown_id_returns_not_found() {
    let driver = AcpDriver::new();

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

// ── resolve_clearance — standard permission path (T8.1, ADR-0016) ────────────

/// A registered standard `session/request_permission` resolved with
/// `approved = true` emits a JSON-RPC `result` carrying a `selected` outcome
/// whose `optionId` is the `allow_once` option — not a bespoke `clearance/response`.
#[tokio::test]
async fn acp_driver_resolve_permission_approved_emits_selected_allow_outcome() {
    use agent_intercom::driver::PermissionOption;

    let (driver, mut rx) = setup_driver_with_session("sess-p1").await;
    let options = vec![
        PermissionOption {
            option_id: "allow-once".to_owned(),
            name: "Allow".to_owned(),
            kind: "allow_once".to_owned(),
        },
        PermissionOption {
            option_id: "reject-once".to_owned(),
            name: "Reject".to_owned(),
            kind: "reject_once".to_owned(),
        },
    ];
    driver
        .register_permission(
            "sess-p1",
            "perm-req-1",
            options,
            serde_json::json!("perm-req-1"),
        )
        .await;

    driver
        .resolve_clearance("perm-req-1", true, None)
        .await
        .expect("resolve_clearance should succeed for a permission");

    let msg = rx.recv().await.expect("must receive a message");
    assert_eq!(msg["id"], "perm-req-1");
    assert_eq!(msg["result"]["outcome"]["outcome"], "selected");
    assert_eq!(msg["result"]["outcome"]["optionId"], "allow-once");
    assert!(
        msg["method"].is_null(),
        "a standard JSON-RPC result reply carries no `method`"
    );
}

/// Rejecting a permission selects the `reject_once` option.
#[tokio::test]
async fn acp_driver_resolve_permission_rejected_selects_reject_option() {
    use agent_intercom::driver::PermissionOption;

    let (driver, mut rx) = setup_driver_with_session("sess-p2").await;
    let options = vec![
        PermissionOption {
            option_id: "allow-once".to_owned(),
            name: "Allow".to_owned(),
            kind: "allow_once".to_owned(),
        },
        PermissionOption {
            option_id: "reject-once".to_owned(),
            name: "Reject".to_owned(),
            kind: "reject_once".to_owned(),
        },
    ];
    driver
        .register_permission(
            "sess-p2",
            "perm-req-2",
            options,
            serde_json::json!("perm-req-2"),
        )
        .await;

    driver
        .resolve_clearance("perm-req-2", false, Some("too risky".to_owned()))
        .await
        .expect("resolve_clearance should succeed");

    let msg = rx.recv().await.expect("must receive a message");
    assert_eq!(msg["result"]["outcome"]["outcome"], "selected");
    assert_eq!(msg["result"]["outcome"]["optionId"], "reject-once");
}

/// A permission with no options cannot be selected, so approval yields a
/// `cancelled` outcome (the conformant way to decline without a valid option).
#[tokio::test]
async fn acp_driver_resolve_permission_no_options_emits_cancelled() {
    let (driver, mut rx) = setup_driver_with_session("sess-p3").await;
    driver
        .register_permission(
            "sess-p3",
            "perm-req-3",
            vec![],
            serde_json::json!("perm-req-3"),
        )
        .await;

    driver
        .resolve_clearance("perm-req-3", true, None)
        .await
        .expect("resolve_clearance should succeed");

    let msg = rx.recv().await.expect("must receive a message");
    assert_eq!(msg["result"]["outcome"]["outcome"], "cancelled");
    assert!(msg["result"]["outcome"]["optionId"].is_null());
}

/// A numeric JSON-RPC `id` (as used by real conformant ACP agents) must be
/// echoed back as a number, not stringified — otherwise the agent cannot match
/// the response to its request and the session deadlocks.
#[tokio::test]
async fn acp_driver_resolve_permission_preserves_numeric_id() {
    use agent_intercom::driver::PermissionOption;

    let (driver, mut rx) = setup_driver_with_session("sess-p4").await;
    let options = vec![PermissionOption {
        option_id: "allow-once".to_owned(),
        name: "Allow".to_owned(),
        kind: "allow_once".to_owned(),
    }];
    // Correlation key is the stringified id "7"; the raw id is numeric 7.
    driver
        .register_permission("sess-p4", "7", options, serde_json::json!(7))
        .await;

    driver
        .resolve_clearance("7", true, None)
        .await
        .expect("resolve_clearance should succeed");

    let msg = rx.recv().await.expect("must receive a message");
    assert_eq!(msg["id"], 7, "numeric id must be echoed as a number");
    assert!(
        msg["id"].is_number(),
        "id must remain numeric, not stringified"
    );
    assert_eq!(msg["result"]["outcome"]["optionId"], "allow-once");
}

/// Sending a prompt to a registered session delivers a `session/prompt`
/// message with the correct agent session ID and prompt text.
#[tokio::test]
async fn acp_driver_send_prompt_delivers_to_session() {
    let (driver, mut rx) = setup_driver_with_session("sess-003").await;

    // Must register agent session ID first (from handshake)
    driver
        .register_agent_session_id("sess-003", "agent-abc")
        .await;

    driver
        .send_prompt("sess-003", "List all files")
        .await
        .expect("send_prompt should succeed");

    let msg = rx.recv().await.expect("must receive a message");
    assert_eq!(msg["method"], "session/prompt");
    assert_eq!(msg["params"]["sessionId"], "agent-abc");

    let prompt_array = msg["params"]["prompt"]
        .as_array()
        .expect("prompt must be array");
    assert_eq!(prompt_array.len(), 1);
    assert_eq!(prompt_array[0]["type"], "text");
    assert_eq!(prompt_array[0]["text"], "List all files");
}

// ── send_prompt — missing agent session ID ──────────────────────────────────

/// Sending a prompt without a registered agent session ID returns `NotFound`.
#[tokio::test]
async fn acp_driver_send_prompt_missing_agent_sid_returns_not_found() {
    let (driver, _rx) = setup_driver_with_session("sess-004").await;
    // Note: register_agent_session_id NOT called

    let result = driver.send_prompt("sess-004", "Hello").await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::NotFound(_)));
}

// ── interrupt — success path ────────────────────────────────────────────────

/// Interrupt sends a `session/interrupt` message to the correct session.
#[tokio::test]
async fn acp_driver_interrupt_sends_interrupt_message() {
    let (driver, mut rx) = setup_driver_with_session("sess-005").await;

    driver
        .interrupt("sess-005")
        .await
        .expect("interrupt should succeed");

    let msg = rx.recv().await.expect("must receive a message");
    assert_eq!(msg["method"], "session/interrupt");
    assert!(msg["params"]["reason"].as_str().is_some());
}

// ── interrupt — idempotent for disconnected session ─────────────────────────

/// Interrupting an unregistered session is a no-op (returns Ok).
#[tokio::test]
async fn acp_driver_interrupt_disconnected_is_noop() {
    let driver = AcpDriver::new();

    let result = driver.interrupt("sess-nonexistent").await;
    assert!(
        result.is_ok(),
        "interrupt on disconnected session must be Ok"
    );
}

// ── deregister_session — cleanup ────────────────────────────────────────────

/// After deregistering, `send_prompt` returns `NotFound` (writer removed).
#[tokio::test]
async fn acp_driver_deregister_removes_session() {
    let (driver, _rx) = setup_driver_with_session("sess-006").await;

    driver
        .register_agent_session_id("sess-006", "agent-xyz")
        .await;
    driver.deregister_session("sess-006").await;

    // Writer removed — send_prompt should fail with NotFound
    let result = driver.send_prompt("sess-006", "Hello").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::NotFound(_)));
}

/// Deregistering an unknown session is a no-op (idempotent).
#[tokio::test]
async fn acp_driver_deregister_unknown_is_noop() {
    let driver = AcpDriver::new();
    // Should not panic or error
    driver.deregister_session("sess-nonexistent").await;
}

// ── resolve_prompt — success path ───────────────────────────────────────────

/// Resolving a registered prompt request delivers a `prompt/response`
/// message with the operator's decision.
#[tokio::test]
async fn acp_driver_resolve_prompt_delivers_decision() {
    let (driver, mut rx) = setup_driver_with_session("sess-007").await;

    driver
        .register_prompt_request("sess-007", "prompt-001")
        .await;

    driver
        .resolve_prompt("prompt-001", "continue", Some("keep going".to_owned()))
        .await
        .expect("resolve_prompt should succeed");

    let msg = rx.recv().await.expect("must receive a message");
    assert_eq!(msg["method"], "prompt/response");
    assert_eq!(msg["id"], "prompt-001");
    assert_eq!(msg["params"]["decision"], "continue");
    assert_eq!(msg["params"]["instruction"], "keep going");
}

// ── resolve_prompt — unknown prompt_id ──────────────────────────────────────

/// An unknown `prompt_id` returns `AppError::NotFound`.
#[tokio::test]
async fn acp_driver_resolve_prompt_unknown_id_returns_not_found() {
    let driver = AcpDriver::new();

    let result = driver
        .resolve_prompt("prompt-nonexistent", "continue", None)
        .await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::NotFound(_)));
}

// ── resolve_wait — success path ─────────────────────────────────────────────

/// Resolving a wait delivers a `session/prompt` with the instruction text.
#[tokio::test]
async fn acp_driver_resolve_wait_sends_prompt() {
    let (driver, mut rx) = setup_driver_with_session("sess-008").await;
    driver
        .register_agent_session_id("sess-008", "agent-wait")
        .await;

    driver
        .resolve_wait("sess-008", Some("do the next thing".to_owned()))
        .await
        .expect("resolve_wait should succeed");

    let msg = rx.recv().await.expect("must receive a message");
    assert_eq!(msg["method"], "session/prompt");
    assert_eq!(msg["params"]["sessionId"], "agent-wait");

    let prompt_array = msg["params"]["prompt"]
        .as_array()
        .expect("prompt must be array");
    assert_eq!(prompt_array[0]["text"], "do the next thing");
}

/// Resolving a wait without instruction defaults to "continue".
#[tokio::test]
async fn acp_driver_resolve_wait_defaults_to_continue() {
    let (driver, mut rx) = setup_driver_with_session("sess-009").await;
    driver
        .register_agent_session_id("sess-009", "agent-wait2")
        .await;

    driver
        .resolve_wait("sess-009", None)
        .await
        .expect("resolve_wait should succeed");

    let msg = rx.recv().await.expect("must receive a message");
    let prompt_array = msg["params"]["prompt"]
        .as_array()
        .expect("prompt must be array");
    assert_eq!(prompt_array[0]["text"], "continue");
}

// ── resolve_wait — missing agent session ID ─────────────────────────────────

/// Wait without agent session ID returns `NotFound`.
#[tokio::test]
async fn acp_driver_resolve_wait_missing_agent_sid_returns_not_found() {
    let (driver, _rx) = setup_driver_with_session("sess-010").await;
    // No register_agent_session_id

    let result = driver
        .resolve_wait("sess-010", Some("instruction".to_owned()))
        .await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::NotFound(_)));
}
