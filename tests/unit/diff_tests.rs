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

use agent_intercom::diff::patcher::apply_patch;
use agent_intercom::diff::writer::write_full_file;

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

#[test]
fn apply_patch_deletes_file_when_all_content_removed() {
    // A unified diff that removes every line should delete the file from disk
    // rather than leaving an empty placeholder (Finding 3 from HITL test).
    let ws = workspace();
    let target = ws.path().join("to_delete.txt");
    fs::write(&target, "line1\nline2\n").expect("seed");

    let patch = "\
--- a/to_delete.txt
+++ b/to_delete.txt
@@ -1,2 +0,0 @@
-line1
-line2
";

    let summary =
        apply_patch(&PathBuf::from("to_delete.txt"), patch, ws.path()).expect("apply should succeed");

    assert_eq!(summary.bytes_written, 0, "bytes_written should be 0 for deletion");
    assert!(!target.exists(), "file should be deleted from disk when all content is removed");
}

// ─── apply_patch: CRLF line-ending handling ───────────────────────────

#[test]
fn apply_patch_succeeds_on_crlf_file_with_lf_patch() {
    // Windows files often use CRLF. Patches submitted by agents always use LF.
    // The patcher must normalise before applying and preserve CRLF in output.
    let ws = workspace();
    let target = ws.path().join("crlf.rs");
    // Write a file with CRLF line endings (simulating a Windows source file).
    fs::write(&target, "#![forbid(unsafe_code)]\r\n\r\n//! module doc\r\n").expect("seed");

    // LF-only patch (as submitted by an agent or Copilot).
    let patch = "\
--- a/crlf.rs
+++ b/crlf.rs
@@ -1,3 +1,4 @@
 #![forbid(unsafe_code)]
 
+// added line
 //! module doc
";

    apply_patch(&PathBuf::from("crlf.rs"), patch, ws.path())
        .expect("CRLF file should patch cleanly with LF patch");

    let result = fs::read_to_string(&target).expect("read back");
    // The inserted line should be present.
    assert!(result.contains("added line"), "inserted line should exist");
    // Output should preserve CRLF endings to avoid corrupting Windows source files.
    assert!(result.contains("\r\n"), "output should preserve CRLF line endings");
    // Sanity: no bare LF should remain (every \n should be preceded by \r).
    for (i, b) in result.as_bytes().iter().enumerate() {
        if *b == b'\n' {
            assert_eq!(
                result.as_bytes().get(i.wrapping_sub(1)).copied(),
                Some(b'\r'),
                "bare LF at byte {i} — CRLF restoration failed"
            );
        }
    }
}

#[test]
fn apply_patch_preserves_lf_on_lf_file() {
    // Files that already use LF should not have CRLF injected.
    let ws = workspace();
    let target = ws.path().join("lf.rs");
    fs::write(&target, "fn foo() {}\nfn bar() {}\n").expect("seed");

    let patch = "\
--- a/lf.rs
+++ b/lf.rs
@@ -1,2 +1,3 @@
 fn foo() {}
+fn baz() {}
 fn bar() {}
";

    apply_patch(&PathBuf::from("lf.rs"), patch, ws.path())
        .expect("LF file should patch cleanly");

    let result = fs::read_to_string(&target).expect("read back");
    assert!(result.contains("baz"), "inserted function should be present");
    // No CRLF should appear in a pure-LF file.
    assert!(
        !result.contains("\r\n"),
        "LF-only file should not gain CRLF endings after patch"
    );
}

#[test]
fn apply_patch_crlf_diff_against_crlf_file() {
    // If both the file and the diff have CRLF (e.g., editor submitted diff),
    // the patcher should normalise both and still apply cleanly.
    let ws = workspace();
    let target = ws.path().join("both_crlf.txt");
    fs::write(&target, "alpha\r\nbeta\r\ngamma\r\n").expect("seed");

    // Diff with CRLF in the hunk lines (unusual but possible).
    let patch = "--- a/both_crlf.txt\r\n+++ b/both_crlf.txt\r\n@@ -1,3 +1,3 @@\r\n alpha\r\n-beta\r\n+BETA\r\n gamma\r\n";

    apply_patch(&PathBuf::from("both_crlf.txt"), patch, ws.path())
        .expect("CRLF diff against CRLF file should apply cleanly");

    let result = fs::read_to_string(&target).expect("read back");
    assert!(result.contains("BETA"), "substitution should be applied");
    assert!(result.contains("\r\n"), "CRLF should be preserved in output");
}
