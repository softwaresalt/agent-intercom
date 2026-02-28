//! Workspace policy file loader (T061).
//!
//! Parses `.intercom/settings.json` from a workspace root into a
//! [`CompiledWorkspacePolicy`]. On parse errors, returns a deny-all default
//! and emits a tracing warning.

use std::fs;
use std::path::Path;

use tracing::warn;

use crate::models::policy::{CompiledWorkspacePolicy, WorkspacePolicy};
use crate::Result;

/// Relative path within a workspace root to the policy file.
const POLICY_PATH: &str = ".intercom/settings.json";

/// Loads and validates a workspace policy.
pub struct PolicyLoader;

impl PolicyLoader {
    /// Load a [`CompiledWorkspacePolicy`] from `{workspace_root}/.intercom/settings.json`.
    ///
    /// # Behaviour
    ///
    /// - **Missing file or directory**: returns `CompiledWorkspacePolicy::deny_all()` (deny-all).
    /// - **Malformed JSON**: returns `CompiledWorkspacePolicy::deny_all()` and logs a warning.
    /// - **Valid JSON**: parses into `CompiledWorkspacePolicy`.
    ///
    /// # Errors
    ///
    /// This function returns `Ok` in all cases — policy loading failures are
    /// non-fatal and degrade to deny-all. The `Result` wrapper is preserved
    /// for future extensibility (e.g., I/O errors on paths outside workspace).
    pub fn load(workspace_root: &Path) -> Result<CompiledWorkspacePolicy> {
        let policy_file = workspace_root.join(POLICY_PATH);

        if !policy_file.exists() {
            return Ok(CompiledWorkspacePolicy::deny_all());
        }

        let raw = match fs::read_to_string(&policy_file) {
            Ok(content) => content,
            Err(err) => {
                warn!(
                    path = %policy_file.display(),
                    %err,
                    "failed to read workspace policy file, falling back to deny-all"
                );
                return Ok(CompiledWorkspacePolicy::deny_all());
            }
        };

        if raw.trim().is_empty() {
            warn!(
                path = %policy_file.display(),
                "workspace policy file is empty, falling back to deny-all"
            );
            return Ok(CompiledWorkspacePolicy::deny_all());
        }

        let policy: WorkspacePolicy = match serde_json::from_str(&raw) {
            Ok(p) => p,
            Err(err) => {
                warn!(
                    path = %policy_file.display(),
                    %err,
                    "malformed workspace policy file, falling back to deny-all"
                );
                return Ok(CompiledWorkspacePolicy::deny_all());
            }
        };

        Ok(CompiledWorkspacePolicy::from_policy(policy))
    }
}
