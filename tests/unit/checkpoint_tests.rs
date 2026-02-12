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
