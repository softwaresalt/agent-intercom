use monocoque_agent_rem::{config::GlobalConfig, AppError};

fn sample_toml(workspace: &str) -> String {
    format!(
        r#"
workspace_root = "{workspace}"
http_port = 3000
ipc_name = "monocoque-agent-rem"
max_concurrent_sessions = 2
host_cli = "claude"
host_cli_args = ["--stdio"]

[slack]
app_token = "xapp-1"
bot_token = "xoxb-1"
channel_id = "C123"

authorized_user_ids = ["U123", "U456"]

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

#[test]
fn parses_valid_config() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    assert_eq!(config.http_port, 3000);
    assert_eq!(config.ipc_name, "monocoque-agent-rem");
    assert_eq!(config.authorized_user_ids.len(), 2);
    assert!(config.commands.contains_key("status"));
    assert_eq!(config.workspace_root(), temp.path());
}

#[test]
fn rejects_unauthorized_user() {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    let result = config.ensure_authorized("U999");
    match result {
        Err(AppError::Unauthorized(_)) => {}
        other => panic!("expected unauthorized error, got {other:?}"),
    }
}
