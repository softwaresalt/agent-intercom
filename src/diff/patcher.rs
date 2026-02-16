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

    // Parse the unified diff.
    let patch = Patch::from_str(unified_diff)
        .map_err(|err| AppError::Diff(format!("failed to parse unified diff: {err}")))?;

    // Apply the patch.
    let patched = diffy_apply(&current, &patch).map_err(|err| {
        AppError::Diff(format!(
            "patch does not apply cleanly to {}: {err}",
            validated.display()
        ))
    })?;

    // Write the patched content atomically using the validated path.
    write_full_file(&validated, &patched, workspace_root)
}
