//! Path validation and symlink-escape detection.
//!
//! Ensures all file operations stay within the workspace root boundary
//! (FR-006). Canonicalizes paths, rejects `..` traversal, and detects
//! symlink-based escapes.

use std::path::{Component, Path, PathBuf};

use crate::{AppError, Result};

/// Validate that `candidate` resides within `workspace_root`.
///
/// Canonicalizes the workspace root and normalizes the candidate path,
/// rejecting `..` traversal and symlink escapes. Returns the resolved
/// absolute path on success.
///
/// # Errors
///
/// Returns `AppError::PathViolation` if:
/// - The workspace root cannot be canonicalized.
/// - The candidate path contains `..` segments that escape the root.
/// - The resolved path does not start with the workspace root.
/// - The resolved path is a symlink whose target escapes the workspace.
pub fn validate_path(workspace_root: &Path, candidate: impl AsRef<Path>) -> Result<PathBuf> {
    let root = workspace_root
        .canonicalize()
        .map_err(|err| AppError::PathViolation(format!("workspace root invalid: {err}")))?;

    let mut normalized = PathBuf::new();
    for component in candidate.as_ref().components() {
        match component {
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(AppError::PathViolation(
                        "path attempts to escape workspace".into(),
                    ));
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => {
                normalized.clear();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    let absolute = if normalized.is_absolute() {
        normalized
    } else {
        root.join(normalized)
    };

    if !absolute.starts_with(&root) {
        return Err(AppError::PathViolation("path outside workspace".into()));
    }

    // Symlink escape detection: if the path exists, canonicalize resolves
    // symlinks and we verify the final target is still within the root.
    if absolute.exists() {
        let canonical = absolute
            .canonicalize()
            .map_err(|err| AppError::PathViolation(format!("cannot resolve path: {err}")))?;

        if !canonical.starts_with(&root) {
            return Err(AppError::PathViolation(
                "symlink target escapes workspace".into(),
            ));
        }

        Ok(canonical)
    } else {
        Ok(absolute)
    }
}
