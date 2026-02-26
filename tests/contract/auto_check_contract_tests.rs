//! Contract tests for `auto_check` cache-based policy evaluation (T049).
//!
//! Validates that the workspace [`PolicyCache`] behaves as a write-then-read
//! contract — the value stored is the value retrieved.  These tests stand in for
//! end-to-end handler tests (which require a live MCP transport) by verifying
//! the cache API contract that `check_auto_approve` now depends on.
//!
//! # Scenarios covered
//!
//! | ID   | Scenario |
//! |------|----------|
//! | S043 | Cache update (simulating hot-reload) provides the new policy on next read |
//! | S044 | Cache hit: pre-populated `PolicyCache` returns the stored `CompiledWorkspacePolicy` |
//!
//! # Role in the architecture
//!
//! The `check_auto_approve` tool handler (after T052) reads policy from
//! `AppState.policy_cache` first, falling back to disk only on a cache miss.
//! These tests verify that the cache data-structure correctly supports that
//! read path — if the cache contract holds, the handler's cache-read branch
//! is sound.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use agent_intercom::models::policy::{CompiledWorkspacePolicy, WorkspacePolicy};
use agent_intercom::policy::watcher::PolicyCache;
use tokio::sync::RwLock;

/// Create a fresh, empty [`PolicyCache`].
fn empty_cache() -> PolicyCache {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Build a simple enabled `CompiledWorkspacePolicy` with the given command patterns.
fn make_policy(commands: &[&str]) -> CompiledWorkspacePolicy {
    CompiledWorkspacePolicy::from_policy(WorkspacePolicy {
        enabled: true,
        auto_approve_commands: commands.iter().map(|s| (*s).to_owned()).collect(),
        ..WorkspacePolicy::default()
    })
}

// ── S044: cache hit returns the stored policy ─────────────────────────────────

/// S044 — A `PolicyCache` pre-populated for a workspace root returns the stored
/// `CompiledWorkspacePolicy` on the next read without any disk access.
///
/// After T052 the `check_auto_approve` handler will call
/// `cache.read().await.get(&workspace_root)` before falling back to disk.
/// This test verifies that read path returns the correct value.
#[tokio::test]
async fn cache_hit_returns_stored_policy() {
    let cache = empty_cache();
    let ws_root = PathBuf::from("/workspace/project");
    let policy = make_policy(&["^cargo test$"]);

    // Write the policy into the cache.
    {
        let mut guard = cache.write().await;
        guard.insert(ws_root.clone(), policy.clone());
    }

    // Read it back — simulates what `check_auto_approve` will do in T052.
    let hit = {
        let guard = cache.read().await;
        guard.get(&ws_root).cloned()
    };

    assert!(
        hit.is_some(),
        "cache must return a value for a populated workspace root"
    );
    let cached = hit.expect("cache hit");
    assert!(cached.raw.enabled, "cached policy must be enabled");
    assert_eq!(
        cached.command_patterns,
        vec!["^cargo test$".to_owned()],
        "cached patterns must match what was stored"
    );
    assert!(
        cached.command_set.is_match("cargo test"),
        "pre-compiled set must match"
    );
}

/// S044 — Cache miss for an unregistered workspace root returns `None`, allowing
/// the caller to fall back to disk loading.
#[tokio::test]
async fn cache_miss_returns_none_for_unknown_workspace() {
    let cache = empty_cache();
    let known = PathBuf::from("/workspace/known");
    let unknown = PathBuf::from("/workspace/unknown");

    {
        let mut guard = cache.write().await;
        guard.insert(known, make_policy(&["^git push$"]));
    }

    let miss = {
        let guard = cache.read().await;
        guard.get(&unknown).cloned()
    };

    assert!(
        miss.is_none(),
        "cache must return None for an unknown workspace root"
    );
}

// ── S043: cache update (hot-reload simulation) reflects new policy ────────────

/// S043 — When the policy watcher fires (hot-reload), the cache is updated
/// in-place.  Subsequent reads return the new `CompiledWorkspacePolicy`, not
/// the stale one.
///
/// The `check_auto_approve` handler reads from the cache on every call, so
/// an update takes effect on the *next* invocation without a restart.
#[tokio::test]
async fn cache_update_reflects_new_policy_immediately() {
    let cache = empty_cache();
    let ws_root = PathBuf::from("/workspace/hot-reload");

    // Initial policy: only "cargo test" allowed.
    let v1 = make_policy(&["^cargo test$"]);
    {
        let mut guard = cache.write().await;
        guard.insert(ws_root.clone(), v1);
    }

    // Read v1.
    let read_v1 = {
        let guard = cache.read().await;
        guard.get(&ws_root).cloned().expect("v1 must be present")
    };
    assert!(
        read_v1.command_set.is_match("cargo test"),
        "v1 must match 'cargo test'"
    );
    assert!(
        !read_v1.command_set.is_match("git push"),
        "v1 must not match 'git push'"
    );

    // Hot-reload: replace with v2 that adds "git push".
    let v2 = make_policy(&["^cargo test$", "^git push$"]);
    {
        let mut guard = cache.write().await;
        guard.insert(ws_root.clone(), v2);
    }

    // Read v2 — must reflect the update.
    let read_v2 = {
        let guard = cache.read().await;
        guard.get(&ws_root).cloned().expect("v2 must be present")
    };
    assert!(
        read_v2.command_set.is_match("cargo test"),
        "v2 must still match 'cargo test'"
    );
    assert!(
        read_v2.command_set.is_match("git push"),
        "v2 must match 'git push' after reload"
    );
    assert_eq!(
        read_v2.command_patterns.len(),
        2,
        "v2 must have 2 compiled patterns"
    );
}

/// S043 — Removing (clearing) a workspace entry from the cache causes the next
/// read to fall back to disk, preventing stale policy from persisting after
/// session termination.
#[tokio::test]
async fn cache_removal_causes_cache_miss() {
    let cache = empty_cache();
    let ws_root = PathBuf::from("/workspace/removed");

    {
        let mut guard = cache.write().await;
        guard.insert(ws_root.clone(), make_policy(&["^cargo test$"]));
    }

    // Verify it's present first.
    {
        let guard = cache.read().await;
        assert!(
            guard.get(&ws_root).is_some(),
            "entry must exist before removal"
        );
    }

    // Remove the entry (simulates session termination / watcher unregistration).
    {
        let mut guard = cache.write().await;
        guard.remove(&ws_root);
    }

    let after_removal = {
        let guard = cache.read().await;
        guard.get(&ws_root).cloned()
    };
    assert!(
        after_removal.is_none(),
        "removed entry must cause a cache miss"
    );
}
