//! Checkpoint creation and restore for session state snapshots.
//!
//! Provides [`create_checkpoint`] to snapshot a session's state and
//! workspace file hashes, and [`restore_checkpoint`] to load a
//! previous checkpoint and detect workspace divergence.

use std::collections::HashMap;
use std::path::Path;

use sha2::{Digest, Sha256};
use tracing::{info, info_span, warn};

use crate::models::checkpoint::Checkpoint;
use crate::persistence::checkpoint_repo::CheckpointRepo;
use crate::persistence::session_repo::SessionRepo;
use crate::{AppError, Result};

/// Create a checkpoint — snapshot session state and workspace file hashes.
///
/// Computes SHA-256 hashes for all regular files (non-recursive) in the
/// session's workspace root and stores them alongside the serialized
/// session state.
///
/// # Errors
///
/// Returns `AppError::Db` if the checkpoint cannot be persisted, or
/// `AppError::Config` if the workspace root is invalid.
pub async fn create_checkpoint(
    session_id: &str,
    label: Option<&str>,
    session_repo: &SessionRepo,
    checkpoint_repo: &CheckpointRepo,
) -> Result<Checkpoint> {
    let span = info_span!("create_checkpoint", session_id, label);
    let _guard = span.enter();

    // Load the current session.
    let session = session_repo.get_by_id(session_id).await?;

    // Compute file hashes for the workspace root.
    let file_hashes = hash_workspace_files(Path::new(&session.workspace_root))?;

    // Serialize session state as JSON for checkpoint storage.
    let session_state = serde_json::to_value(&session)
        .map_err(|err| AppError::Db(format!("failed to serialize session state: {err}")))?;

    let checkpoint = Checkpoint::new(
        session_id.to_owned(),
        label.map(ToOwned::to_owned),
        session_state,
        file_hashes,
        session.workspace_root.clone(),
        session.progress_snapshot.clone(),
    );

    let saved = checkpoint_repo.create(&checkpoint).await?;

    info!(
        session_id,
        checkpoint_id = saved.id,
        files_hashed = saved.file_hashes.len(),
        "checkpoint created"
    );

    Ok(saved)
}

/// Restore a checkpoint — load the checkpoint and detect workspace divergence.
///
/// Compares the checkpoint's stored file hashes against the current workspace
/// state. Returns the checkpoint and a list of files that have diverged since
/// the checkpoint was created.
///
/// # Errors
///
/// Returns `AppError::NotFound` if the checkpoint does not exist, or
/// `AppError::Config` if the workspace root is invalid.
pub async fn restore_checkpoint(
    checkpoint_id: &str,
    checkpoint_repo: &CheckpointRepo,
) -> Result<(Checkpoint, Vec<DivergenceEntry>)> {
    let span = info_span!("restore_checkpoint", checkpoint_id);
    let _guard = span.enter();

    let checkpoint = checkpoint_repo.get_by_id(checkpoint_id).await?;

    // Compute current file hashes for the workspace.
    let current_hashes = hash_workspace_files(Path::new(&checkpoint.workspace_root))?;

    // Find diverged files.
    let divergences = find_divergences(&checkpoint.file_hashes, &current_hashes);

    if divergences.is_empty() {
        info!(checkpoint_id, "no file divergences detected");
    } else {
        warn!(
            checkpoint_id,
            diverged_count = divergences.len(),
            "file divergences detected during restore"
        );
    }

    Ok((checkpoint, divergences))
}

/// A file that has diverged between checkpoint and current workspace state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DivergenceEntry {
    /// Relative file path.
    pub file_path: String,
    /// Kind of divergence.
    pub kind: DivergenceKind,
}

/// The type of file divergence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivergenceKind {
    /// File content has changed.
    Modified,
    /// File existed at checkpoint time but is now missing.
    Deleted,
    /// File was added after the checkpoint.
    Added,
}

/// Compute SHA-256 hashes for all regular files in a directory (non-recursive).
///
/// # Errors
///
/// Returns `AppError::Config` if the directory cannot be read.
pub fn hash_workspace_files(root: &Path) -> Result<HashMap<String, String>> {
    let mut hashes = HashMap::new();

    let entries = std::fs::read_dir(root)
        .map_err(|err| AppError::Config(format!("cannot read workspace directory: {err}")))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Ok(content) = std::fs::read(&path) {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                hashes.insert(rel, sha256_hex(&content));
            }
        }
    }

    Ok(hashes)
}

/// Compare checkpoint hashes against current workspace hashes.
fn find_divergences(
    checkpoint_hashes: &HashMap<String, String>,
    current_hashes: &HashMap<String, String>,
) -> Vec<DivergenceEntry> {
    let mut diverged = Vec::new();

    for (file, old_hash) in checkpoint_hashes {
        match current_hashes.get(file) {
            Some(new_hash) if new_hash != old_hash => {
                diverged.push(DivergenceEntry {
                    file_path: file.clone(),
                    kind: DivergenceKind::Modified,
                });
            }
            None => {
                diverged.push(DivergenceEntry {
                    file_path: file.clone(),
                    kind: DivergenceKind::Deleted,
                });
            }
            _ => {}
        }
    }

    for file in current_hashes.keys() {
        if !checkpoint_hashes.contains_key(file) {
            diverged.push(DivergenceEntry {
                file_path: file.clone(),
                kind: DivergenceKind::Added,
            });
        }
    }

    diverged.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    diverged
}

/// Compute SHA-256 hex digest of the given bytes.
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
