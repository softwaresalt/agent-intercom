//! Atomic file writing utility (T043).
//!
//! Validates the target path against the workspace root, creates parent
//! directories as needed, and writes content atomically via
//! `tempfile::NamedTempFile::persist()` to avoid partial writes.

use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::{AppError, Result};

/// Summary of a completed file write operation.
#[derive(Debug, Clone)]
pub struct WriteSummary {
    /// Absolute path of the written file.
    pub path: PathBuf,
    /// Number of bytes written.
    pub bytes_written: usize,
}

/// Write `content` to a file at `file_path` (relative to `workspace_root`).
///
/// - Validates the path stays within the workspace root.
/// - Creates parent directories if they do not exist.
/// - Writes to a temporary file then atomically renames (`persist`) to
///   prevent partial writes on crash.
///
/// # Errors
///
/// Returns `AppError::PathViolation` if the path escapes the workspace.
/// Returns `AppError::Diff` on I/O failures (directory creation, temp
/// file write, or rename).
pub fn write_full_file(
    file_path: &Path,
    content: &str,
    workspace_root: &Path,
) -> Result<WriteSummary> {
    let validated = crate::diff::validate_workspace_path(workspace_root, file_path)?;

    // Ensure parent directories exist.
    if let Some(parent) = validated.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            AppError::Diff(format!(
                "failed to create parent directories for {}: {err}",
                validated.display()
            ))
        })?;
    }

    // Write to a temp file in the same directory, then atomically rename.
    let parent = validated
        .parent()
        .ok_or_else(|| AppError::Diff("file path has no parent directory".into()))?;

    let mut tmp = NamedTempFile::new_in(parent)
        .map_err(|err| AppError::Diff(format!("failed to create temporary file: {err}")))?;

    let bytes = content.as_bytes();
    tmp.write_all(bytes)
        .map_err(|err| AppError::Diff(format!("failed to write temporary file: {err}")))?;

    tmp.persist(&validated).map_err(|err| {
        AppError::Diff(format!(
            "failed to persist file to {}: {err}",
            validated.display()
        ))
    })?;

    Ok(WriteSummary {
        path: validated,
        bytes_written: bytes.len(),
    })
}
