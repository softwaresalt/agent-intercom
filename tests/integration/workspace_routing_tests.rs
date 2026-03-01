//! Integration tests for workspace-to-channel routing (Phase 6, T044)
//! and multi-session channel routing (Phase 8, T064).
//!
//! Covers:
//! - S034: Config hot-reload updates workspace mappings for new connections
//! - S035: Concurrent sessions in different workspaces resolve independently
//! - S048: Three sessions in three channels each route to the correct session

use std::sync::Arc;
use std::time::Duration;

use agent_intercom::config::{GlobalConfig, WorkspaceMapping};
use agent_intercom::config_watcher::ConfigWatcher;
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::{db, session_repo::SessionRepo};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Minimal TOML containing only `[[workspace]]` entries.
///
/// `ConfigWatcher` uses a lightweight parser that does not require a full
/// valid `GlobalConfig`, so we do not need `default_workspace_root` etc.
fn workspace_only_toml(workspace_id: &str, channel_id: &str) -> String {
    format!("[[workspace]]\nworkspace_id = \"{workspace_id}\"\nchannel_id = \"{channel_id}\"\n")
}

/// Build minimal TOML with two workspace mappings.
fn two_workspace_toml(ws1: &str, ch1: &str, ws2: &str, ch2: &str, workspace_root: &str) -> String {
    format!(
        r#"
default_workspace_root = '{root}'
http_port = 3000
ipc_name = "test"
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
workspace_id = "{ws1}"
channel_id = "{ch1}"

[[workspace]]
workspace_id = "{ws2}"
channel_id = "{ch2}"
"#,
        root = workspace_root.replace('\\', "\\\\"),
        ws1 = ws1,
        ch1 = ch1,
        ws2 = ws2,
        ch2 = ch2,
    )
}

/// Poll a closure over the shared mappings until `pred` returns `true`
/// or `timeout_ms` elapses.  Returns `true` if the condition was met.
async fn poll_until<F>(
    mappings: &Arc<std::sync::RwLock<Vec<WorkspaceMapping>>>,
    timeout_ms: u64,
    pred: F,
) -> bool
where
    F: Fn(&[WorkspaceMapping]) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
    while tokio::time::Instant::now() < deadline {
        {
            let guard = mappings
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if pred(&guard) {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

// ── S034: hot-reload updates mapping ─────────────────────────────────────────

/// Writing a new `[[workspace]]` entry to config.toml while the watcher is
/// active updates the shared mappings within 2 seconds.
#[tokio::test]
async fn workspace_config_hot_reload_updates_mapping() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config_path = tmp.path().join("config.toml");

    // Write initial config: ws1 → C001.
    std::fs::write(&config_path, workspace_only_toml("ws1", "C001")).expect("write initial");

    // Create watcher — loads initial mappings from the file.
    let watcher = ConfigWatcher::new(&config_path).expect("create ConfigWatcher");
    let mappings = watcher.mappings();

    // Verify initial state.
    {
        let guard = mappings.read().expect("read initial");
        let found = guard
            .iter()
            .find(|m| m.workspace_id == "ws1")
            .map(|m| m.channel_id.as_str());
        assert_eq!(found, Some("C001"), "initial mapping should be ws1 → C001");
    }

    // Overwrite config: ws1 → C999.
    std::fs::write(&config_path, workspace_only_toml("ws1", "C999")).expect("write updated");

    // Poll until hot-reload fires (up to 2 s).
    let updated = poll_until(&mappings, 2_000, |ms: &[WorkspaceMapping]| {
        ms.iter()
            .any(|m| m.workspace_id == "ws1" && m.channel_id == "C999")
    })
    .await;

    assert!(
        updated,
        "workspace mapping ws1 → C999 should hot-reload within 2 s"
    );
}

// ── S035: concurrent sessions resolve independently ───────────────────────────

/// Two workspace → channel mappings resolve to different channels, confirming
/// independent routing for concurrent sessions.
#[test]
fn concurrent_sessions_in_different_workspaces() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");

    let toml = two_workspace_toml("workspace-a", "C_ALPHA", "workspace-b", "C_BETA", root);
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    let ch_a = config.resolve_channel_id(Some("workspace-a"), None);
    let ch_b = config.resolve_channel_id(Some("workspace-b"), None);

    assert_eq!(ch_a, Some("C_ALPHA"), "workspace-a must route to C_ALPHA");
    assert_eq!(ch_b, Some("C_BETA"), "workspace-b must route to C_BETA");
    assert_ne!(
        ch_a, ch_b,
        "different workspaces must resolve to different channels"
    );
}

/// Resolving one workspace does not affect the other.
#[test]
fn workspace_resolution_is_independent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");

    let toml = two_workspace_toml("alpha", "CA", "beta", "CB", root);
    let config = GlobalConfig::from_toml_str(&toml).expect("config parses");

    // Resolve alpha — beta should remain unchanged.
    assert_eq!(config.resolve_channel_id(Some("alpha"), None), Some("CA"));
    assert_eq!(config.resolve_channel_id(Some("beta"), None), Some("CB"));

    // Unknown workspace returns None regardless of which other workspaces exist.
    assert_eq!(
        config.resolve_channel_id(Some("gamma"), Some("C_IGNORED")),
        None
    );
}

// ── T064 / S048 ───────────────────────────────────────────────────────────────

/// Three concurrent sessions in three different channels each route to the
/// correct session when `find_active_by_channel` is called for each channel
/// (S048 — independent multi-session channel routing).
#[tokio::test]
async fn three_sessions_in_three_channels_route_correctly() {
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&database));

    // ── Create three sessions, each bound to a distinct channel ──────────
    let channels = ["C_ALPHA", "C_BETA", "C_GAMMA"];
    let mut session_ids = Vec::with_capacity(3);

    for (i, &ch) in channels.iter().enumerate() {
        let mut session = Session::new(
            format!("U_OWNER_{i}"),
            format!("/workspace/s{i}"),
            None,
            SessionMode::Remote,
        );
        session.channel_id = Some(ch.to_owned());
        let created = repo.create(&session).await.expect("create");
        repo.update_status(&created.id, SessionStatus::Active)
            .await
            .expect("activate");
        session_ids.push(created.id);
    }

    // ── Verify each channel resolves to its own session ───────────────────
    for (i, &ch) in channels.iter().enumerate() {
        let results = repo
            .find_active_by_channel(ch)
            .await
            .expect("find_active_by_channel");

        assert_eq!(
            results.len(),
            1,
            "channel {ch} must have exactly one active session (S048)"
        );
        assert_eq!(
            results[0].id, session_ids[i],
            "channel {ch} must route to session {} (S048)",
            session_ids[i]
        );
    }

    // ── Cross-check: no channel bleeds into another ───────────────────────
    let alpha_results = repo
        .find_active_by_channel("C_ALPHA")
        .await
        .expect("find alpha");
    assert_eq!(alpha_results[0].channel_id.as_deref(), Some("C_ALPHA"));

    let beta_results = repo
        .find_active_by_channel("C_BETA")
        .await
        .expect("find beta");
    assert_eq!(beta_results[0].channel_id.as_deref(), Some("C_BETA"));

    let gamma_results = repo
        .find_active_by_channel("C_GAMMA")
        .await
        .expect("find gamma");
    assert_eq!(gamma_results[0].channel_id.as_deref(), Some("C_GAMMA"));

    // Ensure the three sessions are all different.
    let ids: std::collections::HashSet<_> = session_ids.iter().collect();
    assert_eq!(ids.len(), 3, "all three session IDs must be distinct");
}
