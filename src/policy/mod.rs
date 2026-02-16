//! Workspace auto-approve policy modules.
//!
//! Provides policy loading from `.monocoque/settings.json`, evaluation
//! of auto-approve rules, and hot-reload via file system watching.

pub mod evaluator;
pub mod loader;
pub mod watcher;
