//! Integration tests for checkpoint creation and restore with divergence detection.
//!
//! Validates the `checkpoint_manager` module: hash computation, checkpoint
//! persistence, and detection of Modified, Deleted, and Added divergences
//! using real filesystem operations.

use std::sync::Arc;

use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::orchestrator::checkpoint_manager::{
    create_checkpoint, hash_workspace_files, restore_checkpoint, DivergenceKind,
};
use agent_intercom::persistence::checkpoint_repo::CheckpointRepo;
use agent_intercom::persistence::db;
use agent_intercom::persistence::session_repo::SessionRepo;

// ── Checkpoint creation captures file hashes ─────────────────

#[tokio::test]
async fn checkpoint_captures_workspace_file_hashes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    // Create test files in workspace root.
    std::fs::write(root.join("file_a.txt"), "content a").expect("write file_a");
    std::fs::write(root.join("file_b.txt"), "content b").expect("write file_b");

    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&database));

    // Create and activate a session.
    let session = Session::new(
        "U_OWNER".into(),
        root.to_string_lossy().into_owned(),
        None,
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await.expect("create session");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let checkpoint = create_checkpoint(&created.id, Some("test"), &session_repo, &checkpoint_repo)
        .await
        .expect("create checkpoint");

    assert_eq!(checkpoint.file_hashes.len(), 2);
    assert!(checkpoint.file_hashes.contains_key("file_a.txt"));
    assert!(checkpoint.file_hashes.contains_key("file_b.txt"));
}

// ── Restore detects Modified files ───────────────────────────

#[tokio::test]
async fn restore_detects_modified_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    std::fs::write(root.join("file.txt"), "original content").expect("write");

    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        root.to_string_lossy().into_owned(),
        None,
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let checkpoint = create_checkpoint(&created.id, Some("v1"), &session_repo, &checkpoint_repo)
        .await
        .expect("checkpoint");

    // Modify the file.
    std::fs::write(root.join("file.txt"), "modified content").expect("modify");

    let (_restored, divergences) = restore_checkpoint(&checkpoint.id, &checkpoint_repo)
        .await
        .expect("restore");

    assert_eq!(divergences.len(), 1);
    assert_eq!(divergences[0].file_path, "file.txt");
    assert_eq!(divergences[0].kind, DivergenceKind::Modified);
}

// ── Restore detects Deleted files ────────────────────────────

#[tokio::test]
async fn restore_detects_deleted_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    std::fs::write(root.join("will_delete.txt"), "temp content").expect("write");

    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        root.to_string_lossy().into_owned(),
        None,
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let checkpoint = create_checkpoint(&created.id, Some("v1"), &session_repo, &checkpoint_repo)
        .await
        .expect("checkpoint");

    // Delete the file.
    std::fs::remove_file(root.join("will_delete.txt")).expect("delete");

    let (_restored, divergences) = restore_checkpoint(&checkpoint.id, &checkpoint_repo)
        .await
        .expect("restore");

    assert_eq!(divergences.len(), 1);
    assert_eq!(divergences[0].file_path, "will_delete.txt");
    assert_eq!(divergences[0].kind, DivergenceKind::Deleted);
}

// ── Restore detects Added files ──────────────────────────────

#[tokio::test]
async fn restore_detects_added_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    std::fs::write(root.join("existing.txt"), "existing").expect("write");

    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        root.to_string_lossy().into_owned(),
        None,
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let checkpoint = create_checkpoint(&created.id, Some("v1"), &session_repo, &checkpoint_repo)
        .await
        .expect("checkpoint");

    // Add a new file.
    std::fs::write(root.join("new_file.txt"), "new content").expect("add new");

    let (_restored, divergences) = restore_checkpoint(&checkpoint.id, &checkpoint_repo)
        .await
        .expect("restore");

    assert_eq!(divergences.len(), 1);
    assert_eq!(divergences[0].file_path, "new_file.txt");
    assert_eq!(divergences[0].kind, DivergenceKind::Added);
}

// ── Restore with no changes → zero divergences ──────────────

#[tokio::test]
async fn restore_no_changes_zero_divergences() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    std::fs::write(root.join("stable.txt"), "unchanged").expect("write");

    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        root.to_string_lossy().into_owned(),
        None,
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let checkpoint = create_checkpoint(&created.id, Some("v1"), &session_repo, &checkpoint_repo)
        .await
        .expect("checkpoint");

    let (_restored, divergences) = restore_checkpoint(&checkpoint.id, &checkpoint_repo)
        .await
        .expect("restore");

    assert!(
        divergences.is_empty(),
        "no changes should mean no divergences"
    );
}

// ── Multiple divergece types at once ─────────────────────────

#[tokio::test]
async fn restore_detects_multiple_divergence_types() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    std::fs::write(root.join("modify.txt"), "original").expect("write");
    std::fs::write(root.join("delete.txt"), "will go away").expect("write");

    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        root.to_string_lossy().into_owned(),
        None,
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let checkpoint = create_checkpoint(&created.id, Some("v1"), &session_repo, &checkpoint_repo)
        .await
        .expect("checkpoint");

    // Mutate: modify one, delete one, add one.
    std::fs::write(root.join("modify.txt"), "changed").expect("modify");
    std::fs::remove_file(root.join("delete.txt")).expect("delete");
    std::fs::write(root.join("added.txt"), "brand new").expect("add");

    let (_restored, divergences) = restore_checkpoint(&checkpoint.id, &checkpoint_repo)
        .await
        .expect("restore");

    assert_eq!(divergences.len(), 3);

    let kinds: Vec<_> = divergences.iter().map(|d| d.kind).collect();
    assert!(kinds.contains(&DivergenceKind::Modified));
    assert!(kinds.contains(&DivergenceKind::Deleted));
    assert!(kinds.contains(&DivergenceKind::Added));
}

// ── hash_workspace_files: empty directory ────────────────────

#[tokio::test]
async fn hash_workspace_files_empty_dir() {
    let temp = tempfile::tempdir().expect("tempdir");
    let hashes = hash_workspace_files(temp.path()).expect("hash");
    assert!(hashes.is_empty());
}

// ── hash_workspace_files: skips subdirectories ───────────────

#[tokio::test]
async fn hash_workspace_files_skips_subdirs() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    std::fs::write(root.join("file.txt"), "content").expect("write");
    std::fs::create_dir(root.join("subdir")).expect("mkdir");
    std::fs::write(root.join("subdir").join("nested.txt"), "nested").expect("write nested");

    let hashes = hash_workspace_files(root).expect("hash");
    // Only top-level files are hashed (non-recursive).
    assert_eq!(hashes.len(), 1);
    assert!(hashes.contains_key("file.txt"));
}

// ── Nonexistent checkpoint returns not found ─────────────────

#[tokio::test]
async fn restore_nonexistent_checkpoint_errors() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&database));

    let result = restore_checkpoint("nonexistent-id", &checkpoint_repo).await;
    assert!(
        result.is_err(),
        "restoring nonexistent checkpoint should fail"
    );
}
