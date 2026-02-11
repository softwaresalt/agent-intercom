//! Global configuration parsing, validation, and credential loading.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::warn;

use crate::{AppError, Result};

/// Nested Slack configuration for Socket Mode connectivity.
///
/// Tokens are loaded at runtime via OS keychain or environment variables,
/// not from the TOML config file (FR-036).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct SlackConfig {
    /// Default channel where notifications are posted.
    pub channel_id: String,
    /// App-level token used for Socket Mode (populated at runtime).
    #[serde(skip)]
    pub app_token: String,
    /// Bot user token used for posting messages (populated at runtime).
    #[serde(skip)]
    pub bot_token: String,
}

/// Configurable timeout values (seconds) for blocking tool interactions.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct TimeoutConfig {
    /// Approval request timeout.
    #[serde(default = "default_approval_seconds")]
    pub approval_seconds: u64,
    /// Continuation prompt timeout.
    #[serde(default = "default_prompt_seconds")]
    pub prompt_seconds: u64,
    /// Wait-for-instruction timeout; 0 means no timeout.
    #[serde(default)]
    pub wait_seconds: u64,
}

fn default_approval_seconds() -> u64 {
    3600
}

fn default_prompt_seconds() -> u64 {
    1800
}

/// Stall detection configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct StallConfig {
    /// Whether stall detection is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Idle threshold before triggering alert.
    #[serde(default = "default_inactivity_threshold")]
    pub inactivity_threshold_seconds: u64,
    /// Delay before auto-nudging when unattended.
    #[serde(default = "default_escalation_threshold")]
    pub escalation_threshold_seconds: u64,
    /// Maximum consecutive auto-nudges before escalation.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Default nudge message delivered to the agent.
    #[serde(default = "default_nudge_message")]
    pub default_nudge_message: String,
}

fn default_true() -> bool {
    true
}

fn default_inactivity_threshold() -> u64 {
    300
}

fn default_escalation_threshold() -> u64 {
    120
}

fn default_max_retries() -> u32 {
    3
}

fn default_nudge_message() -> String {
    "Continue working on the current task. Pick up where you left off.".into()
}

fn default_retention_days() -> u32 {
    30
}

fn default_max_concurrent_sessions() -> u32 {
    3
}

fn default_http_port() -> u16 {
    3000
}

fn default_ipc_name() -> String {
    "monocoque-agent-rem".into()
}

/// Global configuration parsed from `config.toml`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct GlobalConfig {
    /// Default workspace root used for the primary stdio agent.
    pub default_workspace_root: PathBuf,
    /// Slack connectivity settings.
    pub slack: SlackConfig,
    /// Authorized Slack user IDs allowed to start sessions.
    pub authorized_user_ids: Vec<String>,
    /// Maximum concurrent agent sessions.
    #[serde(default = "default_max_concurrent_sessions")]
    pub max_concurrent_sessions: u32,
    /// Host CLI binary (e.g., `claude`, `gh`).
    pub host_cli: String,
    /// Default arguments for the host CLI.
    #[serde(default)]
    pub host_cli_args: Vec<String>,
    /// Registry of allowed commands.
    #[serde(default)]
    pub commands: HashMap<String, String>,
    /// HTTP port for the SSE transport.
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    /// Named pipe / Unix socket identifier.
    #[serde(default = "default_ipc_name")]
    pub ipc_name: String,
    /// Timeout configuration for blocking flows.
    pub timeouts: TimeoutConfig,
    /// Stall detection thresholds and behavior.
    pub stall: StallConfig,
    /// Days after session termination before data is purged.
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
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

    /// Load Slack credentials from OS keychain with env-var fallback.
    ///
    /// Tries the `monocoque-agent-rem` keyring service first, then falls
    /// back to `SLACK_APP_TOKEN` / `SLACK_BOT_TOKEN` environment variables.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Config` if neither keychain nor env vars provide
    /// the required tokens.
    pub async fn load_credentials(&mut self) -> Result<()> {
        self.slack.app_token = load_credential("slack_app_token", "SLACK_APP_TOKEN").await?;
        self.slack.bot_token = load_credential("slack_bot_token", "SLACK_BOT_TOKEN").await?;
        Ok(())
    }

    /// Absolute path to the default workspace root.
    #[must_use]
    pub fn default_workspace_root(&self) -> &Path {
        &self.default_workspace_root
    }

    /// Derived path for persisted `SurrealDB` data when using `RocksDB`.
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.default_workspace_root.join(".monocoque").join("db")
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
            .default_workspace_root
            .canonicalize()
            .map_err(|err| AppError::Config(format!("default_workspace_root invalid: {err}")))?;
        self.default_workspace_root = canonical_root;

        Ok(())
    }
}

/// Load a single credential from OS keychain with env-var fallback.
async fn load_credential(keyring_key: &str, env_key: &str) -> Result<String> {
    let key = keyring_key.to_owned();

    // Try OS keychain first via spawn_blocking (keyring is synchronous I/O).
    let keychain_result = tokio::task::spawn_blocking(move || {
        keyring::Entry::new("monocoque-agent-rem", &key).and_then(|entry| entry.get_password())
    })
    .await
    .map_err(|err| AppError::Config(format!("keychain task panicked: {err}")))?;

    match keychain_result {
        Ok(value) if !value.is_empty() => return Ok(value),
        Ok(_) => {
            warn!(key = keyring_key, "keychain entry is empty, trying env var");
        }
        Err(err) => {
            warn!(
                key = keyring_key,
                ?err,
                "keychain lookup failed, trying env var"
            );
        }
    }

    // Fallback to environment variable.
    env::var(env_key).map_err(|_| {
        AppError::Config(format!(
            "credential {keyring_key} not found in keychain or {env_key} env var"
        ))
    })
}
