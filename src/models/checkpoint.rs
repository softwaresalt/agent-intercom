//! Checkpoint model for session state snapshots.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::progress::ProgressItem;

/// A named snapshot of a session's state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Checkpoint {
    /// Unique record identifier.
    pub id: String,
    /// Owning session identifier.
    pub session_id: String,
    /// Human-readable label (e.g., "before-refactor").
    pub label: Option<String>,
    /// Serialized session state snapshot.
    pub session_state: serde_json::Value,
    /// Map of `file_path` to SHA-256 hash for divergence detection.
    pub file_hashes: HashMap<String, String>,
    /// Workspace root at checkpoint time.
    pub workspace_root: String,
    /// Session's progress snapshot at checkpoint time.
    pub progress_snapshot: Option<Vec<ProgressItem>>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl Checkpoint {
    /// Construct a new checkpoint snapshot.
    #[must_use]
    pub fn new(
        session_id: String,
        label: Option<String>,
        session_state: serde_json::Value,
        file_hashes: HashMap<String, String>,
        workspace_root: String,
        progress_snapshot: Option<Vec<ProgressItem>>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id,
            label,
            session_state,
            file_hashes,
            workspace_root,
            progress_snapshot,
            created_at: Utc::now(),
        }
    }
}
