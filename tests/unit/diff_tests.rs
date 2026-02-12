//! Unit tests for diff application utilities (T107).
//!
//! Tests cover:
//! - Full-file write (new file creation, overwrite existing)
//! - Unified diff patch (clean apply, failed apply)
//! - Atomic write via tempfile
//! - Parent directory creation

use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use monocoque_agent_rem::diff::patcher::apply_patch;
use monocoque_agent_rem::diff::writer::write_full_file;

/// Helper to create a temp workspace directory.
fn workspace() -> TempDir {
    tempfile::tempdir().expect("create temp workspace")
}

// ─── write_full_file: new file creation ───────────────────────────────

#[test]
fn write_full_file_creates_new_file() {
    let ws = workspace();
    let path = PathBuf::from("src/new_module.rs");
    let content = "fn hello() {}\n";

    let summary = write_full_file(&path, content, ws.path()).expect("write should succeed");

    let written = fs::read_to_string(ws.path().join("src/new_module.rs")).expect("read back file");
    assert_eq!(written, content);
    assert_eq!(summary.bytes_written, content.len());
}

#[test]
fn write_full_file_creates_parent_directories() {
    let ws = workspace();
    let path = PathBuf::from("deep/nested/dir/file.rs");
    let content = "// nested\n";

    write_full_file(&path, content, ws.path()).expect("write should succeed");

    assert!(ws.path().join("deep/nested/dir/file.rs").exists());
}

#[test]
fn write_full_file_overwrites_existing_file() {
    let ws = workspace();
    let target = ws.path().join("existing.rs");
    fs::write(&target, "old content").expect("seed file");

    let path = PathBuf::from("existing.rs");
    let new_content = "new content\n";

    let summary = write_full_file(&path, new_content, ws.path()).expect("overwrite should succeed");

    let written = fs::read_to_string(&target).expect("read back");
    assert_eq!(written, new_content);
    assert_eq!(summary.bytes_written, new_content.len());
}

#[test]
fn write_full_file_returns_correct_path() {
    let ws = workspace();
    let path = PathBuf::from("output.rs");
    let content = "data";

    let summary = write_full_file(&path, content, ws.path()).expect("write should succeed");

    // The returned path should be absolute and within the workspace.
    // On Windows, canonicalize adds a `\\?\` prefix, so we compare
    // using the canonicalized workspace root.
    assert!(summary.path.is_absolute());
    let canonical_ws = ws.path().canonicalize().expect("canonicalize ws");
    assert!(
        summary.path.starts_with(&canonical_ws),
        "path {:?} should start with workspace {:?}",
        summary.path,
        canonical_ws
    );
}

#[test]
fn write_full_file_rejects_path_traversal() {
    let ws = workspace();
    let path = PathBuf::from("../../escape.rs");

    let result = write_full_file(&path, "evil", ws.path());
    assert!(result.is_err(), "path traversal should be rejected");
}

#[test]
fn write_full_file_atomic_write_no_partial_content() {
    // Verify that the file either has the full new content or the old content,
    // never partial. We test this by writing to an existing file and verifying
    // the result is exactly the new content.
    let ws = workspace();
    let target = ws.path().join("atomic.rs");
    fs::write(&target, "original").expect("seed");

    let path = PathBuf::from("atomic.rs");
    let new_content = "replaced content that is longer than original";

    write_full_file(&path, new_content, ws.path()).expect("atomic write");
    let result = fs::read_to_string(&target).expect("read back");
    assert_eq!(result, new_content);
}

// ─── apply_patch: unified diff application ────────────────────────────

#[test]
fn apply_patch_applies_clean_unified_diff() {
    let ws = workspace();
    let target = ws.path().join("patched.rs");
    fs::write(&target, "line1\nline2\nline3\n").expect("seed");

    let patch = "\
--- a/patched.rs
+++ b/patched.rs
@@ -1,3 +1,3 @@
 line1
-line2
+line2_modified
 line3
";

    let summary = apply_patch(&PathBuf::from("patched.rs"), patch, ws.path())
        .expect("patch should apply cleanly");

    let result = fs::read_to_string(&target).expect("read back");
    assert!(result.contains("line2_modified"), "patch should be applied");
    assert!(!result.contains("\nline2\n"), "old line should be gone");
    assert!(summary.bytes_written > 0);
}

#[test]
fn apply_patch_fails_on_content_mismatch() {
    let ws = workspace();
    let target = ws.path().join("mismatch.rs");
    fs::write(&target, "completely different content\n").expect("seed");

    let patch = "\
--- a/mismatch.rs
+++ b/mismatch.rs
@@ -1,3 +1,3 @@
 expected_line1
-expected_line2
+new_line2
 expected_line3
";

    let result = apply_patch(&PathBuf::from("mismatch.rs"), patch, ws.path());
    assert!(result.is_err(), "patch against wrong content should fail");
}

#[test]
fn apply_patch_fails_on_nonexistent_file() {
    let ws = workspace();

    let patch = "\
--- a/missing.rs
+++ b/missing.rs
@@ -1 +1 @@
-old
+new
";

    let result = apply_patch(&PathBuf::from("missing.rs"), patch, ws.path());
    assert!(result.is_err(), "patch on non-existent file should fail");
}

#[test]
fn apply_patch_rejects_path_traversal() {
    let ws = workspace();

    let patch = "\
--- a/../../escape.rs
+++ b/../../escape.rs
@@ -1 +1 @@
-old
+new
";

    let result = apply_patch(&PathBuf::from("../../escape.rs"), patch, ws.path());
    assert!(
        result.is_err(),
        "path traversal in patch should be rejected"
    );
}
