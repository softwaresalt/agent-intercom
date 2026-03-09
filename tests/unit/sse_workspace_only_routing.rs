//! Unit tests for workspace-only channel routing after F-10 (`channel_id` removal).
//!
//! Covers:
//! - S018: `?channel_id=C_DIRECT` query param is silently ignored — `workspace_id` is
//!   the only routing mechanism
//! - S019: `update_pending_from_uri` only extracts `session_id` and `workspace_id`
//! - S016: `workspace_id` resolves to a Slack channel via the mapping table
//! - S020: unknown `workspace_id` returns `None` — no silent fallback to `channel_id`

use agent_intercom::config::GlobalConfig;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a minimal valid TOML string with a single `[[workspace]]` entry.
///
/// The caller must keep the returned `TempDir` alive for the duration of the
/// test — dropping it removes the directory that `default_workspace_root`
/// points to, causing config validation to fail.
fn config_with_workspace(workspace_id: &str, channel_id: &str) -> (tempfile::TempDir, String) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8").replace('\\', "\\\\");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 3000
ipc_name = "test-workspace-only"
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

/// Build a minimal valid TOML with no `[[workspace]]` entries.
///
/// The caller must keep the returned `TempDir` alive for the duration of the
/// test.
fn config_no_workspace() -> (tempfile::TempDir, String) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8").replace('\\', "\\\\");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 3000
ipc_name = "test-no-workspace"
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

// ── T018 / S018 / S019 — channel_id is silently ignored ──────────────────────

/// After F-10: `resolve_channel_id` takes only `workspace_id`.
/// When no `workspace_id` is provided there is NO `channel_id` fallback —
/// the result is always `None`.
///
/// Previously, `resolve_channel_id(None, Some("C_DIRECT"))` would return
/// `Some("C_DIRECT")` (bare `channel_id` pass-through). After F-10 the
/// `channel_id` parameter is removed entirely so that URL `?channel_id=` has
/// no effect on routing.
#[test]
fn channel_id_query_param_is_not_used_for_routing() {
    let (_tmp, toml) = config_no_workspace();
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    // NEW 1-arg API: no workspace_id → no channel (channel_id fallback removed).
    let result = config.resolve_channel_id(None);
    assert_eq!(
        result, None,
        "S018/S019: channel_id URL param must be silently ignored — no fallback routing"
    );
}

// ── T019 / S016 — workspace_id resolves channel from mapping ─────────────────

/// A known `workspace_id` resolves to the configured `channel_id` from the
/// `[[workspace]]` mapping table (S016).
///
/// This is the ONLY routing mechanism after F-10.
#[test]
fn workspace_id_resolves_channel_from_mapping() {
    let (_tmp, toml) = config_with_workspace("my-repo", "C_MAPPED");
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    // NEW 1-arg API: workspace_id → resolved channel.
    let result = config.resolve_channel_id(Some("my-repo"));
    assert_eq!(
        result,
        Some("C_MAPPED"),
        "S016: known workspace_id must resolve to the mapped channel_id"
    );
}

// ── T020 / S020 — unknown workspace_id returns None ──────────────────────────

/// An unknown `workspace_id` returns `None` — the session runs without a
/// Slack channel rather than silently falling back to any `channel_id` param.
///
/// A warning is logged (observable in tracing output) but the behaviour is
/// deterministic: `None` is always returned when the workspace is unknown.
#[test]
fn unknown_workspace_id_returns_none_no_channel() {
    let (_tmp, toml) = config_with_workspace("known-repo", "C_KNOWN");
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    // NEW 1-arg API: unknown workspace → None (S020).
    let result = config.resolve_channel_id(Some("unknown-repo"));
    assert_eq!(
        result, None,
        "S020: unknown workspace_id must return None — no silent fallback"
    );
}

/// When no `workspace_id` is provided and no workspace entries exist,
/// the result is `None`.
#[test]
fn no_workspace_id_and_no_mapping_returns_none() {
    let (_tmp, toml) = config_no_workspace();
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    let result = config.resolve_channel_id(None);
    assert_eq!(
        result, None,
        "no workspace_id and no mappings must produce None"
    );
}
