//! Contract test: `/mcp?channel_id=C_DIRECT` — `channel_id` silently ignored (F-10, S018).
//!
//! After F-10, the MCP endpoint no longer accepts `?channel_id=` as a routing
//! parameter.  Clients must use `?workspace_id=` instead.  Any `channel_id`
//! present in the query string is silently discarded — it has no effect on
//! which Slack channel the session is associated with.

use agent_intercom::config::GlobalConfig;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn toml_with_workspace(workspace_id: &str, channel_id: &str) -> (tempfile::TempDir, String) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8").replace('\\', "\\\\");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 3000
ipc_name = "test-contract-no-channel-id"
max_concurrent_sessions = 2
host_cli = "echo"

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
workspace_id = "{workspace_id}"
channel_id = "{channel_id}"
"#
    );
    (tmp, toml)
}

fn toml_no_workspace() -> (tempfile::TempDir, String) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8").replace('\\', "\\\\");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 3000
ipc_name = "test-contract-bare-channel"
max_concurrent_sessions = 2
host_cli = "echo"

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
    (tmp, toml)
}

// ── S018: channel_id in query string is silently ignored ─────────────────────

/// Contract: a request to `/mcp?channel_id=C_DIRECT` MUST NOT route via the
/// bare `channel_id` parameter.  The parameter is silently discarded.
///
/// The only observable effect of removing the `channel_id` fallback is that
/// `resolve_channel_id(None)` returns `None` instead of the former
/// `resolve_channel_id(None, Some("C_DIRECT"))` → `Some("C_DIRECT")`.
#[test]
fn mcp_channel_id_query_param_is_silently_ignored() {
    let (_tmp, toml) = toml_no_workspace();
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    // NEW 1-arg API: no workspace_id → None (channel_id URL param has been removed).
    let result = config.resolve_channel_id(None);
    assert_eq!(
        result, None,
        "S018 contract: /mcp?channel_id=C_DIRECT must be silently ignored — result must be None"
    );
}

/// Contract: `workspace_id` is the only valid routing parameter.
/// When `workspace_id` is present and known, the channel resolves correctly.
#[test]
fn workspace_id_is_the_only_routing_mechanism() {
    let (_tmp, toml) = toml_with_workspace("my-ws", "C_WS_CHANNEL");
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    // workspace_id present and known → resolves to mapped channel.
    let result = config.resolve_channel_id(Some("my-ws"));
    assert_eq!(
        result,
        Some("C_WS_CHANNEL"),
        "contract: workspace_id must be the sole routing mechanism"
    );
}

/// Contract: when `workspace_id` is present but unknown, routing fails
/// gracefully — `None` is returned and no error is raised.
#[test]
fn unknown_workspace_id_yields_no_channel_gracefully() {
    let (_tmp, toml) = toml_with_workspace("known-ws", "C_KNOWN");
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    let result = config.resolve_channel_id(Some("unknown-ws"));
    assert_eq!(
        result, None,
        "contract: unknown workspace_id must yield None, not panic or fall back"
    );
}
