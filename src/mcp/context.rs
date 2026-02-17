//! Session context resolution for MCP tool handlers.
//!
//! Provides [`ToolContext`] — a per-request bundle of the active session,
//! workspace root, and shared infrastructure references needed by every
//! tool handler.

use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::config::GlobalConfig;
use crate::models::session::Session;
use crate::slack::client::SlackService;

/// Per-request context available to every MCP tool handler.
///
/// Created by resolving the active session from the MCP transport
/// metadata and bundling it with shared infrastructure references.
pub struct ToolContext {
    /// Active session for this request.
    pub session: Session,
    /// Absolute workspace root for path validation.
    pub workspace_root: PathBuf,
    /// Global configuration.
    pub config: Arc<GlobalConfig>,
    /// `SQLite` connection pool.
    pub db: Arc<SqlitePool>,
    /// Slack client (absent in local-only mode).
    pub slack: Option<Arc<SlackService>>,
}

impl ToolContext {
    /// Construct a new tool context.
    #[must_use]
    pub fn new(
        session: Session,
        workspace_root: PathBuf,
        config: Arc<GlobalConfig>,
        db: Arc<SqlitePool>,
        slack: Option<Arc<SlackService>>,
    ) -> Self {
        Self {
            session,
            workspace_root,
            config,
            db,
            slack,
        }
    }
}
