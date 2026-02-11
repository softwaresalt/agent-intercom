//! Diff utilities.

use std::path::{Component, Path, PathBuf};

use crate::{AppError, Result};

pub mod applicator;

/// Validate that `candidate` resides within `workspace_root`, returning an absolute normalized path.
///
/// # Errors
///
/// Returns `AppError::PathViolation` if the candidate path escapes the
/// workspace root or cannot be canonicalized.
pub fn validate_workspace_path(
    workspace_root: &Path,
    candidate: impl AsRef<Path>,
) -> Result<PathBuf> {
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

    if absolute.starts_with(&root) {
        Ok(absolute)
    } else {
        Err(AppError::PathViolation("path outside workspace".into()))
    }
}
