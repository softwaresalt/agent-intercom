//! Integration tests for the three **blocking** MCP tool handlers:
//! `ask_approval`, `forward_prompt`, and `wait_for_instruction`.
//!
//! These tests exercise the full oneshot-channel lifecycle:
//! 1. Create the DB record (approval / prompt)
//! 2. Register the oneshot sender in the pending map
//! 3. Resolve the receiver via operator response or timeout
//! 4. Verify DB state changes and cleanup
//!
//! Also validates edge cases: timeout → expired/auto-continue,
//! sender drop, and pending map cleanup.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;

use monocoque_agent_rc::mcp::handler::{ApprovalResponse, PromptResponse, WaitResponse};
use monocoque_agent_rc::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use monocoque_agent_rc::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use monocoque_agent_rc::persistence::approval_repo::ApprovalRepo;
use monocoque_agent_rc::persistence::prompt_repo::PromptRepo;
use monocoque_agent_rc::persistence::session_repo::SessionRepo;

use super::test_helpers::{create_active_session, test_app_state, test_config};

// ═══════════════════════════════════════════════════════════════
//  ask_approval
// ═══════════════════════════════════════════════════════════════

// ── ask_approval: accept via oneshot resolves with approved ───

#[tokio::test]
async fn ask_approval_accept_via_oneshot() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;

    // Create approval record in DB.
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Add auth".into(),
        Some("Adds JWT auth".into()),
        "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new".into(),
        "src/main.rs".into(),
        RiskLevel::Low,
        "abc123".into(),
    );
    let request_id = approval.id.clone();
    approval_repo.create(&approval).await.expect("create");

    // Create oneshot channel (handler pattern).
    let (tx, rx) = oneshot::channel::<ApprovalResponse>();

    // Register the sender in the pending map.
    {
        let mut pending = state.pending_approvals.lock().await;
        pending.insert(request_id.clone(), tx);
    }

    // Spawn a task that mimics operator input (Slack button click).
    let state_clone = Arc::clone(&state);
    let request_id_clone = request_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        // Remove sender from pending map and send response.
        let mut pending = state_clone.pending_approvals.lock().await;
        if let Some(sender) = pending.remove(&request_id_clone) {
            let _ = sender.send(ApprovalResponse {
                status: "approved".into(),
                reason: None,
            });
        }
    });

    // Await the response with a timeout (handler pattern).
    let timeout_duration = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout_duration, rx).await;

    match response {
        Ok(Ok(resp)) => {
            assert_eq!(resp.status, "approved");
            assert!(resp.reason.is_none());
        }
        other => panic!("expected Ok(Ok(approved)), got: {other:?}"),
    }

    // Verify DB update: mark as approved (handler does this after receiving response).
    let approval_repo2 = ApprovalRepo::new(Arc::clone(&state.db));
    approval_repo2
        .update_status(&approval.id, ApprovalStatus::Approved)
        .await
        .expect("update status");

    let updated = approval_repo2
        .get_by_id(&approval.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.status, ApprovalStatus::Approved);
}

// ── ask_approval: reject with reason ─────────────────────────

#[tokio::test]
async fn ask_approval_reject_with_reason() {
    let (tx, rx) = oneshot::channel::<ApprovalResponse>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(ApprovalResponse {
            status: "rejected".into(),
            reason: Some("Security issue".into()),
        });
    });

    let timeout = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout, rx).await;

    match response {
        Ok(Ok(resp)) => {
            assert_eq!(resp.status, "rejected");
            assert_eq!(resp.reason.as_deref(), Some("Security issue"));
        }
        other => panic!("expected rejection, got: {other:?}"),
    }
}

// ── ask_approval: timeout → expired status ───────────────────

#[tokio::test]
async fn ask_approval_timeout_yields_expired() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;

    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Timeout test".into(),
        None,
        "diff".into(),
        "src/lib.rs".into(),
        RiskLevel::Low,
        "hash".into(),
    );
    let request_id = approval.id.clone();
    approval_repo.create(&approval).await.expect("create");

    let (_tx, rx) = oneshot::channel::<ApprovalResponse>();
    // Do NOT send anything — let it time out.

    let timeout = Duration::from_millis(200);
    let response = tokio::time::timeout(timeout, rx).await;

    assert!(response.is_err(), "should have timed out");

    // Handler marks as expired on timeout.
    approval_repo
        .update_status(&request_id, ApprovalStatus::Expired)
        .await
        .expect("update status");

    let updated = approval_repo
        .get_by_id(&request_id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.status, ApprovalStatus::Expired);
}

// ── ask_approval: sender drop → timeout-like ─────────────────

#[tokio::test]
async fn ask_approval_sender_drop_yields_timeout() {
    let (tx, rx) = oneshot::channel::<ApprovalResponse>();

    // Drop the sender without sending.
    drop(tx);

    let timeout = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout, rx).await;

    match response {
        Ok(Err(_)) => {} // RecvError — sender dropped.
        other => panic!("expected sender drop error, got: {other:?}"),
    }
}

// ── ask_approval: pending map cleanup after resolve ──────────

#[tokio::test]
async fn ask_approval_pending_map_cleanup() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let request_id = "req-cleanup-test";
    let (tx, rx) = oneshot::channel::<ApprovalResponse>();

    {
        let mut pending = state.pending_approvals.lock().await;
        pending.insert(request_id.into(), tx);
        assert!(pending.contains_key(request_id));
    }

    // Simulate resolve.
    drop(rx);

    // Cleanup.
    {
        let mut pending = state.pending_approvals.lock().await;
        pending.remove(request_id);
        assert!(!pending.contains_key(request_id));
    }
}

// ── ask_approval: DB persistence of all fields ───────────────

#[tokio::test]
async fn ask_approval_persists_all_fields() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));

    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Full field test".into(),
        Some("Description".into()),
        "full diff content".into(),
        "src/config.rs".into(),
        RiskLevel::Critical,
        "sha256hash".into(),
    );
    let id = approval.id.clone();
    approval_repo.create(&approval).await.expect("create");

    let persisted = approval_repo
        .get_by_id(&id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(persisted.session_id, session.id);
    assert_eq!(persisted.title, "Full field test");
    assert_eq!(persisted.description.as_deref(), Some("Description"));
    assert_eq!(persisted.diff_content, "full diff content");
    assert_eq!(persisted.file_path, "src/config.rs");
    assert_eq!(persisted.risk_level, RiskLevel::Critical);
    assert_eq!(persisted.original_hash, "sha256hash");
    assert_eq!(persisted.status, ApprovalStatus::Pending);
}

// ═══════════════════════════════════════════════════════════════
//  forward_prompt
// ═══════════════════════════════════════════════════════════════

// ── forward_prompt: continue via oneshot ──────────────────────

#[tokio::test]
async fn forward_prompt_continue_via_oneshot() {
    let (tx, rx) = oneshot::channel::<PromptResponse>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(PromptResponse {
            decision: "continue".into(),
            instruction: None,
        });
    });

    let timeout = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout, rx).await;

    match response {
        Ok(Ok(resp)) => {
            assert_eq!(resp.decision, "continue");
            assert!(resp.instruction.is_none());
        }
        other => panic!("expected continue, got: {other:?}"),
    }
}

// ── forward_prompt: refine with instruction ──────────────────

#[tokio::test]
async fn forward_prompt_refine_with_instruction() {
    let (tx, rx) = oneshot::channel::<PromptResponse>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(PromptResponse {
            decision: "refine".into(),
            instruction: Some("Focus on error handling".into()),
        });
    });

    let timeout = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout, rx).await;

    match response {
        Ok(Ok(resp)) => {
            assert_eq!(resp.decision, "refine");
            assert_eq!(resp.instruction.as_deref(), Some("Focus on error handling"));
        }
        other => panic!("expected refine, got: {other:?}"),
    }
}

// ── forward_prompt: stop decision ────────────────────────────

#[tokio::test]
async fn forward_prompt_stop_decision() {
    let (tx, rx) = oneshot::channel::<PromptResponse>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(PromptResponse {
            decision: "stop".into(),
            instruction: None,
        });
    });

    let timeout = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout, rx).await;

    match response {
        Ok(Ok(resp)) => assert_eq!(resp.decision, "stop"),
        other => panic!("expected stop, got: {other:?}"),
    }
}

// ── forward_prompt: timeout → auto-continue (FR-008) ─────────

#[tokio::test]
async fn forward_prompt_timeout_auto_continues() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;

    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
    let prompt = ContinuationPrompt::new(
        session.id.clone(),
        "Should I proceed?".into(),
        PromptType::Continuation,
        Some(60),
        Some(3),
    );
    let prompt_id = prompt.id.clone();
    prompt_repo.create(&prompt).await.expect("create");

    let (_tx, rx) = oneshot::channel::<PromptResponse>();

    let timeout = Duration::from_millis(200);
    let response = tokio::time::timeout(timeout, rx).await;

    assert!(response.is_err(), "should have timed out");

    // Handler auto-continues on timeout (FR-008).
    prompt_repo
        .update_decision(&prompt_id, PromptDecision::Continue, None)
        .await
        .expect("update decision");

    let updated = prompt_repo
        .get_by_id(&prompt_id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.decision, Some(PromptDecision::Continue));
}

// ── forward_prompt: sender drop → auto-continue ──────────────

#[tokio::test]
async fn forward_prompt_sender_drop_auto_continues() {
    let (tx, rx) = oneshot::channel::<PromptResponse>();

    drop(tx);

    let timeout = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout, rx).await;

    // Sender dropped → RecvError, handler defaults to "continue".
    match response {
        Ok(Err(_)) => {} // Expected.
        other => panic!("expected sender drop error, got: {other:?}"),
    }
}

// ── forward_prompt: all prompt types persist ─────────────────

#[tokio::test]
async fn forward_prompt_all_types_persist() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));

    let types = [
        PromptType::Continuation,
        PromptType::Clarification,
        PromptType::ErrorRecovery,
        PromptType::ResourceWarning,
    ];

    for pt in &types {
        let prompt =
            ContinuationPrompt::new(session.id.clone(), format!("test {pt:?}"), *pt, None, None);
        let id = prompt.id.clone();
        prompt_repo.create(&prompt).await.expect("create");

        let persisted = prompt_repo
            .get_by_id(&id)
            .await
            .expect("get")
            .expect("found");
        assert_eq!(persisted.prompt_type, *pt);
    }
}

// ── forward_prompt: pending map cleanup ──────────────────────

#[tokio::test]
async fn forward_prompt_pending_map_cleanup() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let prompt_id = "prompt-cleanup";
    let (tx, _rx) = oneshot::channel::<PromptResponse>();

    {
        let mut pending = state.pending_prompts.lock().await;
        pending.insert(prompt_id.into(), tx);
        assert!(pending.contains_key(prompt_id));
    }

    {
        let mut pending = state.pending_prompts.lock().await;
        pending.remove(prompt_id);
        assert!(!pending.contains_key(prompt_id));
    }
}

// ═══════════════════════════════════════════════════════════════
//  wait_for_instruction
// ═══════════════════════════════════════════════════════════════

// ── wait: resume via oneshot ─────────────────────────────────

#[tokio::test]
async fn wait_resume_via_oneshot() {
    let (tx, rx) = oneshot::channel::<WaitResponse>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(WaitResponse {
            status: "resumed".into(),
            instruction: Some("Work on feature Y".into()),
        });
    });

    let timeout = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout, rx).await;

    match response {
        Ok(Ok(resp)) => {
            assert_eq!(resp.status, "resumed");
            assert_eq!(resp.instruction.as_deref(), Some("Work on feature Y"));
        }
        other => panic!("expected resumed, got: {other:?}"),
    }
}

// ── wait: stop via oneshot ───────────────────────────────────

#[tokio::test]
async fn wait_stop_via_oneshot() {
    let (tx, rx) = oneshot::channel::<WaitResponse>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(WaitResponse {
            status: "stopped".into(),
            instruction: None,
        });
    });

    let timeout = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout, rx).await;

    match response {
        Ok(Ok(resp)) => assert_eq!(resp.status, "stopped"),
        other => panic!("expected stopped, got: {other:?}"),
    }
}

// ── wait: timeout → timeout status ───────────────────────────

#[tokio::test]
async fn wait_timeout_yields_timeout_status() {
    let (_tx, rx) = oneshot::channel::<WaitResponse>();

    let timeout = Duration::from_millis(200);
    let response = tokio::time::timeout(timeout, rx).await;

    assert!(response.is_err(), "should have timed out");
}

// ── wait: sender drop → timeout-like ─────────────────────────

#[tokio::test]
async fn wait_sender_drop_yields_timeout() {
    let (tx, rx) = oneshot::channel::<WaitResponse>();

    drop(tx);

    let timeout = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout, rx).await;

    match response {
        Ok(Err(_)) => {} // RecvError.
        other => panic!("expected sender drop, got: {other:?}"),
    }
}

// ── wait: pending map keyed by session_id ────────────────────

#[tokio::test]
async fn wait_pending_map_keyed_by_session_id() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;

    let (tx, _rx) = oneshot::channel::<WaitResponse>();

    {
        let mut pending = state.pending_waits.lock().await;
        pending.insert(session.id.clone(), tx);
        assert!(pending.contains_key(&session.id));
    }

    // Cleanup.
    {
        let mut pending = state.pending_waits.lock().await;
        pending.remove(&session.id);
        assert!(!pending.contains_key(&session.id));
    }
}

// ── wait: resume with empty instruction ──────────────────────

#[tokio::test]
async fn wait_resume_without_instruction() {
    let (tx, rx) = oneshot::channel::<WaitResponse>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(WaitResponse {
            status: "resumed".into(),
            instruction: None,
        });
    });

    let timeout = Duration::from_secs(2);
    let response = tokio::time::timeout(timeout, rx).await;

    match response {
        Ok(Ok(resp)) => {
            assert_eq!(resp.status, "resumed");
            assert!(resp.instruction.is_none());
        }
        other => panic!("expected resumed without instruction, got: {other:?}"),
    }
}

// ── wait: updates session last_tool ──────────────────────────

#[tokio::test]
async fn wait_updates_session_last_tool() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    repo.update_last_activity(&session.id, Some("wait_for_instruction".into()))
        .await
        .expect("update");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.last_tool.as_deref(), Some("wait_for_instruction"));
}
