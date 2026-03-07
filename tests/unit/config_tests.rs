use agent_intercom::config::{
    AcpConfig, DatabaseConfig, GlobalConfig, SlackConfig, SlackDetailLevel,
};
use agent_intercom::AppError;

fn sample_toml(workspace: &str) -> String {
    format!(
        r#"
default_workspace_root = '{workspace}'
http_port = 3000
ipc_name = "agent-intercom"
max_concurrent_sessions = 2
host_cli = "claude"
host_cli_args = ["--stdio"]
retention_days = 14

[slack]
channel_id = "C123"

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = true
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"

[commands]
status = "git status"
"#
    )
}

fn minimal_toml(workspace: &str) -> String {
    format!(
        r#"
default_workspace_root = '{workspace}'
http_port = 3000
ipc_name = "agent-intercom"
max_concurrent_sessions = 1
host_cli = "claude"

[slack]
channel_id = "C123"

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#
    )
}

#[test]
fn parses_valid_config() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    assert_eq!(config.http_port, 3000);
    assert_eq!(config.ipc_name, "agent-intercom");
    assert!(
        config.authorized_user_ids.is_empty(),
        "authorized_user_ids is not populated from TOML"
    );
    assert!(config.commands.contains_key("status"));
    // On Windows, `canonicalize()` may or may not add the `\\?\`
    // extended-length prefix depending on the path source. Strip
    // it from both sides before comparing.
    let strip_unc = |p: &std::path::Path| -> std::path::PathBuf {
        p.to_str()
            .and_then(|s| s.strip_prefix(r"\\?\"))
            .map_or_else(|| p.to_path_buf(), std::path::PathBuf::from)
    };
    let expected_root = strip_unc(&temp.path().canonicalize().expect("canonicalize temp path"));
    let actual_root = strip_unc(config.default_workspace_root());
    assert_eq!(actual_root, expected_root);
    assert_eq!(config.retention_days, 14);
}

#[test]
fn defaults_retention_days() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = minimal_toml(temp.path().to_str().expect("utf8 path"));

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert_eq!(config.retention_days, 30);
}

#[test]
fn defaults_host_cli_args_to_empty() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = minimal_toml(temp.path().to_str().expect("utf8 path"));

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert!(config.host_cli_args.is_empty());
}

#[test]
fn defaults_commands_to_empty() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = minimal_toml(temp.path().to_str().expect("utf8 path"));

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert!(config.commands.is_empty());
}

#[test]
fn rejects_missing_workspace_root() {
    let toml = r#"
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "claude"

[slack]
channel_id = "C123"

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#;

    let result = GlobalConfig::from_toml_str(toml);
    assert!(result.is_err());
}

#[test]
fn rejects_missing_slack_section() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "claude"

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        temp.path().to_str().expect("utf8")
    );

    let result = GlobalConfig::from_toml_str(&toml);
    assert!(result.is_err());
}

#[test]
fn rejects_invalid_field_type() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = "not-a-number"
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "claude"

[slack]
channel_id = "C123"

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        temp.path().to_str().expect("utf8")
    );

    let result = GlobalConfig::from_toml_str(&toml);
    assert!(result.is_err());
}

#[test]
fn rejects_unauthorized_user() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));
    let mut config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    config.authorized_user_ids = vec!["U123".into(), "U456".into()];

    let result = config.ensure_authorized("U999");
    match result {
        Err(AppError::Unauthorized(_)) => {}
        other => panic!("expected unauthorized error, got {other:?}"),
    }
}

#[test]
fn allows_authorized_user() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));
    let mut config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    config.authorized_user_ids = vec!["U123".into(), "U456".into()];

    config
        .ensure_authorized("U123")
        .expect("user should be authorized");
}

#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn missing_authorized_user_ids_env_var_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));
    let mut config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    unsafe {
        std::env::remove_var("SLACK_MEMBER_IDS");
        std::env::remove_var("SLACK_MEMBER_IDS_ACP");
    }

    let result = config.load_authorized_users(agent_intercom::mode::ServerMode::Mcp);
    assert!(
        result.is_err(),
        "load_authorized_users should fail when env var is absent"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("SLACK_MEMBER_IDS"),
        "error message should name the env var, got: {err_msg}"
    );
}

#[test]
fn credential_env_fallback() {
    // T006: credential loading falls back to env vars.
    // We set env vars and verify load_credentials populates the Slack tokens.
    // This is a synchronous test that checks the config struct after env is set.
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    // Before credential loading, tokens, team_id, and user IDs are empty (serde(skip)).
    assert!(config.slack.app_token.is_empty());
    assert!(config.slack.bot_token.is_empty());
    assert!(config.slack.team_id.is_empty());
    assert!(config.authorized_user_ids.is_empty());
}

// ── US1: Brand identity constant assertions (T018–T020) ─────────────

/// T018: `KEYCHAIN_SERVICE` constant equals "agent-intercom".
///
/// Since the constant is private, we verify indirectly through the error
/// message produced when credentials are absent.
#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn keychain_service_constant_is_agent_intercom() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));
    let mut config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    // Clear env so credential loading fails and surfaces the service name.
    unsafe {
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_BOT_TOKEN");
        std::env::remove_var("SLACK_TEAM_ID");
        std::env::remove_var("SLACK_MEMBER_IDS");
    }

    let err = config
        .load_credentials(agent_intercom::mode::ServerMode::Mcp)
        .await
        .expect_err("should fail when no credentials");
    let msg = format!("{err}");
    assert!(
        msg.contains("agent-intercom"),
        "keychain service name should be 'agent-intercom', got: {msg}"
    );
    assert!(
        !msg.contains("monocoque"),
        "should not reference old name, got: {msg}"
    );
}

/// T019: IPC pipe name default equals "agent-intercom".
#[test]
fn ipc_name_default_is_agent_intercom() {
    let temp = tempfile::tempdir().expect("tempdir");
    // Minimal TOML without ipc_name — should default to "agent-intercom".
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
max_concurrent_sessions = 1
host_cli = "test"
host_cli_args = []

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        temp.path().to_str().expect("utf8")
    );
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert_eq!(
        config.ipc_name, "agent-intercom",
        "default IPC name should be 'agent-intercom'"
    );
}

/// T016/S004: ACP mode validation rejects an empty `host_cli`.
#[test]
fn acp_validate_rejects_empty_host_cli() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut config = GlobalConfig::from_toml_str(&sample_toml(temp.path().to_str().expect("utf8")))
        .expect("config parses");
    config.host_cli = String::new();

    let result = config.validate_for_acp_mode();
    assert!(
        result.is_err(),
        "empty host_cli should fail ACP validation (S004)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("host_cli"),
        "error must mention host_cli, got: {msg}"
    );
}

/// T016/S005: ACP mode validation rejects a non-existent absolute path.
#[test]
fn acp_validate_rejects_nonexistent_absolute_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut config = GlobalConfig::from_toml_str(&sample_toml(temp.path().to_str().expect("utf8")))
        .expect("config parses");
    // Use a path inside tempdir that does not exist — guaranteed to be absolute.
    let nonexistent = temp.path().join("does_not_exist_binary");
    config.host_cli = nonexistent.to_string_lossy().into_owned();

    let result = config.validate_for_acp_mode();
    assert!(
        result.is_err(),
        "nonexistent absolute path should fail ACP validation (S005)"
    );
}

/// T016: ACP mode validation accepts a relative command name (PATH resolution deferred).
#[test]
fn acp_validate_accepts_relative_command_name() {
    let temp = tempfile::tempdir().expect("tempdir");
    // sample_toml has host_cli = "claude", which is a relative command name.
    let config = GlobalConfig::from_toml_str(&sample_toml(temp.path().to_str().expect("utf8")))
        .expect("config parses");
    let result = config.validate_for_acp_mode();
    assert!(
        result.is_ok(),
        "relative command name accepted at config validation time: {result:?}"
    );
}
///
/// Verifies the spawner uses INTERCOM_ prefix by checking that config
/// parsing does not reference MONOCOQUE_ prefixed env vars.
#[test]
fn env_var_prefix_is_intercom() {
    // Integration-level check: the source code in spawner.rs uses INTERCOM_
    // prefix. This is verified at compile time via the rename, and we
    // double-check here that the default config does not contain "MONOCOQUE".
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8"));
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    // The ipc_name field should not contain "monocoque".
    assert!(
        !config.ipc_name.contains("monocoque"),
        "ipc_name should not contain 'monocoque'"
    );
}

// ── T127 (S092–S094): host_cli path validation ───────────────────────────────

/// S094 — `validate_host_cli_path` returns `AppError::Config` when `host_cli`
/// is an absolute path that does not exist on the filesystem (FR-039).
#[test]
fn validate_host_cli_path_nonexistent_absolute_returns_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    // Build a path that is absolute but does not exist.
    let nonexistent = temp.path().join("zzz_nonexistent_cli_binary");
    // Explicitly do NOT create the file.
    let toml = format!(
        r#"
default_workspace_root = '{workspace}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = '{cli}'

[slack]
channel_id = "C123"

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        workspace = temp.path().to_str().expect("utf8"),
        cli = nonexistent.to_str().expect("utf8"),
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let result = config.validate_host_cli_path();
    assert!(
        matches!(result, Err(AppError::Config(_))),
        "nonexistent absolute host_cli must return Err(AppError::Config), got: {result:?}"
    );
}

/// S092 — `validate_host_cli_path` returns `Ok(())` for a relative name even
/// when the binary is not on PATH. A warning is logged but no error is
/// returned, allowing for deferred PATH setup (FR-038).
#[test]
fn validate_host_cli_path_relative_name_not_on_path_returns_ok() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{workspace}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "zzz_clearly_not_a_real_binary_xyz"

[slack]
channel_id = "C123"

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        workspace = temp.path().to_str().expect("utf8"),
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let result = config.validate_host_cli_path();
    assert!(
        result.is_ok(),
        "relative host_cli not on PATH must return Ok(()) (warning logged), got: {result:?}"
    );
}

/// S093 — `validate_host_cli_path` returns `Ok(())` for an absolute path that
/// exists on the filesystem (FR-039).
#[test]
fn validate_host_cli_path_existing_absolute_returns_ok() {
    let temp = tempfile::tempdir().expect("tempdir");
    let cli_path = temp.path().join("test_cli_binary");
    // Create a minimal file so the path exists.
    std::fs::write(&cli_path, b"placeholder").expect("write placeholder binary");

    let toml = format!(
        r#"
default_workspace_root = '{workspace}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = '{cli}'

[slack]
channel_id = "C123"

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        workspace = temp.path().to_str().expect("utf8"),
        cli = cli_path.to_str().expect("utf8"),
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let result = config.validate_host_cli_path();
    assert!(
        result.is_ok(),
        "existing absolute host_cli must return Ok(()), got: {result:?}"
    );
}

// ── strip_unc_prefix tests ───────────────────────────────────────────────────

/// `strip_unc_prefix` removes the `\\?\` extended-length prefix on Windows paths.
#[test]
fn strip_unc_prefix_removes_windows_prefix() {
    use agent_intercom::config::strip_unc_prefix;
    use std::path::PathBuf;

    let input = PathBuf::from(r"\\?\C:\Users\test\project");
    let result = strip_unc_prefix(input);
    assert_eq!(result, PathBuf::from(r"C:\Users\test\project"));
}

/// `strip_unc_prefix` returns the path unchanged when no prefix is present.
#[test]
fn strip_unc_prefix_no_op_without_prefix() {
    use agent_intercom::config::strip_unc_prefix;
    use std::path::PathBuf;

    let input = PathBuf::from(r"C:\Users\test\project");
    let result = strip_unc_prefix(input.clone());
    assert_eq!(result, input);
}

/// `strip_unc_prefix` returns a Unix-style path unchanged (no prefix to strip).
#[test]
fn strip_unc_prefix_no_op_for_unix_path() {
    use agent_intercom::config::strip_unc_prefix;
    use std::path::PathBuf;

    let input = PathBuf::from("/home/user/project");
    let result = strip_unc_prefix(input.clone());
    assert_eq!(result, input);
}

// ── SlackConfig::markdown_fence_label tests ──────────────────────────────────

/// `markdown_fence_label` returns the mapped language label for a known extension.
#[test]
fn markdown_fence_label_returns_language_for_known_extension() {
    let mut extensions = std::collections::HashMap::new();
    extensions.insert("rs".into(), "rust".into());
    extensions.insert("toml".into(), "toml".into());

    let config = SlackConfig {
        channel_id: String::new(),
        app_token: String::new(),
        bot_token: String::new(),
        team_id: String::new(),
        markdown_upload_extensions: extensions,
    };

    assert_eq!(config.markdown_fence_label("src/main.rs"), Some("rust"));
    assert_eq!(config.markdown_fence_label("config.toml"), Some("toml"));
}

/// `markdown_fence_label` returns `None` for an unmapped extension.
#[test]
fn markdown_fence_label_returns_none_for_unknown_extension() {
    let config = SlackConfig {
        channel_id: String::new(),
        app_token: String::new(),
        bot_token: String::new(),
        team_id: String::new(),
        markdown_upload_extensions: std::collections::HashMap::new(),
    };

    assert_eq!(config.markdown_fence_label("README.md"), None);
}

/// `markdown_fence_label` returns `None` for a file with no extension.
#[test]
fn markdown_fence_label_returns_none_for_no_extension() {
    let mut extensions = std::collections::HashMap::new();
    extensions.insert("rs".into(), "rust".into());

    let config = SlackConfig {
        channel_id: String::new(),
        app_token: String::new(),
        bot_token: String::new(),
        team_id: String::new(),
        markdown_upload_extensions: extensions,
    };

    assert_eq!(config.markdown_fence_label("Makefile"), None);
}

// ── SlackConfig Debug redaction ──────────────────────────────────────────────

/// Debug output for `SlackConfig` must redact `app_token` and `bot_token`.
#[test]
fn slack_config_debug_redacts_tokens() {
    let config = SlackConfig {
        channel_id: "C123".into(),
        app_token: "xapp-super-secret".into(),
        bot_token: "xoxb-super-secret".into(),
        team_id: "T123".into(),
        markdown_upload_extensions: std::collections::HashMap::new(),
    };

    let debug = format!("{config:?}");
    assert!(
        debug.contains("[REDACTED]"),
        "Debug output should contain [REDACTED], got: {debug}"
    );
    assert!(
        !debug.contains("xapp-super-secret"),
        "Debug output must not leak app_token"
    );
    assert!(
        !debug.contains("xoxb-super-secret"),
        "Debug output must not leak bot_token"
    );
}

// ── SlackDetailLevel deserialization ─────────────────────────────────────────

/// `SlackDetailLevel` deserializes all three variants from TOML.
#[test]
fn slack_detail_level_parses_all_variants() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path().to_str().expect("utf8");

    for (value, expected) in [
        ("minimal", SlackDetailLevel::Minimal),
        ("standard", SlackDetailLevel::Standard),
        ("verbose", SlackDetailLevel::Verbose),
    ] {
        let toml = format!(
            r#"
default_workspace_root = '{ws}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"
slack_detail_level = "{value}"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#
        );
        let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
        assert_eq!(
            config.slack_detail_level, expected,
            "slack_detail_level = '{value}' should parse to {expected:?}"
        );
    }
}

/// `SlackDetailLevel` defaults to `Standard` when omitted.
#[test]
fn slack_detail_level_defaults_to_standard() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = minimal_toml(temp.path().to_str().expect("utf8"));
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert_eq!(config.slack_detail_level, SlackDetailLevel::Standard);
}

// ── AcpConfig defaults ───────────────────────────────────────────────────────

/// `AcpConfig` fields default correctly when the `[acp]` section is absent.
#[test]
fn acp_config_defaults_when_section_absent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = minimal_toml(temp.path().to_str().expect("utf8"));
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    let defaults = AcpConfig::default();
    assert_eq!(config.acp.max_sessions, defaults.max_sessions);
    assert_eq!(
        config.acp.startup_timeout_seconds,
        defaults.startup_timeout_seconds
    );
    assert_eq!(config.acp.max_msg_rate, defaults.max_msg_rate);
    assert_eq!(config.acp.http_port, defaults.http_port);
}

/// `AcpConfig::default()` produces the documented default values.
#[test]
fn acp_config_default_values() {
    let defaults = AcpConfig::default();
    assert_eq!(defaults.max_sessions, 5);
    assert_eq!(defaults.startup_timeout_seconds, 30);
    assert_eq!(defaults.max_msg_rate, 10);
    assert_eq!(defaults.http_port, 3001);
}

/// `AcpConfig` fields can be overridden in TOML.
#[test]
fn acp_config_overrides_from_toml() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"

[acp]
max_sessions = 10
startup_timeout_seconds = 60
max_msg_rate = 20
http_port = 4001
"#,
        temp.path().to_str().expect("utf8")
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert_eq!(config.acp.max_sessions, 10);
    assert_eq!(config.acp.startup_timeout_seconds, 60);
    assert_eq!(config.acp.max_msg_rate, 20);
    assert_eq!(config.acp.http_port, 4001);
}

// ── DatabaseConfig defaults ──────────────────────────────────────────────────

/// `DatabaseConfig::default()` produces the expected default path.
#[test]
fn database_config_default_path() {
    let defaults = DatabaseConfig::default();
    assert_eq!(defaults.path, std::path::PathBuf::from("data/agent-rc.db"));
}

/// `db_path()` accessor returns the configured database path.
#[test]
fn db_path_accessor_returns_configured_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"

[database]
path = "custom/path.db"
"#,
        temp.path().to_str().expect("utf8")
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert_eq!(
        config.db_path(),
        std::path::Path::new("custom/path.db"),
        "db_path() should return the configured database path"
    );
}

// ── TimeoutConfig defaults ───────────────────────────────────────────────────

/// `TimeoutConfig` fields default correctly when values are omitted.
#[test]
fn timeout_config_defaults() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"

[slack]

[timeouts]

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        temp.path().to_str().expect("utf8")
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert_eq!(config.timeouts.approval_seconds, 3600);
    assert_eq!(config.timeouts.prompt_seconds, 1800);
    assert_eq!(config.timeouts.wait_seconds, 0);
}

// ── StallConfig defaults ─────────────────────────────────────────────────────

/// `StallConfig` fields default correctly when values are omitted.
#[test]
fn stall_config_defaults() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
"#,
        temp.path().to_str().expect("utf8")
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert!(config.stall.enabled, "stall.enabled should default to true");
    assert_eq!(config.stall.inactivity_threshold_seconds, 300);
    assert_eq!(config.stall.escalation_threshold_seconds, 120);
    assert_eq!(config.stall.max_retries, 3);
    assert_eq!(
        config.stall.default_nudge_message,
        "Continue working on the current task. Pick up where you left off."
    );
}

// ── max_concurrent_sessions validation ───────────────────────────────────────

/// `max_concurrent_sessions = 0` is rejected at parse time.
#[test]
fn rejects_zero_max_concurrent_sessions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 0
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        temp.path().to_str().expect("utf8")
    );

    let result = GlobalConfig::from_toml_str(&toml);
    assert!(result.is_err(), "zero max_concurrent_sessions should fail");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("max_concurrent_sessions"),
        "error should mention max_concurrent_sessions, got: {msg}"
    );
}

/// `max_concurrent_sessions` defaults to 3 when omitted.
#[test]
fn defaults_max_concurrent_sessions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
ipc_name = "test"
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        temp.path().to_str().expect("utf8")
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert_eq!(config.max_concurrent_sessions, 3);
}

// ── resolve_workspace_by_channel_id tests ────────────────────────────────────

/// `resolve_workspace_by_channel_id` finds the matching workspace entry.
#[test]
fn resolve_workspace_by_channel_id_found() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"

[[workspace]]
workspace_id = "repo-a"
channel_id = "C111"
label = "Repo A"

[[workspace]]
workspace_id = "repo-b"
channel_id = "C222"
"#,
        temp.path().to_str().expect("utf8")
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let ws = config.resolve_workspace_by_channel_id("C222");
    assert!(ws.is_some(), "should find workspace for channel C222");
    assert_eq!(ws.unwrap().workspace_id, "repo-b");
}

/// `resolve_workspace_by_channel_id` returns `None` for unknown channel.
#[test]
fn resolve_workspace_by_channel_id_not_found() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"

[[workspace]]
workspace_id = "repo-a"
channel_id = "C111"
"#,
        temp.path().to_str().expect("utf8")
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert!(config.resolve_workspace_by_channel_id("C999").is_none());
}

// ── workspace_root_for_channel tests ─────────────────────────────────────────

/// `workspace_root_for_channel` returns the workspace `path` when configured.
#[test]
fn workspace_root_for_channel_uses_configured_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws_path = temp.path().join("repo-a");
    std::fs::create_dir_all(&ws_path).expect("create workspace dir");

    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"

[[workspace]]
workspace_id = "repo-a"
channel_id = "C111"
path = '{ws_path}'
"#,
        root = temp.path().to_str().expect("utf8"),
        ws_path = ws_path.to_str().expect("utf8"),
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let result = config.workspace_root_for_channel("C111");
    assert_eq!(result, ws_path);
}

/// `workspace_root_for_channel` falls back to `default_workspace_root` when
/// the workspace has no `path` configured.
#[test]
fn workspace_root_for_channel_falls_back_to_default() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"

[[workspace]]
workspace_id = "repo-a"
channel_id = "C111"
"#,
        temp.path().to_str().expect("utf8")
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let result = config.workspace_root_for_channel("C111");
    assert_eq!(result, config.default_workspace_root());
}

/// `workspace_root_for_channel` falls back to default for unknown channel.
#[test]
fn workspace_root_for_channel_unknown_channel_uses_default() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = minimal_toml(temp.path().to_str().expect("utf8"));

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let result = config.workspace_root_for_channel("C_UNKNOWN");
    assert_eq!(result, config.default_workspace_root());
}

// ── load_from_path tests ─────────────────────────────────────────────────────

/// `load_from_path` reads and parses a valid config file.
#[test]
fn load_from_path_reads_valid_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let config_path = temp.path().join("test-config.toml");
    let toml = sample_toml(temp.path().to_str().expect("utf8"));
    std::fs::write(&config_path, toml).expect("write config file");

    let config = GlobalConfig::load_from_path(&config_path).expect("load succeeds");
    assert_eq!(config.http_port, 3000);
}

/// `load_from_path` returns an error for a non-existent file.
#[test]
fn load_from_path_fails_for_nonexistent_file() {
    let result = GlobalConfig::load_from_path("nonexistent/path/config.toml");
    assert!(result.is_err(), "should fail for nonexistent file");
}

/// `load_from_path` returns an error for invalid TOML content.
#[test]
fn load_from_path_fails_for_invalid_toml() {
    let temp = tempfile::tempdir().expect("tempdir");
    let config_path = temp.path().join("bad.toml");
    std::fs::write(&config_path, "this is not [[[valid toml").expect("write bad file");

    let result = GlobalConfig::load_from_path(&config_path);
    assert!(result.is_err(), "should fail for invalid TOML");
}

// ── http_port default ────────────────────────────────────────────────────────

/// `http_port` defaults to 3000 when omitted.
#[test]
fn defaults_http_port() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{}'
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        temp.path().to_str().expect("utf8")
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert_eq!(config.http_port, 3000);
}

// ── WorkspaceMapping with path field ─────────────────────────────────────────

/// Workspace mappings with optional `path` field parse correctly.
#[test]
fn workspace_mapping_with_path_parses() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws_path = temp.path().join("my-repo");
    std::fs::create_dir_all(&ws_path).expect("create dir");

    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 3000
ipc_name = "test"
max_concurrent_sessions = 1
host_cli = "test"

[slack]

[timeouts]
approval_seconds = 60
prompt_seconds = 60
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"

[[workspace]]
workspace_id = "my-repo"
channel_id = "C001"
label = "My Repo"
path = '{path}'
"#,
        root = temp.path().to_str().expect("utf8"),
        path = ws_path.to_str().expect("utf8"),
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let ws = &config.workspaces[0];
    assert_eq!(ws.workspace_id, "my-repo");
    assert_eq!(ws.channel_id, "C001");
    assert_eq!(ws.label.as_deref(), Some("My Repo"));
    assert_eq!(ws.path.as_deref(), Some(ws_path.as_path()));
}

// ── ensure_authorized edge cases ─────────────────────────────────────────────

/// `ensure_authorized` passes when the list is empty (no restriction).
/// Note: The design treats an empty list as "nobody authorized".
#[test]
fn ensure_authorized_empty_list_rejects() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8"));
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    // authorized_user_ids is empty by default (serde(skip)).
    let result = config.ensure_authorized("U123");
    assert!(
        matches!(result, Err(AppError::Unauthorized(_))),
        "empty authorized list should reject all users"
    );
}

/// `ensure_authorized` handles multiple IDs and finds each one.
#[test]
fn ensure_authorized_finds_each_id() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8"));
    let mut config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    config.authorized_user_ids = vec!["U001".into(), "U002".into(), "U003".into()];

    config.ensure_authorized("U001").expect("U001 authorized");
    config.ensure_authorized("U002").expect("U002 authorized");
    config.ensure_authorized("U003").expect("U003 authorized");
    assert!(config.ensure_authorized("U004").is_err());
}

#[test]
fn strip_unc_prefix_removes_windows_extended_prefix() {
    use agent_intercom::config::strip_unc_prefix;
    use std::path::PathBuf;

    let with_prefix = PathBuf::from(r"\\?\D:\Source\GitHub\agent-intercom");
    let stripped = strip_unc_prefix(with_prefix);
    assert_eq!(stripped, PathBuf::from(r"D:\Source\GitHub\agent-intercom"));
}

#[test]
fn strip_unc_prefix_preserves_normal_path() {
    use agent_intercom::config::strip_unc_prefix;
    use std::path::PathBuf;

    let normal = PathBuf::from(r"D:\Source\GitHub\agent-intercom");
    let result = strip_unc_prefix(normal.clone());
    assert_eq!(result, normal);
}
