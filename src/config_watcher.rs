//! Hot-reload watcher for workspace-to-channel mappings in `config.toml`.
//!
//! [`ConfigWatcher`] uses the `notify` crate to watch `config.toml` for
//! file-system changes.  When a change is detected it re-parses the
//! `[[workspace]]` entries (only — it does not re-validate the full
//! [`GlobalConfig`]) and atomically updates the shared
//! `Arc<RwLock<Vec<WorkspaceMapping>>>`.
//!
//! This design keeps the hot-reload scope minimal: workspace mappings can
//! be reconfigured without a server restart, while all other configuration
//! fields require a restart.
//!
//! ## Usage
//!
//! ```no_run
//! use std::path::Path;
//! use agent_intercom::config_watcher::ConfigWatcher;
//!
//! let watcher = ConfigWatcher::new(Path::new("config.toml")).expect("watcher");
//! let mappings = watcher.mappings(); // Arc<RwLock<Vec<WorkspaceMapping>>>
//! ```
//!
//! ## Thread safety
//!
//! The shared `Arc<std::sync::RwLock<Vec<WorkspaceMapping>>>` is updated
//! from inside a synchronous `notify` callback.  The `std::sync::RwLock`
//! (not `tokio::sync::RwLock`) is used intentionally so that the callback
//! does not need an async context.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use tracing::{info, warn};

use crate::config::WorkspaceMapping;
use crate::{AppError, Result};

/// Minimal TOML structure used for hot-reload parsing.
///
/// Only the `[[workspace]]` array is extracted; all other fields are ignored.
/// This avoids requiring a fully valid `GlobalConfig` (e.g. an existing
/// `default_workspace_root`) when re-reading mappings after a file change.
#[derive(Debug, Deserialize)]
struct MappingsOnlyConfig {
    #[serde(default, rename = "workspace")]
    workspace: Vec<WorkspaceMapping>,
}

/// Parse only the `[[workspace]]` entries from a TOML file.
///
/// Unknown fields in the file are silently ignored, so the full
/// `config.toml` can be passed without triggering "unknown field" errors.
///
/// # Errors
///
/// Returns `AppError::Config` if the file cannot be read or contains
/// invalid TOML in the `[[workspace]]` sections.
pub(crate) fn parse_workspace_mappings(path: &Path) -> Result<Vec<WorkspaceMapping>> {
    let raw = std::fs::read_to_string(path).map_err(|err| {
        AppError::Config(format!(
            "failed to read config for workspace hot-reload: {err}"
        ))
    })?;
    // Use `toml::from_str` with `ignored_any` via the struct's `deny_unknown_fields`
    // absent by default — unknown keys are silently dropped.
    let parsed: MappingsOnlyConfig = toml::from_str(&raw).map_err(|err| {
        AppError::Config(format!(
            "failed to parse workspace mappings from config: {err}"
        ))
    })?;
    Ok(parsed.workspace)
}

/// Returns `true` for file-system events that indicate the watched file was
/// written or replaced (create, modify, remove).
fn is_config_change(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

/// Hot-reload watcher for workspace-to-channel mappings.
///
/// Holds a [`notify`] file-system watcher alive for its own lifetime.
/// Dropping a `ConfigWatcher` stops the underlying OS watch, so callers
/// must keep the watcher alive for as long as hot-reload is needed.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use agent_intercom::config_watcher::ConfigWatcher;
///
/// let watcher = ConfigWatcher::new(Path::new("config.toml")).expect("watcher");
/// let mappings = watcher.mappings();
///
/// // Later, in SSE factory or test:
/// let guard = mappings.read().expect("read");
/// for m in guard.iter() {
///     println!("{} → {}", m.workspace_id, m.channel_id);
/// }
/// ```
pub struct ConfigWatcher {
    /// Underlying notify watcher — kept alive by owning it here.
    _watcher: RecommendedWatcher,
    /// Shared, hot-reloadable workspace mapping list.
    mappings: Arc<RwLock<Vec<WorkspaceMapping>>>,
}

impl ConfigWatcher {
    /// Create a new `ConfigWatcher` that watches `config_path` for changes.
    ///
    /// Parses the initial `[[workspace]]` entries from the file at creation
    /// time.  If the file cannot be read or parsed, the watcher starts with
    /// an empty mapping list and logs a warning — this is treated as
    /// non-fatal so the server can still start without workspace mappings.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Config` if the `notify` watcher itself cannot be
    /// created (OS resource exhaustion or unsupported file system).
    pub fn new(config_path: &Path) -> Result<Self> {
        // Load the initial mappings — non-fatal on failure.
        let initial = parse_workspace_mappings(config_path).unwrap_or_else(|err| {
            warn!(%err, path = %config_path.display(), "failed to load initial workspace mappings; starting empty");
            Vec::new()
        });

        let mappings: Arc<RwLock<Vec<WorkspaceMapping>>> = Arc::new(RwLock::new(initial));
        let mappings_for_callback = Arc::clone(&mappings);
        let path_for_callback: PathBuf = config_path.to_path_buf();

        let mut watcher = notify::recommended_watcher(
            move |result: std::result::Result<Event, notify::Error>| match result {
                Ok(event) if is_config_change(&event) => {
                    match parse_workspace_mappings(&path_for_callback) {
                        Ok(new_mappings) => match mappings_for_callback.write() {
                            Ok(mut guard) => {
                                *guard = new_mappings;
                                info!(
                                    path = %path_for_callback.display(),
                                    "hot-reloaded workspace mappings from config"
                                );
                            }
                            Err(err) => {
                                warn!(
                                    %err,
                                    "workspace mappings RwLock poisoned during hot-reload"
                                );
                            }
                        },
                        Err(err) => {
                            warn!(
                                %err,
                                path = %path_for_callback.display(),
                                "failed to reload workspace mappings — keeping previous values"
                            );
                        }
                    }
                }
                Err(err) => {
                    warn!(%err, "config file watcher error");
                }
                _ => {}
            },
        )
        .map_err(|err| AppError::Config(format!("failed to create config file watcher: {err}")))?;

        // Watch the directory containing config.toml (not the file itself) so
        // that atomic rename-based writes (write to temp + rename) are detected.
        // Fall back to watching the file directly when the parent is unavailable.
        let watch_target = config_path
            .parent()
            .filter(|p| p != &Path::new(""))
            .unwrap_or(config_path);

        watcher
            .watch(watch_target, RecursiveMode::NonRecursive)
            .map_err(|err| {
                AppError::Config(format!(
                    "failed to watch config path '{}': {err}",
                    watch_target.display()
                ))
            })?;

        info!(
            path = %config_path.display(),
            "config watcher started for workspace mapping hot-reload"
        );

        Ok(Self {
            _watcher: watcher,
            mappings,
        })
    }

    /// Return a clone of the shared workspace mappings `Arc`.
    ///
    /// The returned handle can be stored in [`crate::mcp::handler::AppState`]
    /// or passed to the SSE factory so that every new session sees the
    /// latest mappings.
    #[must_use]
    pub fn mappings(&self) -> Arc<RwLock<Vec<WorkspaceMapping>>> {
        Arc::clone(&self.mappings)
    }

    /// Resolve the effective Slack channel from `workspace_id` / `channel_id`
    /// using the current (possibly hot-reloaded) mapping table.
    ///
    /// See [`GlobalConfig::resolve_channel_id`] for the full resolution rules.
    #[must_use]
    pub fn resolve_channel_id(
        &self,
        workspace_id: Option<&str>,
        channel_id: Option<&str>,
    ) -> Option<String> {
        let guard = self
            .mappings
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(ws_id) = workspace_id {
            guard
                .iter()
                .find(|m| m.workspace_id == ws_id)
                .map(|m| m.channel_id.clone())
        } else {
            channel_id.map(str::to_owned)
        }
    }
}
