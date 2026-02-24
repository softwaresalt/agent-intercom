//! Hot-reload file watcher for workspace policy files (T063).
//!
//! Watches `.intercom/settings.json` for each active workspace root
//! using the `notify` crate. On change events, reloads the policy via
//! [`PolicyLoader`] and updates the in-memory cache (FR-010).
//!
//! Watchers are registered when sessions start and unregistered when
//! sessions terminate.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{Mutex, RwLock};
use tracing::{info, info_span, warn};

use crate::models::policy::WorkspacePolicy;
use crate::policy::loader::PolicyLoader;

/// Relative path within a workspace root to the policy file.
const POLICY_FILENAME: &str = "settings.json";
const POLICY_DIR: &str = ".intercom";

/// Thread-safe in-memory policy cache keyed by workspace root.
pub type PolicyCache = Arc<RwLock<HashMap<PathBuf, WorkspacePolicy>>>;

/// Manages file watchers for workspace policy hot-reload.
pub struct PolicyWatcher {
    /// Active watchers keyed by workspace root path.
    watchers: Arc<Mutex<HashMap<PathBuf, RecommendedWatcher>>>,
    /// Shared policy cache updated on file changes.
    cache: PolicyCache,
}

impl Default for PolicyWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyWatcher {
    /// Create a new policy watcher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            watchers: Arc::new(Mutex::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a reference to the shared policy cache.
    #[must_use]
    pub fn cache(&self) -> &PolicyCache {
        &self.cache
    }

    /// Load the initial policy for a workspace and start watching for changes.
    ///
    /// # Errors
    ///
    /// Returns an error if the file watcher cannot be created. Policy loading
    /// errors are non-fatal (deny-all fallback).
    pub async fn register(&self, workspace_root: &Path) -> crate::Result<()> {
        let _span = info_span!(
            "policy_watcher_register",
            workspace = %workspace_root.display(),
        )
        .entered();

        let canonical = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_owned());

        // ── Load initial policy ──────────────────────────────
        let policy = PolicyLoader::load(&canonical)?;
        {
            let mut cache = self.cache.write().await;
            cache.insert(canonical.clone(), policy);
        }
        info!("loaded initial workspace policy");

        // ── Set up file watcher ──────────────────────────────
        let watch_dir = canonical.join(POLICY_DIR);
        let cache = Arc::clone(&self.cache);
        let root = canonical.clone();

        let mut watcher = notify::recommended_watcher(
            move |result: std::result::Result<Event, notify::Error>| {
                match result {
                    Ok(event) => {
                        if is_policy_event(&event) {
                            let _span = info_span!(
                                "policy_reload",
                                workspace = %root.display(),
                            )
                            .entered();

                            match PolicyLoader::load(&root) {
                                Ok(new_policy) => {
                                    // Use blocking write since we're in a sync callback.
                                    // This is safe because the RwLock is tokio-based but
                                    // we use blocking_write for sync contexts.
                                    let mut guard = cache.blocking_write();
                                    guard.insert(root.clone(), new_policy);
                                    info!("hot-reloaded workspace policy");
                                }
                                Err(err) => {
                                    warn!(%err, "failed to reload workspace policy");
                                }
                            }
                        }
                    }
                    Err(err) => {
                        warn!(%err, "file watcher error");
                    }
                }
            },
        )
        .map_err(|err| crate::AppError::Policy(format!("failed to create watcher: {err}")))?;

        // Watch the .intercom directory (create it if needed for the watch).
        if watch_dir.exists() {
            watcher
                .watch(&watch_dir, RecursiveMode::NonRecursive)
                .map_err(|err| {
                    crate::AppError::Policy(format!("failed to watch directory: {err}"))
                })?;
        } else {
            // The directory doesn't exist yet. The watcher is stored but
            // will not receive events until the directory is created.
            // Callers should re-register after creating the directory,
            // or rely on the loader's fallback to the default deny-all policy.
            info!(
                dir = %watch_dir.display(),
                "policy directory does not exist yet, watcher deferred"
            );
        }

        // Store watcher to keep it alive.
        let mut watchers = self.watchers.lock().await;
        watchers.insert(canonical, watcher);

        Ok(())
    }

    /// Stop watching a workspace root and remove its cached policy.
    pub async fn unregister(&self, workspace_root: &Path) {
        let canonical = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_owned());

        let mut watchers = self.watchers.lock().await;
        watchers.remove(&canonical);

        let mut cache = self.cache.write().await;
        cache.remove(&canonical);

        info!(
            workspace = %canonical.display(),
            "unregistered policy watcher"
        );
    }

    /// Get the current policy for a workspace root, or deny-all if not cached.
    pub async fn get_policy(&self, workspace_root: &Path) -> WorkspacePolicy {
        let canonical = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_owned());

        let cache = self.cache.read().await;
        cache.get(&canonical).cloned().unwrap_or_default()
    }
}

/// Check whether a notify event relates to the policy file.
fn is_policy_event(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && event
        .paths
        .iter()
        .any(|p| p.file_name().is_some_and(|name| name == POLICY_FILENAME))
}
