//! Unit tests for checkpoint hash comparison (T120).
//!
//! Validates:
//! - Create checkpoint with file hashes → mutate files → restore →
//!   verify divergence warning includes correct file list.
//! - Hash computation for workspace files.
//! - Divergence detection between checkpoint hashes and current files.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

/// Compute SHA-256 hex digest for the given content bytes.
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compute SHA-256 hashes for all regular files in a directory (non-recursive).
fn hash_workspace_files(root: &Path) -> HashMap<String, String> {
    let mut hashes = HashMap::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(content) = fs::read(&path) {
                    let rel = path
                        .file_name()
                        .expect("has filename")
                        .to_string_lossy()
                        .to_string();
                    hashes.insert(rel, sha256_hex(&content));
                }
            }
        }
    }
    hashes
}

/// Compare checkpoint hashes against current file hashes and return diverged file names.
fn find_diverged_files(
    checkpoint_hashes: &HashMap<String, String>,
    current_hashes: &HashMap<String, String>,
) -> Vec<String> {
    let mut diverged = Vec::new();

    for (file, old_hash) in checkpoint_hashes {
        match current_hashes.get(file) {
            Some(new_hash) if new_hash != old_hash => {
                diverged.push(file.clone());
            }
            None => {
                // File was deleted since checkpoint.
                diverged.push(file.clone());
            }
            _ => {}
        }
    }

    // Files added since checkpoint.
    for file in current_hashes.keys() {
        if !checkpoint_hashes.contains_key(file) {
            diverged.push(file.clone());
        }
    }

    diverged.sort();
    diverged
}

#[test]
fn hash_unchanged_files_match() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
    fs::write(dir.path().join("lib.rs"), "pub mod foo;").expect("write");

    let checkpoint_hashes = hash_workspace_files(dir.path());
    let current_hashes = hash_workspace_files(dir.path());

    let diverged = find_diverged_files(&checkpoint_hashes, &current_hashes);
    assert!(diverged.is_empty(), "no files should have diverged");
}

#[test]
fn hash_detects_modified_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
    fs::write(dir.path().join("lib.rs"), "pub mod foo;").expect("write");

    let checkpoint_hashes = hash_workspace_files(dir.path());

    // Mutate one file.
    fs::write(
        dir.path().join("main.rs"),
        "fn main() { println!(\"hi\"); }",
    )
    .expect("write");

    let current_hashes = hash_workspace_files(dir.path());
    let diverged = find_diverged_files(&checkpoint_hashes, &current_hashes);

    assert_eq!(diverged, vec!["main.rs"]);
}

#[test]
fn hash_detects_deleted_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
    fs::write(dir.path().join("lib.rs"), "pub mod foo;").expect("write");

    let checkpoint_hashes = hash_workspace_files(dir.path());

    // Delete one file.
    fs::remove_file(dir.path().join("lib.rs")).expect("remove");

    let current_hashes = hash_workspace_files(dir.path());
    let diverged = find_diverged_files(&checkpoint_hashes, &current_hashes);

    assert_eq!(diverged, vec!["lib.rs"]);
}

#[test]
fn hash_detects_added_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");

    let checkpoint_hashes = hash_workspace_files(dir.path());

    // Add a new file.
    fs::write(dir.path().join("extra.rs"), "// new").expect("write");

    let current_hashes = hash_workspace_files(dir.path());
    let diverged = find_diverged_files(&checkpoint_hashes, &current_hashes);

    assert_eq!(diverged, vec!["extra.rs"]);
}

#[test]
fn hash_detects_multiple_divergences() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("a.rs"), "a").expect("write");
    fs::write(dir.path().join("b.rs"), "b").expect("write");
    fs::write(dir.path().join("c.rs"), "c").expect("write");

    let checkpoint_hashes = hash_workspace_files(dir.path());

    // Modify a.rs, delete b.rs, add d.rs.
    fs::write(dir.path().join("a.rs"), "modified_a").expect("write");
    fs::remove_file(dir.path().join("b.rs")).expect("remove");
    fs::write(dir.path().join("d.rs"), "new_d").expect("write");

    let current_hashes = hash_workspace_files(dir.path());
    let diverged = find_diverged_files(&checkpoint_hashes, &current_hashes);

    assert_eq!(diverged, vec!["a.rs", "b.rs", "d.rs"]);
}

#[test]
fn sha256_hex_produces_correct_digest() {
    // Known SHA-256 of "hello".
    let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    assert_eq!(sha256_hex(b"hello"), expected);
}

#[test]
fn empty_checkpoint_no_divergence() {
    let empty: HashMap<String, String> = HashMap::new();
    let diverged = find_diverged_files(&empty, &empty);
    assert!(diverged.is_empty());
}

#[test]
fn checkpoint_with_no_current_files_all_diverged() {
    let mut checkpoint_hashes = HashMap::new();
    checkpoint_hashes.insert("a.rs".to_owned(), "hash_a".to_owned());
    checkpoint_hashes.insert("b.rs".to_owned(), "hash_b".to_owned());

    let current_hashes = HashMap::new();
    let diverged = find_diverged_files(&checkpoint_hashes, &current_hashes);

    assert_eq!(diverged, vec!["a.rs", "b.rs"]);
}

// ── CheckpointRepo CRUD tests (T020) ────────────────────────────────────

use std::sync::Arc;

use monocoque_agent_rc::models::checkpoint::Checkpoint;
use monocoque_agent_rc::persistence::{checkpoint_repo::CheckpointRepo, db};

fn sample_checkpoint(session_id: &str) -> Checkpoint {
    let mut hashes = HashMap::new();
    hashes.insert("main.rs".to_owned(), sha256_hex(b"fn main() {}"));
    Checkpoint::new(
        session_id.to_owned(),
        Some("test-label".to_owned()),
        serde_json::json!({"status": "active"}),
        hashes,
        "/tmp/workspace".to_owned(),
        None,
    )
}

#[tokio::test]
async fn repo_create_persists_all_fields() {
    let db = db::connect_memory().await.expect("db");
    let repo = CheckpointRepo::new(Arc::new(db));

    let checkpoint = sample_checkpoint("sess-1");
    let id = checkpoint.id.clone();
    let created = repo.create(&checkpoint).await.expect("create");

    assert_eq!(created.id, id);
    assert_eq!(created.session_id, "sess-1");
    assert_eq!(created.label, Some("test-label".to_owned()));
    assert_eq!(created.workspace_root, "/tmp/workspace");
    assert!(created.file_hashes.contains_key("main.rs"));
}

#[tokio::test]
async fn repo_get_by_id_returns_none_for_missing() {
    let db = db::connect_memory().await.expect("db");
    let repo = CheckpointRepo::new(Arc::new(db));

    let result = repo.get_by_id("nonexistent").await.expect("query");
    assert!(result.is_none());
}

#[tokio::test]
async fn repo_get_by_id_round_trips() {
    let db = db::connect_memory().await.expect("db");
    let repo = CheckpointRepo::new(Arc::new(db));

    let checkpoint = sample_checkpoint("sess-2");
    let id = checkpoint.id.clone();
    repo.create(&checkpoint).await.expect("create");

    let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(fetched.label, Some("test-label".to_owned()));
    assert_eq!(
        fetched.session_state,
        serde_json::json!({"status": "active"})
    );
    assert!(fetched.file_hashes.contains_key("main.rs"));
}

#[tokio::test]
async fn repo_list_for_session_returns_session_checkpoints() {
    let db = db::connect_memory().await.expect("db");
    let repo = CheckpointRepo::new(Arc::new(db));

    let c1 = sample_checkpoint("sess-3");
    let c2 = sample_checkpoint("sess-3");
    let c3 = sample_checkpoint("sess-other");
    repo.create(&c1).await.expect("create1");
    repo.create(&c2).await.expect("create2");
    repo.create(&c3).await.expect("create3");

    let list = repo.list_for_session("sess-3").await.expect("list");
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|c| c.session_id == "sess-3"));
}

#[tokio::test]
async fn repo_delete_for_session_removes_all() {
    let db = db::connect_memory().await.expect("db");
    let repo = CheckpointRepo::new(Arc::new(db));

    let c1 = sample_checkpoint("sess-4");
    let c2 = sample_checkpoint("sess-4");
    let c3 = sample_checkpoint("sess-other");
    let c3_id = c3.id.clone();
    repo.create(&c1).await.expect("create1");
    repo.create(&c2).await.expect("create2");
    repo.create(&c3).await.expect("create3");

    repo.delete_for_session("sess-4").await.expect("delete");

    let remaining = repo.list_for_session("sess-4").await.expect("list");
    assert!(remaining.is_empty());

    // Other session's checkpoint should be untouched.
    let other = repo.get_by_id(&c3_id).await.expect("query");
    assert!(other.is_some());
}
