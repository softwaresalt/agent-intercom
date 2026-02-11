//! Diff utilities and path safety.

use std::path::{Path, PathBuf};

use crate::Result;

pub mod applicator;
pub mod path_safety;

/// Validate that `candidate` resides within `workspace_root`, returning an absolute normalized path.
///
/// Delegates to [`path_safety::validate_path`] which also performs symlink
/// escape detection.
///
/// # Errors
///
/// Returns `AppError::PathViolation` if the candidate path escapes the
/// workspace root or cannot be canonicalized.
pub fn validate_workspace_path(
    workspace_root: &Path,
    candidate: impl AsRef<Path>,
) -> Result<PathBuf> {
    path_safety::validate_path(workspace_root, candidate)
}
