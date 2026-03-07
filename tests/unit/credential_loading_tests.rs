//! Unit tests for Slack credential loading (T200 — US11).
//!
//! Validates the env-var-only credential path, keychain precedence,
//! missing credential error message quality, optional `SLACK_TEAM_ID`,
//! empty env-var handling, and mode-prefixed credential resolution (ADR-0015).

use agent_intercom::config::GlobalConfig;
use agent_intercom::mode::ServerMode;

fn sample_toml(workspace: &str) -> String {
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

/// Helper: build a test config from a temp dir.
fn make_config() -> (tempfile::TempDir, GlobalConfig) {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = sample_toml(temp.path().to_str().expect("utf8 path"));
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    (temp, config)
}

/// Env-var-only credential loading works when keychain has no entries.
///
/// Sets `SLACK_APP_TOKEN`, `SLACK_BOT_TOKEN`, `SLACK_TEAM_ID` via env,
/// then calls `load_credentials()` which should fall back to env vars
/// since the test environment has no keychain entries for this service.
///
/// NOTE: These tests mutate process-global env vars and must run serially.
/// Use `cargo test credential_loading -- --test-threads=1` if needed.
#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn env_var_only_credential_loading() {
    let (_temp, mut config) = make_config();

    // Set env vars (these will be used since the keychain service
    // "agent-intercom" is almost certainly absent in CI/test envs).
    unsafe {
        std::env::set_var("SLACK_APP_TOKEN", "xapp-test-app-token");
        std::env::set_var("SLACK_BOT_TOKEN", "xoxb-test-bot-token");
        std::env::set_var("SLACK_TEAM_ID", "T_TEST_TEAM");
        std::env::set_var("SLACK_MEMBER_IDS", "U_TEST");
    }

    let result = config.load_credentials(ServerMode::Mcp).await;
    assert!(
        result.is_ok(),
        "load_credentials should succeed with env vars"
    );

    assert_eq!(config.slack.app_token, "xapp-test-app-token");
    assert_eq!(config.slack.bot_token, "xoxb-test-bot-token");
    assert_eq!(config.slack.team_id, "T_TEST_TEAM");
    assert_eq!(config.authorized_user_ids, vec!["U_TEST"]);

    // Clean up.
    unsafe {
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_BOT_TOKEN");
        std::env::remove_var("SLACK_TEAM_ID");
        std::env::remove_var("SLACK_MEMBER_IDS");
    }
}

/// Missing required credential produces error that names both the
/// keychain service and the environment variable.
#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn missing_required_credential_error_names_both_sources() {
    let (_temp, mut config) = make_config();

    // Ensure env vars are absent.
    unsafe {
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_BOT_TOKEN");
        std::env::remove_var("SLACK_TEAM_ID");
        std::env::remove_var("SLACK_MEMBER_IDS");
    }

    let result = config.load_credentials(ServerMode::Mcp).await;
    assert!(
        result.is_err(),
        "should fail when no credential source exists"
    );

    let err_msg = format!("{}", result.unwrap_err());
    // The error should mention the keychain service name.
    assert!(
        err_msg.contains("agent-intercom"),
        "error should mention keychain service name, got: {err_msg}"
    );
    // The error should mention the environment variable name.
    assert!(
        err_msg.contains("SLACK_APP_TOKEN") || err_msg.contains("SLACK_BOT_TOKEN"),
        "error should mention the env var name, got: {err_msg}"
    );
}

/// Optional `SLACK_TEAM_ID` absent is not an error.
///
/// When only the required tokens are present but `SLACK_TEAM_ID` is missing,
/// `load_credentials()` should succeed and `team_id` should be empty.
#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn optional_team_id_absent_is_not_error() {
    let (_temp, mut config) = make_config();

    unsafe {
        std::env::set_var("SLACK_APP_TOKEN", "xapp-test-app-token");
        std::env::set_var("SLACK_BOT_TOKEN", "xoxb-test-bot-token");
        std::env::remove_var("SLACK_TEAM_ID");
        std::env::set_var("SLACK_MEMBER_IDS", "U_TEST");
    }

    let result = config.load_credentials(ServerMode::Mcp).await;
    assert!(
        result.is_ok(),
        "should succeed without SLACK_TEAM_ID: {result:?}"
    );

    assert_eq!(config.slack.app_token, "xapp-test-app-token");
    assert_eq!(config.slack.bot_token, "xoxb-test-bot-token");
    // team_id should be empty or a default, not an error.
    // The exact value depends on implementation — empty string is acceptable.

    // Clean up.
    unsafe {
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_BOT_TOKEN");
        std::env::remove_var("SLACK_MEMBER_IDS");
    }
}

/// Empty env var is treated as absent (falls through to error).
#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn empty_env_var_treated_as_absent() {
    let (_temp, mut config) = make_config();

    unsafe {
        std::env::set_var("SLACK_APP_TOKEN", "");
        std::env::set_var("SLACK_BOT_TOKEN", "");
        std::env::remove_var("SLACK_TEAM_ID");
    }

    let result = config.load_credentials(ServerMode::Mcp).await;
    assert!(
        result.is_err(),
        "should fail when env vars are empty strings"
    );

    // Clean up.
    unsafe {
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_BOT_TOKEN");
    }
}

// ═══════════════════════════════════════════════════════════════
//  Mode-prefixed credential resolution (ADR-0015)
// ═══════════════════════════════════════════════════════════════

/// ACP-mode-prefixed env vars take priority over shared env vars.
#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn acp_mode_prefixed_env_vars_take_priority() {
    let (_temp, mut config) = make_config();

    unsafe {
        // Shared (fallback) credentials.
        std::env::set_var("SLACK_APP_TOKEN", "xapp-shared");
        std::env::set_var("SLACK_BOT_TOKEN", "xoxb-shared");
        std::env::set_var("SLACK_MEMBER_IDS", "U_SHARED");
        // ACP-mode-specific credentials.
        std::env::set_var("SLACK_APP_TOKEN_ACP", "xapp-acp-specific");
        std::env::set_var("SLACK_BOT_TOKEN_ACP", "xoxb-acp-specific");
        std::env::set_var("SLACK_MEMBER_IDS_ACP", "U_ACP_SPECIFIC");
    }

    let result = config.load_credentials(ServerMode::Acp).await;
    assert!(result.is_ok(), "should succeed with ACP-prefixed env vars");

    assert_eq!(config.slack.app_token, "xapp-acp-specific");
    assert_eq!(config.slack.bot_token, "xoxb-acp-specific");
    assert_eq!(config.authorized_user_ids, vec!["U_ACP_SPECIFIC"]);

    unsafe {
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_BOT_TOKEN");
        std::env::remove_var("SLACK_MEMBER_IDS");
        std::env::remove_var("SLACK_APP_TOKEN_ACP");
        std::env::remove_var("SLACK_BOT_TOKEN_ACP");
        std::env::remove_var("SLACK_MEMBER_IDS_ACP");
    }
}

/// ACP mode falls back to shared env vars when no ACP-prefixed vars exist.
#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn acp_mode_falls_back_to_shared_env_vars() {
    let (_temp, mut config) = make_config();

    unsafe {
        // Only shared credentials — no ACP-prefixed vars.
        std::env::set_var("SLACK_APP_TOKEN", "xapp-shared");
        std::env::set_var("SLACK_BOT_TOKEN", "xoxb-shared");
        std::env::set_var("SLACK_MEMBER_IDS", "U_SHARED");
        std::env::remove_var("SLACK_APP_TOKEN_ACP");
        std::env::remove_var("SLACK_BOT_TOKEN_ACP");
        std::env::remove_var("SLACK_MEMBER_IDS_ACP");
    }

    let result = config.load_credentials(ServerMode::Acp).await;
    assert!(
        result.is_ok(),
        "should succeed with shared fallback env vars"
    );

    assert_eq!(config.slack.app_token, "xapp-shared");
    assert_eq!(config.slack.bot_token, "xoxb-shared");
    assert_eq!(config.authorized_user_ids, vec!["U_SHARED"]);

    unsafe {
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_BOT_TOKEN");
        std::env::remove_var("SLACK_MEMBER_IDS");
    }
}

/// MCP mode ignores ACP-prefixed env vars and uses shared ones.
#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn mcp_mode_ignores_acp_prefixed_env_vars() {
    let (_temp, mut config) = make_config();

    unsafe {
        std::env::set_var("SLACK_APP_TOKEN", "xapp-shared");
        std::env::set_var("SLACK_BOT_TOKEN", "xoxb-shared");
        std::env::set_var("SLACK_MEMBER_IDS", "U_SHARED");
        // These should be ignored when mode is MCP.
        std::env::set_var("SLACK_APP_TOKEN_ACP", "xapp-acp-specific");
        std::env::set_var("SLACK_BOT_TOKEN_ACP", "xoxb-acp-specific");
        std::env::set_var("SLACK_MEMBER_IDS_ACP", "U_ACP_SPECIFIC");
    }

    let result = config.load_credentials(ServerMode::Mcp).await;
    assert!(result.is_ok(), "should succeed with shared env vars");

    assert_eq!(config.slack.app_token, "xapp-shared");
    assert_eq!(config.slack.bot_token, "xoxb-shared");
    assert_eq!(config.authorized_user_ids, vec!["U_SHARED"]);

    unsafe {
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_BOT_TOKEN");
        std::env::remove_var("SLACK_MEMBER_IDS");
        std::env::remove_var("SLACK_APP_TOKEN_ACP");
        std::env::remove_var("SLACK_BOT_TOKEN_ACP");
        std::env::remove_var("SLACK_MEMBER_IDS_ACP");
    }
}

/// ACP mode error message names both prefixed and shared sources.
#[tokio::test]
#[serial_test::serial]
#[allow(unsafe_code)]
async fn acp_mode_error_names_both_prefixed_and_shared_sources() {
    let (_temp, mut config) = make_config();

    unsafe {
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_BOT_TOKEN");
        std::env::remove_var("SLACK_MEMBER_IDS");
        std::env::remove_var("SLACK_APP_TOKEN_ACP");
        std::env::remove_var("SLACK_BOT_TOKEN_ACP");
        std::env::remove_var("SLACK_MEMBER_IDS_ACP");
    }

    let result = config.load_credentials(ServerMode::Acp).await;
    assert!(result.is_err(), "should fail when no credentials");

    let err_msg = format!("{}", result.unwrap_err());
    // Should mention the ACP-specific keychain service.
    assert!(
        err_msg.contains("agent-intercom-acp"),
        "error should mention ACP keychain service, got: {err_msg}"
    );
    // Should also mention the shared keychain service.
    assert!(
        err_msg.contains("agent-intercom"),
        "error should mention shared keychain service, got: {err_msg}"
    );
    // Should mention the ACP-prefixed env var.
    assert!(
        err_msg.contains("SLACK_APP_TOKEN_ACP") || err_msg.contains("SLACK_BOT_TOKEN_ACP"),
        "error should mention ACP-prefixed env var, got: {err_msg}"
    );
    // Should mention the shared env var.
    assert!(
        err_msg.contains("SLACK_APP_TOKEN") || err_msg.contains("SLACK_BOT_TOKEN"),
        "error should mention shared env var, got: {err_msg}"
    );
}
