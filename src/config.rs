//! Global configuration parsing and validation.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{AppError, Result};

/// Nested Slack configuration required for Socket Mode connectivity.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct SlackConfig {
    /// App-level token used for Socket Mode.
    pub app_token: String,
    /// Bot user token used for posting messages.
    pub bot_token: String,
    /// Default channel where notifications are posted.
    pub channel_id: String,
}

/// Configurable timeout values (seconds) for blocking tool interactions.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct TimeoutConfig {
    /// Approval request timeout.
    pub approval_seconds: u64,
    /// Continuation prompt timeout.
    pub prompt_seconds: u64,
    /// Wait-for-instruction timeout; 0 means no timeout.
    pub wait_seconds: u64,
}

/// Stall detection configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct StallConfig {
    /// Whether stall detection is enabled.
    pub enabled: bool,
    /// Idle threshold before triggering alert.
    pub inactivity_threshold_seconds: u64,
    /// Delay before auto-nudging when unattended.
    pub escalation_threshold_seconds: u64,
    /// Maximum consecutive auto-nudges before escalation.
    pub max_retries: u32,
    /// Default nudge message delivered to the agent.
    pub default_nudge_message: String,
}

/// Global configuration parsed from `config.toml`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct GlobalConfig {
    /// Absolute workspace root used for path validation.
    pub workspace_root: PathBuf,
    /// Slack connectivity settings.
    pub slack: SlackConfig,
    /// Authorized Slack user IDs allowed to start sessions.
    pub authorized_user_ids: Vec<String>,
    /// Maximum concurrent agent sessions.
    pub max_concurrent_sessions: u32,
    /// Host CLI binary (e.g., `claude`, `gh`).
    pub host_cli: String,
    /// Default arguments for the host CLI.
    pub host_cli_args: Vec<String>,
    /// Registry of allowed commands.
    pub commands: HashMap<String, String>,
    /// HTTP port for the SSE transport.
    pub http_port: u16,
    /// Named pipe / Unix socket identifier.
    pub ipc_name: String,
    /// Timeout configuration for blocking flows.
    pub timeouts: TimeoutConfig,
    /// Stall detection thresholds and behavior.
    pub stall: StallConfig,
}

impl GlobalConfig {
    /// Load and validate configuration from a TOML file path.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Config` if the file cannot be read or contains
    /// invalid TOML, or if validation fails.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .map_err(|err| AppError::Config(format!("failed to read config: {err}")))?;
        Self::from_toml_str(&raw)
    }

    /// Parse configuration from a TOML string and normalize paths.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Config` if parsing or validation fails.
    pub fn from_toml_str(raw: &str) -> Result<Self> {
        let mut config: Self = toml::from_str(raw)?;
        config.validate()?;
        Ok(config)
    }

    /// Absolute path to the workspace root.
    #[must_use]
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Derived path for persisted `SurrealDB` data when using `RocksDB`.
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.workspace_root.join(".monocoque").join("db")
    }

    /// Validate that a Slack user is authorized to manage sessions.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Unauthorized` if the user is not in the allowed list.
    pub fn ensure_authorized(&self, user_id: &str) -> Result<()> {
        if self.authorized_user_ids.iter().any(|id| id == user_id) {
            Ok(())
        } else {
            Err(AppError::Unauthorized("user is not authorized".into()))
        }
    }

    fn validate(&mut self) -> Result<()> {
        if self.max_concurrent_sessions == 0 {
            return Err(AppError::Config(
                "max_concurrent_sessions must be greater than zero".into(),
            ));
        }

        if self.authorized_user_ids.is_empty() {
            return Err(AppError::Config(
                "authorized_user_ids must not be empty".into(),
            ));
        }

        let canonical_root = self
            .workspace_root
            .canonicalize()
            .map_err(|err| AppError::Config(format!("workspace_root invalid: {err}")))?;
        self.workspace_root = canonical_root;

        Ok(())
    }
}
