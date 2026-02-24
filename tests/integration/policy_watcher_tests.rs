//! Integration tests for policy hot-reload via `PolicyWatcher`.
//!
//! Validates:
//! - S045: Register loads initial policy from `settings.json`
//! - S046: File modification is detected and policy cache updated
//! - S047: File deletion falls back to deny-all default
//! - S048: Malformed JSON file falls back to deny-all default
//! - S049: Unregister stops watching for changes
//! - S050: Multiple workspaces have independent policies
//!
//! FR-007 — Policy Hot-Reload

use std::time::Duration;

use agent_intercom::policy::watcher::PolicyWatcher;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Create a `.intercom/settings.json` file in the given directory with the provided JSON body.
fn write_policy_file(dir: &std::path::Path, json: &str) {
    let agentrc = dir.join(".intercom");
    std::fs::create_dir_all(&agentrc).expect("create .intercom dir");
    std::fs::write(agentrc.join("settings.json"), json).expect("write settings.json");
}

/// Poll `get_policy()` for the workspace root until the predicate returns `true`,
/// or until `timeout_ms` milliseconds have elapsed.  Returns `true` if the
/// condition was met within the deadline, `false` otherwise.
async fn poll_until<F>(
    watcher: &PolicyWatcher,
    root: &std::path::Path,
    timeout_ms: u64,
    pred: F,
) -> bool
where
    F: Fn(&agent_intercom::models::policy::WorkspacePolicy) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
    while tokio::time::Instant::now() < deadline {
        let policy = watcher.get_policy(root).await;
        if pred(&policy) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

// ── S045: register loads initial policy ──────────────────────────────────────

#[tokio::test]
async fn register_loads_initial_policy() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    write_policy_file(
        root,
        r#"{"enabled": true, "auto_approve_commands": ["cargo test"]}"#,
    );

    let watcher = PolicyWatcher::new();
    watcher.register(root).await.expect("register");

    let policy = watcher.get_policy(root).await;
    assert!(policy.enabled, "initial policy should have enabled=true");
    assert!(
        policy
            .auto_approve_commands
            .contains(&"cargo test".to_owned()),
        "initial policy should include 'cargo test' command"
    );
}

// ── S046: file modification detected ─────────────────────────────────────────

#[tokio::test]
async fn policy_file_modification_detected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    write_policy_file(root, r#"{"enabled": false}"#);

    let watcher = PolicyWatcher::new();
    watcher.register(root).await.expect("register");

    // Confirm initial state.
    let initial = watcher.get_policy(root).await;
    assert!(!initial.enabled, "initial policy should have enabled=false");

    // Modify the file.
    write_policy_file(root, r#"{"enabled": true}"#);

    // Poll until the hot-reload fires (up to 2 s).
    let updated = poll_until(&watcher, root, 2_000, |p| p.enabled).await;
    assert!(
        updated,
        "policy should have been hot-reloaded to enabled=true within 2 s"
    );
}

// ── S047: file deletion falls back to deny-all ────────────────────────────────

#[tokio::test]
async fn policy_file_deletion_falls_back_to_deny_all() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    write_policy_file(root, r#"{"enabled": true}"#);

    let watcher = PolicyWatcher::new();
    watcher.register(root).await.expect("register");

    // Confirm enabled initially.
    let initial = watcher.get_policy(root).await;
    assert!(initial.enabled, "initial policy should have enabled=true");

    // Delete the policy file.
    let policy_path = root.join(".intercom").join("settings.json");
    std::fs::remove_file(&policy_path).expect("remove settings.json");

    // Poll until the deny-all default is reflected (up to 2 s).
    let fell_back = poll_until(&watcher, root, 2_000, |p| !p.enabled).await;
    assert!(
        fell_back,
        "policy should have fallen back to deny-all within 2 s after file deletion"
    );
}

// ── S048: malformed JSON falls back to deny-all ───────────────────────────────

#[tokio::test]
async fn malformed_policy_file_uses_deny_all() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // Write invalid JSON before registering.
    write_policy_file(root, r"{ this is not valid json }");

    let watcher = PolicyWatcher::new();
    watcher
        .register(root)
        .await
        .expect("register should succeed even with malformed JSON");

    // PolicyLoader falls back to deny-all on parse errors — no polling needed.
    let policy = watcher.get_policy(root).await;
    assert!(
        !policy.enabled,
        "malformed policy file should result in deny-all (enabled=false)"
    );
}

// ── S049: unregister stops watching ──────────────────────────────────────────

#[tokio::test]
async fn unregister_stops_watching() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    write_policy_file(root, r#"{"enabled": false}"#);

    let watcher = PolicyWatcher::new();
    watcher.register(root).await.expect("register");

    // Unregister — this removes the watcher and clears the cache entry.
    watcher.unregister(root).await;

    // Allow time for the OS-level watcher deregistration to complete so no
    // stale events fire after we write to disk.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Modify the file — the watcher is gone so no hot-reload should fire.
    write_policy_file(root, r#"{"enabled": true}"#);

    // Wait long enough that a stale watcher could have fired.
    tokio::time::sleep(Duration::from_millis(300)).await;

    // After unregister the cache is cleared, so get_policy returns the deny-all default —
    // not the enabled:true from the file.
    let policy = watcher.get_policy(root).await;
    assert!(
        !policy.enabled,
        "after unregister, get_policy should return deny-all default, not the updated file value"
    );
}

// ── S050: multiple workspaces have independent policies ───────────────────────

#[tokio::test]
async fn multiple_workspaces_independent_policies() {
    let tmp1 = tempfile::tempdir().expect("tempdir 1");
    let tmp2 = tempfile::tempdir().expect("tempdir 2");
    let root1 = tmp1.path();
    let root2 = tmp2.path();

    write_policy_file(root1, r#"{"enabled": false}"#);
    write_policy_file(root2, r#"{"enabled": false}"#);

    let watcher = PolicyWatcher::new();
    watcher.register(root1).await.expect("register workspace 1");
    watcher.register(root2).await.expect("register workspace 2");

    // Confirm both start as disabled.
    assert!(
        !watcher.get_policy(root1).await.enabled,
        "ws1 initial: disabled"
    );
    assert!(
        !watcher.get_policy(root2).await.enabled,
        "ws2 initial: disabled"
    );

    // Modify only workspace 1.
    write_policy_file(root1, r#"{"enabled": true}"#);

    // Poll until workspace 1 hot-reloads (up to 2 s).
    let ws1_updated = poll_until(&watcher, root1, 2_000, |p| p.enabled).await;
    assert!(
        ws1_updated,
        "workspace 1 policy should have updated to enabled=true"
    );

    // Workspace 2 must remain unchanged.
    let ws2 = watcher.get_policy(root2).await;
    assert!(
        !ws2.enabled,
        "workspace 2 policy should still be disabled after modifying workspace 1"
    );
}
