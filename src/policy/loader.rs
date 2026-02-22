//! Workspace policy file loader (T061).
//!
//! Parses `.agentrc/settings.json` from a workspace root into a
//! [`WorkspacePolicy`]. On parse errors, returns a deny-all default
//! and emits a tracing warning. Validates that workspace `commands`
//! entries exist in the global allowlist (FR-011).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use tracing::warn;

use crate::models::policy::WorkspacePolicy;
use crate::Result;

/// Relative path within a workspace root to the policy file.
const POLICY_PATH: &str = ".agentrc/settings.json";

/// Loads and validates a workspace policy.
pub struct PolicyLoader;

impl PolicyLoader {
    /// Load a [`WorkspacePolicy`] from `{workspace_root}/.agentrc/settings.json`.
    ///
    /// # Behaviour
    ///
    /// - **Missing file or directory**: returns `WorkspacePolicy::default()` (deny-all).
    /// - **Malformed JSON**: returns `WorkspacePolicy::default()` and logs a warning.
    /// - **Valid JSON**: parses into `WorkspacePolicy`, then strips any `commands`
    ///   entries that are absent from `global_commands` (FR-011).
    ///
    /// # Errors
    ///
    /// This function returns `Ok` in all cases — policy loading failures are
    /// non-fatal and degrade to deny-all. The `Result` wrapper is preserved
    /// for future extensibility (e.g., I/O errors on paths outside workspace).
    pub fn load(
        workspace_root: &Path,
        global_commands: &HashMap<String, String>,
    ) -> Result<WorkspacePolicy> {
        let policy_file = workspace_root.join(POLICY_PATH);

        if !policy_file.exists() {
            return Ok(WorkspacePolicy::default());
        }

        let raw = match fs::read_to_string(&policy_file) {
            Ok(content) => content,
            Err(err) => {
                warn!(
                    path = %policy_file.display(),
                    %err,
                    "failed to read workspace policy file, falling back to deny-all"
                );
                return Ok(WorkspacePolicy::default());
            }
        };

        if raw.trim().is_empty() {
            warn!(
                path = %policy_file.display(),
                "workspace policy file is empty, falling back to deny-all"
            );
            return Ok(WorkspacePolicy::default());
        }

        let mut policy: WorkspacePolicy = match serde_json::from_str(&raw) {
            Ok(p) => p,
            Err(err) => {
                warn!(
                    path = %policy_file.display(),
                    %err,
                    "malformed workspace policy file, falling back to deny-all"
                );
                return Ok(WorkspacePolicy::default());
            }
        };

        // FR-011: workspace policy cannot introduce commands beyond the global allowlist.
        let original_count = policy.commands.len();
        policy
            .commands
            .retain(|cmd| global_commands.contains_key(cmd));

        let stripped = original_count - policy.commands.len();
        if stripped > 0 {
            warn!(
                stripped_count = stripped,
                "stripped {stripped} workspace commands not present in global allowlist"
            );
        }

        Ok(policy)
    }
}
