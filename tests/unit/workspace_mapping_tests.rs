//! Unit tests for workspace-to-channel mapping configuration (Phase 6, T042–T043).
//!
//! Covers:
//! - S027: `[[workspace]]` TOML entries parse into `Vec<WorkspaceMapping>`
//! - S028: `workspace_id` resolves to the configured `channel_id`
//! - S029: Unknown `workspace_id` returns `None` (no fallback)
//! - S030: Bare `channel_id` parameter is used as-is (backward compat)
//! - S031: `workspace_id` takes precedence over `channel_id`
//! - S032: Empty `workspace_id` is rejected at parse time
//! - S033: Duplicate `workspace_id` values are rejected at parse time

use agent_intercom::config::GlobalConfig;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build the boilerplate TOML lines common to every test config.
fn base_toml(workspace_root: &str) -> String {
    format!(
        r#"
default_workspace_root = '{root}'
http_port = 3000
ipc_name = "test-ws-mapping"
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
"#,
        root = workspace_root.replace('\\', "\\\\"),
    )
}

/// Append one `[[workspace]]` entry to the base TOML.
fn with_mapping(base: &str, workspace_id: &str, channel_id: &str, label: Option<&str>) -> String {
    let label_line = label.map_or_else(String::new, |l| format!("\nlabel = \"{l}\""));
    format!(
        "{base}\n[[workspace]]\nworkspace_id = \"{workspace_id}\"\nchannel_id = \"{channel_id}\"{label_line}\n"
    )
}

// ── S027: config parsing ──────────────────────────────────────────────────────

/// Parses `[[workspace]]` TOML entries into a `Vec<WorkspaceMapping>`.
#[test]
fn workspace_mapping_parses_from_config() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let base = base_toml(root);
    let toml = format!(
        "{base}\n\
         [[workspace]]\n\
         workspace_id = \"project-a\"\n\
         channel_id = \"C001\"\n\
         label = \"Project A\"\n\
         \n\
         [[workspace]]\n\
         workspace_id = \"project-b\"\n\
         channel_id = \"C002\"\n"
    );

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    assert_eq!(
        config.workspaces.len(),
        2,
        "expected two workspace mappings"
    );

    let a = &config.workspaces[0];
    assert_eq!(a.workspace_id, "project-a");
    assert_eq!(a.channel_id, "C001");
    assert_eq!(a.label.as_deref(), Some("Project A"));

    let b = &config.workspaces[1];
    assert_eq!(b.workspace_id, "project-b");
    assert_eq!(b.channel_id, "C002");
    assert!(b.label.is_none(), "label should be absent when not set");
}

/// Config with no `[[workspace]]` entries deserialises to an empty vec.
#[test]
fn workspace_mappings_default_to_empty() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let toml = base_toml(tmp.path().to_str().expect("utf8"));

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert!(
        config.workspaces.is_empty(),
        "workspaces should be empty when no [[workspace]] entries are present"
    );
}

// ── S032: empty workspace_id rejected ────────────────────────────────────────

/// An empty `workspace_id` string must be rejected at parse/validate time.
#[test]
fn workspace_mapping_empty_id_is_invalid() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = base_toml(tmp.path().to_str().expect("utf8"));
    let toml = format!("{base}\n[[workspace]]\nworkspace_id = \"\"\nchannel_id = \"C001\"\n");

    let result = GlobalConfig::from_toml_str(&toml);
    assert!(result.is_err(), "empty workspace_id should be rejected");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("workspace_id"),
        "error should mention 'workspace_id', got: {msg}"
    );
}

/// An empty `channel_id` string must also be rejected.
#[test]
fn workspace_mapping_empty_channel_id_is_invalid() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = base_toml(tmp.path().to_str().expect("utf8"));
    let toml = format!("{base}\n[[workspace]]\nworkspace_id = \"my-repo\"\nchannel_id = \"\"\n");

    let result = GlobalConfig::from_toml_str(&toml);
    assert!(result.is_err(), "empty channel_id should be rejected");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("channel_id"),
        "error should mention 'channel_id', got: {msg}"
    );
}

// ── S033: duplicate workspace_ids rejected ────────────────────────────────────

/// Duplicate `workspace_id` values within `[[workspace]]` entries must be
/// rejected with an error that names the offending ID.
#[test]
fn workspace_mapping_duplicate_ids_are_invalid() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = base_toml(tmp.path().to_str().expect("utf8"));
    let toml = format!(
        "{base}\n\
         [[workspace]]\n\
         workspace_id = \"same\"\n\
         channel_id = \"C001\"\n\
         \n\
         [[workspace]]\n\
         workspace_id = \"same\"\n\
         channel_id = \"C002\"\n"
    );

    let result = GlobalConfig::from_toml_str(&toml);
    assert!(result.is_err(), "duplicate workspace_id should be rejected");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("duplicate") || msg.contains("same"),
        "error should mention duplication or the ID, got: {msg}"
    );
}

// ── S028: workspace_id resolves to channel ────────────────────────────────────

/// A known `workspace_id` resolves to the configured `channel_id`.
#[test]
fn workspace_id_resolves_to_channel() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = base_toml(tmp.path().to_str().expect("utf8"));
    let toml = with_mapping(&base, "my-repo", "C_MAPPED", None);

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let result = config.resolve_channel_id(Some("my-repo"), None);

    assert_eq!(
        result,
        Some("C_MAPPED"),
        "known workspace_id should resolve to mapped channel"
    );
}

// ── S029: unknown workspace_id returns None ───────────────────────────────────

/// An unknown `workspace_id` returns `None` — no silent fallback to the
/// raw `channel_id` parameter.
#[test]
fn unknown_workspace_id_returns_none() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = base_toml(tmp.path().to_str().expect("utf8"));
    let toml = with_mapping(&base, "known-repo", "C_KNOWN", None);

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let result = config.resolve_channel_id(Some("unknown-repo"), Some("C_FALLBACK"));

    assert_eq!(
        result, None,
        "unknown workspace_id must return None, not fall back to channel_id"
    );
}

// ── S030: channel_id param used when no workspace_id ─────────────────────────

/// When `workspace_id` is absent, the raw `channel_id` query parameter is
/// returned unchanged for backward compatibility.
#[test]
fn channel_id_param_falls_back_when_no_workspace() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let toml = base_toml(tmp.path().to_str().expect("utf8"));

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let result = config.resolve_channel_id(None, Some("C_DIRECT"));

    assert_eq!(
        result,
        Some("C_DIRECT"),
        "bare channel_id should be returned as-is when workspace_id is absent"
    );
}

/// Both params absent → None.
#[test]
fn both_params_absent_returns_none() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let toml = base_toml(tmp.path().to_str().expect("utf8"));

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    assert_eq!(config.resolve_channel_id(None, None), None);
}

// ── S031: workspace_id takes precedence ──────────────────────────────────────

/// When both `workspace_id` and `channel_id` are supplied, `workspace_id`
/// wins and `channel_id` is ignored.
#[test]
fn workspace_id_takes_precedence_over_channel_id() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = base_toml(tmp.path().to_str().expect("utf8"));
    let toml = with_mapping(&base, "my-repo", "C_WORKSPACE", None);

    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");
    let result = config.resolve_channel_id(Some("my-repo"), Some("C_IGNORED"));

    assert_eq!(
        result,
        Some("C_WORKSPACE"),
        "workspace_id must take precedence over channel_id"
    );
}
