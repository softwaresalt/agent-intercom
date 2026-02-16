//! Progress tracking types for agent task snapshots.

use serde::{Deserialize, Serialize};

use crate::{AppError, Result};

/// Status of a single progress tracking item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProgressStatus {
    /// Task completed.
    Done,
    /// Task currently executing.
    InProgress,
    /// Task not yet started.
    Pending,
}

/// A single item in an agent's progress snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ProgressItem {
    /// Human-readable task description.
    pub label: String,
    /// Current status of the task.
    pub status: ProgressStatus,
}

/// Validate that a progress snapshot contains well-formed items.
///
/// # Errors
///
/// Returns `AppError::Config` if any item has an empty label.
pub fn validate_snapshot(items: &[ProgressItem]) -> Result<()> {
    for item in items {
        if item.label.trim().is_empty() {
            return Err(AppError::Config(
                "progress item label must not be empty".into(),
            ));
        }
    }
    Ok(())
}
