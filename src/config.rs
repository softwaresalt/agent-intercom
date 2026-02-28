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
/// Tokens and team ID are loaded at runtime via OS keychain or environment
/// variables, not from the TOML config file (FR-036).
#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct SlackConfig {
    /// Default channel where notifications are posted.
    ///
    /// Intentionally left empty in `config.toml`. The channel is always
    /// supplied per-workspace via the `?channel_id=` SSE query parameter
    /// in `mcp.json`. Server-level notifications are skipped when empty.
    #[serde(default)]
    pub channel_id: String,
    /// App-level token used for Socket Mode (populated at runtime).
    #[serde(skip)]
    pub app_token: String,
    /// Bot user token used for posting messages (populated at runtime).
    #[serde(skip)]
    pub bot_token: String,
    /// Slack workspace team ID (populated at runtime).
    #[serde(skip)]
    pub team_id: String,
    /// File extensions that should be uploaded to Slack as markdown-fenced
    /// `.md` files so that Slack renders them as text instead of "Binary".
    ///
    /// Keys are bare extensions (without leading dot, e.g. `rs`, `toml`);
    /// values are the markdown code-fence language label (e.g. `rust`, `toml`).
    /// Files whose extension is NOT in this map are uploaded as plain `.txt`.
    #[serde(default)]
    pub markdown_upload_extensions: HashMap<String, String>,
}

impl std::fmt::Debug for SlackConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackConfig")
            .field("channel_id", &self.channel_id)
            .field("app_token", &"[REDACTED]")
            .field("bot_token", &"[REDACTED]")
            .field("team_id", &self.team_id)
            .field(
                "markdown_upload_extensions",
                &self.markdown_upload_extensions,
            )
            .finish()
    }
}

impl SlackConfig {
    /// Look up the markdown code-fence language label for a file path.
    ///
    /// Returns `Some("rust")` for a path ending in `.rs` when `rs = "rust"`
    /// is present in `[slack.markdown_upload_extensions]`.  Returns `None`
    /// when the extension is absent from the map (the file should be
    /// uploaded as plain `.txt` instead).
    pub fn markdown_fence_label(&self, file_path: &str) -> Option<&str> {
        let ext = Path::new(file_path).extension()?.to_str()?;
        self.markdown_upload_extensions.get(ext).map(String::as_str)
    }
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
    "agent-intercom".into()
}

/// Database configuration for the `SQLite` persistence layer.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct DatabaseConfig {
    /// Relative or absolute path to the `SQLite` database file.
    ///
    /// The `connect()` function auto-creates parent directories if they
    /// do not exist. Defaults to `data/agent-rc.db`.
    #[serde(default = "default_db_path")]
    pub path: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("data/agent-rc.db"),
        }
    }
}

fn default_db_path() -> PathBuf {
    PathBuf::from("data/agent-rc.db")
}

/// Verbosity level for Slack status messages.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SlackDetailLevel {
    /// Minimal output — errors and key events only.
    Minimal,
    /// Standard output — normal operational messages (default).
    #[default]
    Standard,
    /// Verbose output — all events including auto-approved actions.
    Verbose,
}

fn default_slack_detail_level() -> SlackDetailLevel {
    SlackDetailLevel::Standard
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
    ///
    /// Populated at runtime from the `SLACK_MEMBER_IDS` environment
    /// variable via [`GlobalConfig::load_authorized_users`]. Not read from `config.toml`.
    #[serde(skip)]
    pub authorized_user_ids: Vec<String>,
    /// Maximum concurrent agent sessions.
    #[serde(default = "default_max_concurrent_sessions")]
    pub max_concurrent_sessions: u32,
    /// Host CLI binary (e.g., `claude`, `gh`).
    pub host_cli: String,
    /// Default arguments for the host CLI.
    #[serde(default)]
    pub host_cli_args: Vec<String>,
    /// Registry of Slack slash-command aliases for the `/run` command (FR-014).
    ///
    /// Maps a short alias (e.g. `status`) to a shell command string (e.g. `git status -s`).
    /// Invoked by the Slack command handler only — has no effect on MCP
    /// auto-approve policy (see ADR-0012).
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
    /// Database configuration.
    #[serde(default)]
    pub database: DatabaseConfig,
    /// Verbosity level for Slack status messages.
    ///
    /// Controls how much detail is posted to Slack during agent sessions.
    #[serde(default = "default_slack_detail_level")]
    pub slack_detail_level: SlackDetailLevel,
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

    /// Load Slack credentials from OS keychain with env-var fallback, and load
    /// authorized user IDs from `SLACK_MEMBER_IDS`.
    ///
    /// Tries the `agent-intercom` keyring service first for Slack tokens,
    /// then falls back to `SLACK_APP_TOKEN` / `SLACK_BOT_TOKEN` environment
    /// variables. `SLACK_TEAM_ID` is optional (FR-041) and will not cause an
    /// error if absent.
    ///
    /// Authorized user IDs are always read from `SLACK_MEMBER_IDS`
    /// (comma-separated Slack user IDs, e.g. `U0123456789,U9876543210`).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Config` if neither keychain nor env vars provide
    /// the required Slack tokens, or if `SLACK_MEMBER_IDS` is
    /// absent or empty.
    pub async fn load_credentials(&mut self) -> Result<()> {
        let _span = tracing::info_span!("load_credentials").entered();
        self.slack.app_token = load_credential("slack_app_token", "SLACK_APP_TOKEN").await?;
        self.slack.bot_token = load_credential("slack_bot_token", "SLACK_BOT_TOKEN").await?;
        // SLACK_TEAM_ID is optional per FR-041 — absence is not an error.
        self.slack.team_id = load_optional_credential("slack_team_id", "SLACK_TEAM_ID").await;
        self.load_authorized_users()?;
        Ok(())
    }

    /// Load authorized Slack user IDs from the `SLACK_MEMBER_IDS`
    /// environment variable.
    ///
    /// The variable must contain a comma-separated list of Slack user IDs
    /// (e.g., `U0123456789,U9876543210`). Whitespace around each entry is
    /// trimmed and empty entries are ignored.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Config` if the variable is absent, empty, or
    /// resolves to an empty list after trimming.
    ///
    /// # Note
    ///
    /// This function is `pub` to allow direct testing from the integration
    /// test crate.  It is an internal implementation detail of
    /// [`load_credentials`] and is not part of the public API contract.
    #[doc(hidden)]
    pub fn load_authorized_users(&mut self) -> Result<()> {
        let raw = env::var("SLACK_MEMBER_IDS").unwrap_or_default();
        let ids: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        if ids.is_empty() {
            return Err(AppError::Config(
                "no authorized user IDs found: set SLACK_MEMBER_IDS to a \
                 comma-separated list of Slack user IDs (e.g. U0123456789,U9876543210)"
                    .into(),
            ));
        }
        self.authorized_user_ids = ids;
        Ok(())
    }

    /// Absolute path to the default workspace root.
    #[must_use]
    pub fn default_workspace_root(&self) -> &Path {
        &self.default_workspace_root
    }

    /// Configured path to the `SQLite` database file.
    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.database.path
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

        let canonical_root = self
            .default_workspace_root
            .canonicalize()
            .map_err(|err| AppError::Config(format!("default_workspace_root invalid: {err}")))?;
        self.default_workspace_root = canonical_root;

        Ok(())
    }
}

/// Keychain service identifier used for credential storage.
const KEYCHAIN_SERVICE: &str = "agent-intercom";

/// Load a single credential from OS keychain with env-var fallback.
///
/// Resolution order:
/// 1. OS keychain service `agent-intercom`, key `{keyring_key}`
/// 2. Environment variable `{env_key}`
///
/// Empty values from either source are treated as absent.
///
/// # Errors
///
/// Returns `AppError::Config` with a message naming both the keychain
/// service and the environment variable so the operator knows exactly
/// which sources were checked.
async fn load_credential(keyring_key: &str, env_key: &str) -> Result<String> {
    let key = keyring_key.to_owned();
    let _span = tracing::info_span!("load_credential", key = keyring_key, env = env_key).entered();

    // Try OS keychain first via spawn_blocking (keyring is synchronous I/O).
    let keychain_result = tokio::task::spawn_blocking(move || {
        keyring::Entry::new(KEYCHAIN_SERVICE, &key).and_then(|entry| entry.get_password())
    })
    .await
    .map_err(|err| AppError::Config(format!("keychain task panicked: {err}")))?;

    match keychain_result {
        Ok(value) if !value.is_empty() => {
            tracing::info!(key = keyring_key, source = "keychain", "credential loaded");
            return Ok(value);
        }
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

    // Fallback to environment variable (empty value treated as absent).
    match env::var(env_key) {
        Ok(value) if !value.is_empty() => {
            tracing::info!(key = keyring_key, source = "env", "credential loaded");
            Ok(value)
        }
        _ => Err(AppError::Config(format!(
            "credential `{keyring_key}` not found: checked keychain service \
             `{KEYCHAIN_SERVICE}` and environment variable `{env_key}`"
        ))),
    }
}

/// Load an optional credential — returns an empty string if absent.
///
/// Uses the same resolution order as [`load_credential`] but never fails.
async fn load_optional_credential(keyring_key: &str, env_key: &str) -> String {
    if let Ok(value) = load_credential(keyring_key, env_key).await {
        value
    } else {
        tracing::info!(
            key = keyring_key,
            "optional credential not found, using empty default"
        );
        String::new()
    }
}
