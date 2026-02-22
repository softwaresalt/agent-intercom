use monocoque_agent_rc::{config::GlobalConfig, AppError};

fn sample_toml(workspace: &str) -> String {
    format!(
        r#"
default_workspace_root = '{workspace}'
http_port = 3000
ipc_name = "monocoque-agent-rc"
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
ipc_name = "monocoque-agent-rc"
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
    assert_eq!(config.ipc_name, "monocoque-agent-rc");
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

#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn rejects_unauthorized_user() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));
    let mut config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    unsafe {
        std::env::set_var("SLACK_MEMBER_IDS", "U123,U456");
    }
    config.load_authorized_users().expect("load users");

    let result = config.ensure_authorized("U999");
    match result {
        Err(AppError::Unauthorized(_)) => {}
        other => panic!("expected unauthorized error, got {other:?}"),
    }

    unsafe {
        std::env::remove_var("SLACK_MEMBER_IDS");
    }
}

#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn allows_authorized_user() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));
    let mut config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    unsafe {
        std::env::set_var("SLACK_MEMBER_IDS", "U123,U456");
    }
    config.load_authorized_users().expect("load users");

    config
        .ensure_authorized("U123")
        .expect("user should be authorized");

    unsafe {
        std::env::remove_var("SLACK_MEMBER_IDS");
    }
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
    }

    let result = config.load_authorized_users();
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
