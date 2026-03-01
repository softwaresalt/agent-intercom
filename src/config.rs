//! Global configuration parsing, validation, and credential loading.

use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::mode::ServerMode;
use crate::{AppError, Result};

/// A single workspace-to-channel mapping entry configured via `[[workspace]]`.
///
/// Each entry maps a short `workspace_id` string (e.g. `"my-repo"`) to a
/// Slack `channel_id` so that agents connecting with
/// `?workspace_id=my-repo` are automatically routed to the correct channel.
///
/// # Examples
///
/// ```toml
/// [[workspace]]
/// workspace_id = "my-repo"
/// channel_id   = "C0123456789"
/// label        = "My Repository"
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct WorkspaceMapping {
    /// Short identifier supplied by agents as `?workspace_id=<id>`.
    ///
    /// Must be non-empty and unique within the `[[workspace]]` list.
    pub workspace_id: String,
    /// Slack channel ID that messages for this workspace are routed to.
    ///
    /// Must be non-empty.
    pub channel_id: String,
    /// Optional human-readable label shown in logs and Slack messages.
    pub label: Option<String>,
}

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

fn default_acp_max_sessions() -> usize {
    5
}

fn default_acp_startup_timeout_seconds() -> u64 {
    30
}

/// ACP-mode specific configuration.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct AcpConfig {
    /// Maximum number of concurrent ACP sessions.
    ///
    /// Requests beyond this limit are rejected with a descriptive error.
    /// Defaults to `5`.
    #[serde(default = "default_acp_max_sessions")]
    pub max_sessions: usize,
    /// Seconds to wait for the agent to emit its ready signal on stdout.
    ///
    /// If no line is received within this window the spawner kills the
    /// process and returns `AppError::Acp("startup timeout")`. Defaults
    /// to `30`.
    #[serde(default = "default_acp_startup_timeout_seconds")]
    pub startup_timeout_seconds: u64,
}

impl Default for AcpConfig {
    fn default() -> Self {
        Self {
            max_sessions: default_acp_max_sessions(),
            startup_timeout_seconds: default_acp_startup_timeout_seconds(),
        }
    }
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
    /// ACP-mode configuration (max sessions, startup timeout).
    #[serde(default)]
    pub acp: AcpConfig,
    /// Workspace-to-channel routing table.
    ///
    /// Each `[[workspace]]` entry in `config.toml` maps a `workspace_id`
    /// (the string passed by agents as `?workspace_id=…`) to a Slack
    /// `channel_id`.  The list is validated for uniqueness and non-empty
    /// identifiers during [`GlobalConfig::from_toml_str`].
    ///
    /// Hot-reload support: changes to `config.toml` take effect for new
    /// sessions when a [`crate::config_watcher::ConfigWatcher`] is active.
    #[serde(default, rename = "workspace")]
    pub workspaces: Vec<WorkspaceMapping>,
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
    /// When `mode` is [`ServerMode::Acp`], mode-prefixed sources are tried
    /// first (keychain service `agent-intercom-acp`, env vars with `_ACP`
    /// suffix) before falling back to the shared names. This allows two
    /// server instances (one MCP, one ACP) to run on the same machine with
    /// independent Slack app credentials. See ADR-0015.
    ///
    /// Resolution order per credential:
    /// 1. Keychain `agent-intercom-{mode}` / `{key}`
    /// 2. Env var `{ENV_VAR}_{MODE}`
    /// 3. Keychain `agent-intercom` / `{key}` (shared fallback)
    /// 4. Env var `{ENV_VAR}` (shared fallback)
    ///
    /// # Errors
    ///
    /// Returns `AppError::Config` if neither keychain nor env vars provide
    /// the required Slack tokens, or if `SLACK_MEMBER_IDS` is
    /// absent or empty.
    pub async fn load_credentials(&mut self, mode: ServerMode) -> Result<()> {
        let _span = tracing::info_span!("load_credentials", ?mode).entered();
        self.slack.app_token = load_credential("slack_app_token", "SLACK_APP_TOKEN", mode).await?;
        self.slack.bot_token = load_credential("slack_bot_token", "SLACK_BOT_TOKEN", mode).await?;
        // SLACK_TEAM_ID is optional per FR-041 — absence is not an error.
        self.slack.team_id = load_optional_credential("slack_team_id", "SLACK_TEAM_ID", mode).await;
        self.load_authorized_users(mode)?;
        Ok(())
    }

    /// Load authorized Slack user IDs from the `SLACK_MEMBER_IDS`
    /// environment variable, with mode-prefixed fallback.
    ///
    /// Resolution order:
    /// 1. `SLACK_MEMBER_IDS_{MODE}` (e.g. `SLACK_MEMBER_IDS_ACP`)
    /// 2. `SLACK_MEMBER_IDS` (shared fallback)
    ///
    /// The variable must contain a comma-separated list of Slack user IDs
    /// (e.g., `U0123456789,U9876543210`). Whitespace around each entry is
    /// trimmed and empty entries are ignored.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Config` if both variables are absent, empty, or
    /// resolve to an empty list after trimming.
    ///
    /// # Note
    ///
    /// This function is `pub` to allow direct testing from the integration
    /// test crate.  It is an internal implementation detail of
    /// [`load_credentials`] and is not part of the public API contract.
    #[doc(hidden)]
    pub fn load_authorized_users(&mut self, mode: ServerMode) -> Result<()> {
        let mode_suffix = mode_env_suffix(mode);
        let mode_env = format!("SLACK_MEMBER_IDS{mode_suffix}");

        // Try mode-prefixed variable first, then shared fallback.
        let raw = env::var(&mode_env)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| env::var("SLACK_MEMBER_IDS").ok().filter(|v| !v.is_empty()))
            .unwrap_or_default();

        let ids: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        if ids.is_empty() {
            return Err(AppError::Config(format!(
                "no authorized user IDs found: set {mode_env} or SLACK_MEMBER_IDS to a \
                 comma-separated list of Slack user IDs (e.g. U0123456789,U9876543210)"
            )));
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

        self.validate_workspace_mappings()?;

        Ok(())
    }

    /// Validate workspace-to-channel mapping entries.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Config` when:
    /// - any `workspace_id` or `channel_id` is empty
    /// - `workspace_id` values are not unique within the list
    pub fn validate_workspace_mappings(&self) -> Result<()> {
        let mut seen: HashSet<&str> = HashSet::new();
        for mapping in &self.workspaces {
            if mapping.workspace_id.is_empty() {
                return Err(AppError::Config(
                    "workspace_id cannot be empty in [[workspace]] entry".into(),
                ));
            }
            if mapping.channel_id.is_empty() {
                return Err(AppError::Config(
                    "channel_id cannot be empty in [[workspace]] entry".into(),
                ));
            }
            if !seen.insert(mapping.workspace_id.as_str()) {
                return Err(AppError::Config(format!(
                    "duplicate workspace_id '{}' in [[workspace]] entries",
                    mapping.workspace_id
                )));
            }
        }
        Ok(())
    }

    /// Resolve the effective Slack channel ID from connection parameters.
    ///
    /// Implements FR-011, FR-012, and FR-013:
    ///
    /// 1. If `workspace_id` is `Some(_)`, look it up in the `[[workspace]]`
    ///    entries.
    ///    - **Found** → return the mapped `channel_id`.
    ///    - **Not found** → return `None` (`workspace_id` takes precedence;
    ///      the bare `channel_id` parameter is **not** used as a fallback).
    /// 2. If `workspace_id` is `None`, return `channel_id` unchanged
    ///    (backward compatibility with legacy `?channel_id=` clients).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # let config = agent_intercom::config::GlobalConfig::from_toml_str("").unwrap();
    /// // workspace_id resolves to mapped channel
    /// let ch = config.resolve_channel_id(Some("my-repo"), None);
    ///
    /// // bare channel_id used as-is (legacy clients)
    /// let ch = config.resolve_channel_id(None, Some("C0123456789"));
    /// ```
    #[must_use]
    pub fn resolve_channel_id<'a>(
        &'a self,
        workspace_id: Option<&str>,
        channel_id: Option<&'a str>,
    ) -> Option<&'a str> {
        if let Some(ws_id) = workspace_id {
            // workspace_id present → look up in the mapping table.
            // If not found, return None (no silent fallback).
            self.workspaces
                .iter()
                .find(|m| m.workspace_id == ws_id)
                .map(|m| m.channel_id.as_str())
        } else {
            // No workspace_id → pass channel_id through unchanged.
            channel_id
        }
    }

    /// Validate configuration requirements for ACP mode.
    ///
    /// Verifies that `host_cli` is non-empty and, if it is an absolute path,
    /// that the path exists on disk. Relative command names (resolved via
    /// `PATH` at spawn time) are accepted as-is.
    ///
    /// Call this before starting the ACP transport to surface misconfiguration
    /// early with a clear error message.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Config` when:
    /// - `host_cli` is empty
    /// - `host_cli` is an absolute path that does not exist
    pub fn validate_for_acp_mode(&self) -> Result<()> {
        if self.host_cli.is_empty() {
            return Err(AppError::Config(
                "host_cli must be set to use ACP mode".into(),
            ));
        }
        let path = std::path::Path::new(&self.host_cli);
        if path.is_absolute() && !path.exists() {
            return Err(AppError::Config(format!(
                "host_cli '{}' does not exist",
                self.host_cli
            )));
        }
        Ok(())
    }
}

/// Keychain service identifier used for credential storage (shared/default).
const KEYCHAIN_SERVICE: &str = "agent-intercom";

/// Return the uppercase env-var suffix for a given server mode.
///
/// MCP is the default protocol, so it uses no suffix (empty string) for
/// backwards compatibility. ACP mode uses `_ACP`.
fn mode_env_suffix(mode: ServerMode) -> &'static str {
    match mode {
        ServerMode::Mcp => "",
        ServerMode::Acp => "_ACP",
    }
}

/// Return the keychain service name scoped to a server mode.
///
/// MCP uses the shared service (`agent-intercom`), ACP uses
/// `agent-intercom-acp`.
fn mode_keychain_service(mode: ServerMode) -> &'static str {
    match mode {
        ServerMode::Mcp => KEYCHAIN_SERVICE,
        ServerMode::Acp => "agent-intercom-acp",
    }
}

/// Try a single keychain lookup and return `Ok(value)` on success.
async fn try_keyring(service: &str, key: &str) -> std::result::Result<String, ()> {
    let service = service.to_owned();
    let key = key.to_owned();
    let result = tokio::task::spawn_blocking(move || {
        keyring::Entry::new(&service, &key).and_then(|entry| entry.get_password())
    })
    .await
    .map_err(|_| ())?;
    match result {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => Err(()),
    }
}

/// Load a single credential using mode-prefixed resolution with fallback.
///
/// Resolution order (first non-empty wins):
/// 1. Keychain `agent-intercom-{mode}` / `{keyring_key}`
/// 2. Env var `{env_key}_{MODE}`
/// 3. Keychain `agent-intercom` / `{keyring_key}` (shared)
/// 4. Env var `{env_key}` (shared)
///
/// For MCP mode (the default), steps 1–2 are identical to 3–4 because the
/// mode suffix is empty, so the function behaves exactly as before.
///
/// # Errors
///
/// Returns `AppError::Config` with a message naming all checked sources.
async fn load_credential(keyring_key: &str, env_key: &str, mode: ServerMode) -> Result<String> {
    let _span =
        tracing::info_span!("load_credential", key = keyring_key, env = env_key, ?mode,).entered();

    let mode_service = mode_keychain_service(mode);
    let mode_suffix = mode_env_suffix(mode);
    let mode_env = format!("{env_key}{mode_suffix}");

    // 1. Mode-specific keychain.
    if let Ok(value) = try_keyring(mode_service, keyring_key).await {
        tracing::info!(
            key = keyring_key,
            source = "keychain",
            service = mode_service,
            "credential loaded"
        );
        return Ok(value);
    }

    // 2. Mode-specific env var.
    if let Ok(value) = env::var(&mode_env) {
        if !value.is_empty() {
            tracing::info!(
                key = keyring_key,
                source = "env",
                var = mode_env.as_str(),
                "credential loaded"
            );
            return Ok(value);
        }
    }

    // 3–4: Only needed when mode ≠ Mcp (MCP has no suffix, so 1–2 already
    //       checked the shared names).
    if mode != ServerMode::Mcp {
        if let Ok(value) = try_keyring(KEYCHAIN_SERVICE, keyring_key).await {
            tracing::info!(
                key = keyring_key,
                source = "keychain",
                service = KEYCHAIN_SERVICE,
                "credential loaded (shared fallback)"
            );
            return Ok(value);
        }
        if let Ok(value) = env::var(env_key) {
            if !value.is_empty() {
                tracing::info!(
                    key = keyring_key,
                    source = "env",
                    var = env_key,
                    "credential loaded (shared fallback)"
                );
                return Ok(value);
            }
        }
    }

    if mode == ServerMode::Mcp {
        Err(AppError::Config(format!(
            "credential `{keyring_key}` not found: checked keychain service \
             `{KEYCHAIN_SERVICE}` and environment variable `{env_key}`"
        )))
    } else {
        Err(AppError::Config(format!(
            "credential `{keyring_key}` not found: checked keychain services \
             `{mode_service}` and `{KEYCHAIN_SERVICE}`, and environment \
             variables `{mode_env}` and `{env_key}`"
        )))
    }
}

/// Load an optional credential — returns an empty string if absent.
///
/// Uses the same resolution order as [`load_credential`] but never fails.
async fn load_optional_credential(keyring_key: &str, env_key: &str, mode: ServerMode) -> String {
    if let Ok(value) = load_credential(keyring_key, env_key, mode).await {
        value
    } else {
        tracing::info!(
            key = keyring_key,
            "optional credential not found, using empty default"
        );
        String::new()
    }
}
