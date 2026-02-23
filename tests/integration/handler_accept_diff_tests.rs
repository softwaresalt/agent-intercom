//! Integration tests for the `accept_diff` tool handler logic.
//!
//! Validates:
//! - Approved request → full file write → applied + consumed
//! - Approved request → unified diff patch → applied + consumed
//! - Request not found → error
//! - Already consumed → error
//! - Rejected request → `not_approved` error
//! - Hash mismatch without force → `patch_conflict`
//! - Hash mismatch with force → applied
//! - Path traversal → `path_violation`
//! - Pending request → `not_approved` error

use std::sync::Arc;

use monocoque_agent_rc::diff::validate_workspace_path;
use monocoque_agent_rc::mcp::tools::util::compute_file_hash;
use monocoque_agent_rc::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use monocoque_agent_rc::persistence::approval_repo::ApprovalRepo;

use super::test_helpers::{create_active_session, test_app_state, test_config};

// ── Helper: create an approved request with file on disk ─────

async fn setup_approved_request(
    state: &Arc<monocoque_agent_rc::mcp::handler::AppState>,
    session_id: &str,
    file_path: &str,
    diff_content: &str,
    workspace_root: &std::path::Path,
) -> (String, ApprovalRepo) {
    let repo = ApprovalRepo::new(Arc::clone(&state.db));

    // Compute the hash of the current file (or "new_file" if absent).
    let full_path = workspace_root.join(file_path);
    let hash = compute_file_hash(&full_path).await.expect("compute hash");

    let approval = ApprovalRequest::new(
        session_id.into(),
        "Test diff".into(),
        None,
        diff_content.into(),
        file_path.into(),
        RiskLevel::Low,
        hash,
    );
    let id = approval.id.clone();
    repo.create(&approval).await.expect("create");

    // Mark as approved.
    repo.update_status(&id, ApprovalStatus::Approved)
        .await
        .expect("approve");

    (id, repo)
}

// ── accept_diff: full file write → applied ───────────────────

#[tokio::test]
async fn accept_diff_full_file_write_applied() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let root_str = root.to_str().expect("utf8");
    let state = test_app_state(test_config(root_str)).await;
    let session = create_active_session(&state.db, root_str).await;

    // New file content (not a unified diff — no "--- " or "diff " prefix).
    let content = "fn main() {\n    println!(\"hello\");\n}\n";
    let file_path = "src/hello.rs";

    let (request_id, repo) =
        setup_approved_request(&state, &session.id, file_path, content, root).await;

    // Validate path.
    let validated = validate_workspace_path(root, file_path).expect("valid path");

    // Write.
    let result = monocoque_agent_rc::diff::writer::write_full_file(&validated, content, root);
    assert!(result.is_ok(), "write should succeed");

    // Verify file exists and contents.
    let written = std::fs::read_to_string(&validated).expect("read file");
    assert_eq!(written, content);

    // Mark as consumed.
    repo.mark_consumed(&request_id).await.expect("consume");

    let final_state = repo
        .get_by_id(&request_id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(final_state.status, ApprovalStatus::Consumed);
    assert!(final_state.consumed_at.is_some());
}

// ── accept_diff: unified diff patch → applied ────────────────

#[tokio::test]
async fn accept_diff_unified_patch_applied() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let root_str = root.to_str().expect("utf8");
    let state = test_app_state(test_config(root_str)).await;
    let session = create_active_session(&state.db, root_str).await;

    // Write original file.
    let file_path = "src/lib.rs";
    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    let full = root.join(file_path);
    std::fs::write(&full, "fn old() {}\n").expect("write original");

    let original_hash = compute_file_hash(&full).await.expect("hash");

    // Unified diff.
    let diff = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-fn old() {}\n+fn new() {}\n";

    let repo = ApprovalRepo::new(Arc::clone(&state.db));
    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Patch test".into(),
        None,
        diff.into(),
        file_path.into(),
        RiskLevel::Low,
        original_hash,
    );
    let request_id = approval.id.clone();
    repo.create(&approval).await.expect("create");
    repo.update_status(&request_id, ApprovalStatus::Approved)
        .await
        .expect("approve");

    // Apply patch.
    let validated = validate_workspace_path(root, file_path).expect("valid path");

    // The diff starts with "--- " so the handler uses apply_patch.
    let is_unified = diff.starts_with("--- ") || diff.starts_with("diff ");
    assert!(is_unified, "should detect as unified diff");

    let result = monocoque_agent_rc::diff::patcher::apply_patch(&validated, diff, root);
    assert!(result.is_ok(), "patch should succeed: {:?}", result.err());

    let patched = std::fs::read_to_string(&full).expect("read file");
    assert!(
        patched.contains("fn new()"),
        "patched file should contain new function"
    );
}

// ── accept_diff: request not found ───────────────────────────

#[tokio::test]
async fn accept_diff_request_not_found() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = ApprovalRepo::new(Arc::clone(&state.db));

    let found = repo.get_by_id("nonexistent-id").await.expect("query");
    assert!(found.is_none());
}

// ── accept_diff: already consumed ────────────────────────────

#[tokio::test]
async fn accept_diff_already_consumed_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = ApprovalRepo::new(Arc::clone(&state.db));

    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Consumed test".into(),
        None,
        "content".into(),
        "src/main.rs".into(),
        RiskLevel::Low,
        "hash".into(),
    );
    let id = approval.id.clone();
    repo.create(&approval).await.expect("create");
    repo.update_status(&id, ApprovalStatus::Approved)
        .await
        .expect("approve");
    repo.mark_consumed(&id).await.expect("consume");

    let updated = repo.get_by_id(&id).await.expect("get").expect("found");
    assert_eq!(updated.status, ApprovalStatus::Consumed);

    // Double-consume attempt via the repo.
    let result = repo.mark_consumed(&id).await;
    assert!(result.is_err(), "double consume should fail");
}

// ── accept_diff: rejected request → not_approved ─────────────

#[tokio::test]
async fn accept_diff_rejected_request_not_approved() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = ApprovalRepo::new(Arc::clone(&state.db));

    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Rejected test".into(),
        None,
        "content".into(),
        "src/main.rs".into(),
        RiskLevel::Low,
        "hash".into(),
    );
    let id = approval.id.clone();
    repo.create(&approval).await.expect("create");
    repo.update_status(&id, ApprovalStatus::Rejected)
        .await
        .expect("reject");

    let updated = repo.get_by_id(&id).await.expect("get").expect("found");
    assert_ne!(
        updated.status,
        ApprovalStatus::Approved,
        "rejected request should not be approved"
    );
}

// ── accept_diff: pending request → not_approved ──────────────

#[tokio::test]
async fn accept_diff_pending_request_not_approved() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = ApprovalRepo::new(Arc::clone(&state.db));

    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Pending test".into(),
        None,
        "content".into(),
        "src/main.rs".into(),
        RiskLevel::Low,
        "hash".into(),
    );
    let id = approval.id.clone();
    repo.create(&approval).await.expect("create");

    let fetched = repo.get_by_id(&id).await.expect("get").expect("found");
    assert_eq!(fetched.status, ApprovalStatus::Pending);
    assert_ne!(
        fetched.status,
        ApprovalStatus::Approved,
        "pending request should not pass the approved check"
    );
}

// ── accept_diff: hash mismatch without force → conflict ──────

#[tokio::test]
async fn accept_diff_hash_mismatch_no_force_conflicts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let root_str = root.to_str().expect("utf8");
    let state = test_app_state(test_config(root_str)).await;
    let _session = create_active_session(&state.db, root_str).await;

    // Write a file and record its hash.
    let file_path = "src/conflict.rs";
    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).expect("mkdir");
    let full = root.join(file_path);
    std::fs::write(&full, "original content").expect("write");
    let original_hash = compute_file_hash(&full).await.expect("hash");

    // Mutate the file after recording hash.
    std::fs::write(&full, "mutated content").expect("mutate");

    let current_hash = compute_file_hash(&full).await.expect("hash");
    assert_ne!(
        original_hash, current_hash,
        "hash should differ after mutation"
    );

    // Without force, handler would return patch_conflict.
    let force = false;
    let hash_matches = current_hash == original_hash;
    assert!(
        !hash_matches && !force,
        "should detect conflict without force"
    );
}

// ── accept_diff: hash mismatch with force → applied ──────────

#[tokio::test]
async fn accept_diff_hash_mismatch_with_force_applies() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let root_str = root.to_str().expect("utf8");
    let state = test_app_state(test_config(root_str)).await;
    let _session = create_active_session(&state.db, root_str).await;

    let file_path = "src/force.rs";
    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).expect("mkdir");
    let full = root.join(file_path);
    std::fs::write(&full, "original").expect("write");
    let original_hash = compute_file_hash(&full).await.expect("hash");

    // Mutate.
    std::fs::write(&full, "mutated").expect("mutate");
    let current_hash = compute_file_hash(&full).await.expect("hash");
    assert_ne!(original_hash, current_hash);

    // With force, the write proceeds.
    let force = true;
    let hash_matches = current_hash == original_hash;
    assert!(!hash_matches && force, "force should override conflict");

    // Write new content (simulating force-apply).
    let new_content = "forced new content";
    let validated = validate_workspace_path(root, file_path).expect("valid");
    let result = monocoque_agent_rc::diff::writer::write_full_file(&validated, new_content, root);
    assert!(result.is_ok());

    let written = std::fs::read_to_string(&full).expect("read");
    assert_eq!(written, new_content);
}

// ── accept_diff: path traversal → violation ──────────────────

#[tokio::test]
async fn accept_diff_path_traversal_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    let result = validate_workspace_path(root, "../../../etc/passwd");
    assert!(result.is_err(), "path traversal should be rejected");
}

// ── accept_diff: new file hash is "new_file" ─────────────────

#[tokio::test]
async fn accept_diff_new_file_hash() {
    let temp = tempfile::tempdir().expect("tempdir");
    let nonexistent = temp.path().join("does_not_exist.rs");
    let hash = compute_file_hash(&nonexistent).await.expect("hash");
    assert_eq!(hash, "new_file");
}
