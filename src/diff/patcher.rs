//! Unified diff patch application utility (T044).
//!
//! Parses a unified diff via `diffy::Patch::from_str`, reads the
//! existing file, applies the patch, and writes the result atomically
//! via [`crate::diff::writer::write_full_file`].

use std::path::Path;

use diffy::{apply as diffy_apply, Patch};

use crate::{AppError, Result};

use super::writer::{write_full_file, WriteSummary};

/// Apply a unified diff patch to an existing file.
///
/// Reads the current file contents, parses the patch, applies it, and
/// writes the result atomically. The target path is validated against
/// the workspace root.
///
/// # Errors
///
/// Returns `AppError::PathViolation` if the path escapes the workspace.
/// Returns `AppError::Diff` if the file cannot be read, the patch cannot
/// be parsed, or the patch does not apply cleanly.
pub fn apply_patch(
    file_path: &Path,
    unified_diff: &str,
    workspace_root: &Path,
) -> Result<WriteSummary> {
    let validated = crate::diff::validate_workspace_path(workspace_root, file_path)?;

    // Read the current file contents.
    let current = std::fs::read_to_string(&validated).map_err(|err| {
        AppError::Diff(format!(
            "failed to read file for patching {}: {err}",
            validated.display()
        ))
    })?;

    // On Windows, files may use CRLF line endings while the submitted unified
    // diff uses LF. `diffy` performs literal string matching on context lines,
    // so a CRLF file will never match an LF patch. Normalize to LF before
    // applying, then restore CRLF in the output if the original used it.
    let has_crlf = current.contains("\r\n");
    let current_lf = if has_crlf {
        current.replace("\r\n", "\n")
    } else {
        current.clone()
    };

    // Also normalize the diff itself — callers may submit diffs with CRLF.
    let diff_lf;
    let unified_diff_lf = if unified_diff.contains("\r\n") {
        diff_lf = unified_diff.replace("\r\n", "\n");
        diff_lf.as_str()
    } else {
        unified_diff
    };

    // Parse the unified diff.
    let patch = Patch::from_str(unified_diff_lf)
        .map_err(|err| AppError::Diff(format!("failed to parse unified diff: {err}")))?;

    // Apply the patch against the LF-normalized source.
    let patched_lf = diffy_apply(&current_lf, &patch).map_err(|err| {
        AppError::Diff(format!(
            "patch does not apply cleanly to {}: {err}",
            validated.display()
        ))
    })?;

    // Restore CRLF if the original file used it.
    let patched = if has_crlf {
        patched_lf.replace('\n', "\r\n")
    } else {
        patched_lf
    };

    // When the patch removes all content, delete the file from disk rather
    // than leaving an empty placeholder. An empty result is the canonical
    // indicator that every line was removed by the diff.
    if patched.is_empty() {
        std::fs::remove_file(&validated).map_err(|err| {
            AppError::Diff(format!(
                "failed to delete file after all content removed {}: {err}",
                validated.display()
            ))
        })?;
        return Ok(WriteSummary {
            path: validated,
            bytes_written: 0,
        });
    }

    // Write the patched content atomically using the validated path.
    write_full_file(&validated, &patched, workspace_root)
}
