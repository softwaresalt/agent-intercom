//! Unit tests for original file attachment logic in `ask_approval` (T081, T082).
//!
//! Scenarios covered:
//! - S087: Existing file — read returns content for Slack upload
//! - S088: New-file hash — read returns `None` (skip upload)
//! - S089: Large existing file — content returned regardless of size
//! - S090: Small existing file — content returned
//! - S091: File deleted after hash computation — returns `None` with warning
//! - S093: File unreadable (not found) — returns `None` gracefully

use agent_intercom::mcp::tools::ask_approval::read_original_file_for_attachment;
use std::io::Write;
use tempfile::NamedTempFile;

// ─── S087: Existing file returns content ─────────────────────────────────────

/// S087 — When `original_hash` is not `"new_file"` and the file exists,
/// `read_original_file_for_attachment` returns `Some(content)`.
#[tokio::test]
async fn existing_file_returns_content() {
    let mut tmp = NamedTempFile::new().expect("tempfile");
    tmp.write_all(b"hello world").expect("write");
    let path = tmp.path().to_owned();

    let result = read_original_file_for_attachment(&path, "some_sha256_hash").await;
    assert!(result.is_some(), "should return Some for existing file");
    assert_eq!(result.unwrap(), "hello world");
}

// ─── S088: New-file hash means no original attachment ────────────────────────

/// S088 — When `original_hash` is `"new_file"`, the function returns `None`
/// without attempting to read the file (no original content exists).
#[tokio::test]
async fn new_file_hash_returns_none() {
    // Use a path that doesn't exist — should still return None because of hash check.
    let path = std::path::Path::new("/nonexistent/absolutely_does_not_exist.txt");
    let result = read_original_file_for_attachment(path, "new_file").await;
    assert!(
        result.is_none(),
        "new_file hash should yield None immediately"
    );
}

// ─── S089: Large existing file returns content ───────────────────────────────

/// S089 — For a 150 KB file, the function still returns the content.
/// The decision to upload (vs. inline) is made at the call site, not here.
#[tokio::test]
async fn large_file_returns_content() {
    let large_content: String = "x".repeat(150_000);
    let mut tmp = NamedTempFile::new().expect("tempfile");
    tmp.write_all(large_content.as_bytes()).expect("write");
    let path = tmp.path().to_owned();

    let result = read_original_file_for_attachment(&path, "any_hash").await;
    assert!(result.is_some(), "large file should return Some");
    assert_eq!(result.unwrap().len(), 150_000, "content length must match");
}

// ─── S090: Small existing file returns content ───────────────────────────────

/// S090 — For a 2 KB file, the function returns the content correctly.
#[tokio::test]
async fn small_file_returns_content() {
    let small_content: String = "a".repeat(2_048);
    let mut tmp = NamedTempFile::new().expect("tempfile");
    tmp.write_all(small_content.as_bytes()).expect("write");
    let path = tmp.path().to_owned();

    let result = read_original_file_for_attachment(&path, "some_hash").await;
    assert_eq!(result.as_deref(), Some(small_content.as_str()));
}

// ─── S091: File deleted between hash and upload ───────────────────────────────

/// S091 — When the original file is deleted after the hash is computed,
/// `read_original_file_for_attachment` returns `None` (non-blocking fallback).
#[tokio::test]
async fn deleted_file_returns_none() {
    let path = {
        // Create a temp file and capture its path, then drop (delete) it.
        let tmp = NamedTempFile::new().expect("tempfile");
        let p = tmp.path().to_owned();
        drop(tmp); // file is deleted here
        p
    };

    // File is now gone. With a non-"new_file" hash we should get None.
    let result = read_original_file_for_attachment(&path, "deadbeef1234").await;
    assert!(
        result.is_none(),
        "deleted file should yield None without panic"
    );
}

// ─── S093: File unreadable (not found path) ───────────────────────────────────

/// S093 — When the file path does not exist (simulating a permission/IO error),
/// the function returns `None` gracefully, logging a warning.
#[tokio::test]
async fn unreadable_file_returns_none() {
    let path = std::path::Path::new("/no/such/directory/file.txt");
    let result = read_original_file_for_attachment(path, "cafebabe9876").await;
    assert!(
        result.is_none(),
        "unreadable/missing file should yield None gracefully"
    );
}
