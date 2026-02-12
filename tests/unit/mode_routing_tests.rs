//! Unit tests for mode-aware message routing (T125).
//!
//! Validates:
//! - Remote mode posts to Slack only
//! - Local mode suppresses Slack and routes to IPC
//! - Hybrid mode posts to both Slack and IPC

use monocoque_agent_rem::models::session::SessionMode;

/// Remote mode should indicate Slack is the channel.
#[test]
fn remote_mode_routes_to_slack() {
    let mode = SessionMode::Remote;
    assert!(should_post_to_slack(mode), "remote mode uses Slack");
    assert!(!should_post_to_ipc(mode), "remote mode does not use IPC");
}

/// Local mode should suppress Slack and use IPC.
#[test]
fn local_mode_suppresses_slack() {
    let mode = SessionMode::Local;
    assert!(!should_post_to_slack(mode), "local mode suppresses Slack");
    assert!(should_post_to_ipc(mode), "local mode uses IPC");
}

/// Hybrid mode should use both Slack and IPC.
#[test]
fn hybrid_mode_uses_both() {
    let mode = SessionMode::Hybrid;
    assert!(should_post_to_slack(mode), "hybrid mode uses Slack");
    assert!(should_post_to_ipc(mode), "hybrid mode uses IPC");
}

/// Verify all mode enum variants have a defined routing behavior.
#[test]
fn all_modes_have_defined_routing() {
    for mode in [SessionMode::Remote, SessionMode::Local, SessionMode::Hybrid] {
        let slack = should_post_to_slack(mode);
        let ipc = should_post_to_ipc(mode);
        // Every mode must have at least one channel active.
        assert!(
            slack || ipc,
            "mode {mode:?} must route to at least one channel"
        );
    }
}

/// Verify mode switching from remote to local changes routing.
#[test]
fn mode_switch_remote_to_local_changes_routing() {
    let before = SessionMode::Remote;
    let after = SessionMode::Local;

    assert!(should_post_to_slack(before));
    assert!(!should_post_to_slack(after));
    assert!(!should_post_to_ipc(before));
    assert!(should_post_to_ipc(after));
}

/// Verify mode switching from local to hybrid adds Slack.
#[test]
fn mode_switch_local_to_hybrid_adds_slack() {
    let before = SessionMode::Local;
    let after = SessionMode::Hybrid;

    assert!(!should_post_to_slack(before));
    assert!(should_post_to_slack(after));
    assert!(should_post_to_ipc(before));
    assert!(should_post_to_ipc(after));
}

/// Verify mode switching from hybrid to remote removes IPC.
#[test]
fn mode_switch_hybrid_to_remote_removes_ipc() {
    let before = SessionMode::Hybrid;
    let after = SessionMode::Remote;

    assert!(should_post_to_slack(before));
    assert!(should_post_to_slack(after));
    assert!(should_post_to_ipc(before));
    assert!(!should_post_to_ipc(after));
}

// ── Routing decision helpers ─────────────────────────────────────────
// These mirror the routing logic that will be implemented in the Slack
// client and IPC server. They exist here to define the contract first.

/// Whether a message should be posted to Slack for the given mode.
fn should_post_to_slack(mode: SessionMode) -> bool {
    matches!(mode, SessionMode::Remote | SessionMode::Hybrid)
}

/// Whether a message should be routed to IPC for the given mode.
fn should_post_to_ipc(mode: SessionMode) -> bool {
    matches!(mode, SessionMode::Local | SessionMode::Hybrid)
}
