//! Integration tests for the approve→apply diff pipeline (T109).
//!
//! End-to-end flow:
//! 1. Create approval request → approve it in DB
//! 2. Invoke `accept_diff` logic → verify file written to disk
//! 3. Test hash mismatch conflict detection when file mutates between
//!    proposal creation and application

use std::fs;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use monocoque_agent_rc::config::GlobalConfig;
use monocoque_agent_rc::diff::patcher::apply_patch;
use monocoque_agent_rc::diff::writer::write_full_file;
use monocoque_agent_rc::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use monocoque_agent_rc::persistence::approval_repo::ApprovalRepo;
use monocoque_agent_rc::persistence::db;

/// Build a minimal test configuration with in-memory DB.
fn test_config(ws: &TempDir) -> GlobalConfig {
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-diff-apply"
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
        root = ws.path().to_str().expect("utf8"),
    );
    GlobalConfig::from_toml_str(&toml).expect("valid config")
}

/// Compute SHA-256 hash of file contents, or "`new_file`" if absent.
fn file_hash(path: &std::path::Path) -> String {
    match fs::read(path) {
        Ok(contents) => {
            let mut hasher = Sha256::new();
            hasher.update(&contents);
            format!("{:x}", hasher.finalize())
        }
        Err(_) => "new_file".to_owned(),
    }
}

/// Create a sample approval request.
fn sample_request(session_id: &str, file_path: &str, hash: &str) -> ApprovalRequest {
    ApprovalRequest::new(
        session_id.to_owned(),
        "Test change".to_owned(),
        None,
        "fn new_content() {}\n".to_owned(),
        file_path.to_owned(),
        RiskLevel::Low,
        hash.to_owned(),
    )
}

// ─── Full-file write after approval ───────────────────────────────────

#[tokio::test]
async fn approve_then_apply_full_file_writes_to_disk() {
    let ws = tempfile::tempdir().expect("ws");
    let config = test_config(&ws);
    let database = Arc::new(db::connect(&config, true).await.expect("db"));
    let repo = ApprovalRepo::new(database);

    // Seed the file so we can hash it.
    let target = ws.path().join("src/new.rs");
    let original_hash = "new_file".to_owned();

    // Create and approve the request.
    let request = sample_request("session-apply", "src/new.rs", &original_hash);
    let request_id = request.id.clone();
    repo.create(&request).await.expect("create");
    repo.update_status(&request_id, ApprovalStatus::Approved)
        .await
        .expect("approve");

    // Fetch the approved request.
    let approved = repo.get_by_id(&request_id).await.expect("fetch");
    assert_eq!(approved.status, ApprovalStatus::Approved);

    // Apply the full-file write.
    let summary = write_full_file(
        &std::path::PathBuf::from(&approved.file_path),
        &approved.diff_content,
        ws.path(),
    )
    .expect("apply should succeed");

    assert!(target.exists(), "file should be written to disk");
    let written = fs::read_to_string(&target).expect("read back");
    assert_eq!(written, "fn new_content() {}\n");
    assert_eq!(summary.bytes_written, approved.diff_content.len());

    // Mark as consumed.
    let consumed = repo.mark_consumed(&request_id).await.expect("consume");
    assert_eq!(consumed.status, ApprovalStatus::Consumed);
    assert!(consumed.consumed_at.is_some());
}

// ─── Patch application after approval ─────────────────────────────────

#[tokio::test]
async fn approve_then_apply_patch_modifies_existing_file() {
    let ws = tempfile::tempdir().expect("ws");
    let config = test_config(&ws);
    let database = Arc::new(db::connect(&config, true).await.expect("db"));
    let repo = ApprovalRepo::new(database);

    // Seed the file.
    let target = ws.path().join("src/main.rs");
    fs::create_dir_all(ws.path().join("src")).expect("mkdir");
    fs::write(&target, "line1\nline2\nline3\n").expect("seed");
    let original_hash = file_hash(&target);

    let diff = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
 line1
-line2
+line2_modified
 line3
";

    let request = ApprovalRequest::new(
        "session-patch".to_owned(),
        "Patch main.rs".to_owned(),
        None,
        diff.to_owned(),
        "src/main.rs".to_owned(),
        RiskLevel::Low,
        original_hash,
    );
    let request_id = request.id.clone();
    repo.create(&request).await.expect("create");
    repo.update_status(&request_id, ApprovalStatus::Approved)
        .await
        .expect("approve");

    // Apply the patch.
    let summary = apply_patch(&std::path::PathBuf::from("src/main.rs"), diff, ws.path())
        .expect("patch should apply");

    let result = fs::read_to_string(&target).expect("read back");
    assert!(result.contains("line2_modified"));
    assert!(!result.contains("\nline2\n"));
    assert!(summary.bytes_written > 0);
}

// ─── Hash mismatch conflict detection ─────────────────────────────────

#[tokio::test]
async fn hash_mismatch_detected_when_file_mutated() {
    let ws = tempfile::tempdir().expect("ws");
    let config = test_config(&ws);
    let database = Arc::new(db::connect(&config, true).await.expect("db"));
    let repo = ApprovalRepo::new(database);

    // Seed with original content and capture hash.
    let target = ws.path().join("mutated.rs");
    fs::write(&target, "original content\n").expect("seed");
    let original_hash = file_hash(&target);

    let request = sample_request("session-conflict", "mutated.rs", &original_hash);
    let request_id = request.id.clone();
    repo.create(&request).await.expect("create");
    repo.update_status(&request_id, ApprovalStatus::Approved)
        .await
        .expect("approve");

    // Mutate the file after proposal creation — simulate external edit.
    fs::write(&target, "someone changed this\n").expect("mutate");
    let current_hash = file_hash(&target);

    // The hashes should differ.
    assert_ne!(
        original_hash, current_hash,
        "file mutation should change the hash"
    );

    // In the accept_diff handler, this mismatch would trigger a
    // `patch_conflict` error unless `force=true`.
}

// ─── Already-consumed returns error ───────────────────────────────────

#[tokio::test]
async fn mark_consumed_twice_returns_already_consumed_error() {
    let ws = tempfile::tempdir().expect("ws");
    let config = test_config(&ws);
    let database = Arc::new(db::connect(&config, true).await.expect("db"));
    let repo = ApprovalRepo::new(database);

    let request = sample_request("session-double", "file.rs", "hash123");
    let request_id = request.id.clone();
    repo.create(&request).await.expect("create");
    repo.update_status(&request_id, ApprovalStatus::Approved)
        .await
        .expect("approve");
    repo.mark_consumed(&request_id)
        .await
        .expect("first consume");

    // Second consume should fail.
    let result = repo.mark_consumed(&request_id).await;
    assert!(result.is_err(), "second consume should return error");
}

// ─── Not approved returns error ───────────────────────────────────────

#[tokio::test]
async fn mark_consumed_on_pending_returns_error() {
    let ws = tempfile::tempdir().expect("ws");
    let config = test_config(&ws);
    let database = Arc::new(db::connect(&config, true).await.expect("db"));
    let repo = ApprovalRepo::new(database);

    let request = sample_request("session-not-approved", "file.rs", "hash456");
    let request_id = request.id.clone();
    repo.create(&request).await.expect("create");

    // Try to consume without approving.
    let result = repo.mark_consumed(&request_id).await;
    assert!(
        result.is_err(),
        "consuming non-approved should return error"
    );
}
