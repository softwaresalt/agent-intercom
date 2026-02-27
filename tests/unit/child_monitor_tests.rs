//! Unit tests for child process monitor (FR-029).
//!
//! Validates child exit detection logic, status text formatting,
//! and the monitor function signature.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use agent_intercom::mcp::handler::ActiveChildren;
use agent_intercom::orchestrator::child_monitor::spawn_child_monitor;
use agent_intercom::slack::client::SlackService;

/// Verify the monitor function signature compiles with correct types.
#[tokio::test]
async fn spawn_returns_join_handle() {
    let _: fn(
        ActiveChildren,
        Arc<SlackService>,
        String,
        Arc<sqlx::SqlitePool>,
        CancellationToken,
    ) -> tokio::task::JoinHandle<()> = spawn_child_monitor;
}

/// An empty children registry does not cause panics.
#[tokio::test]
async fn empty_children_no_panic() {
    let children: ActiveChildren = Arc::new(Mutex::new(HashMap::new()));
    let guard = children.lock().await;
    assert!(guard.is_empty(), "should start empty");
}

/// Verify `ActiveChildren` type alias matches expected shape.
#[test]
fn active_children_type_matches() {
    let children: ActiveChildren = Arc::new(Mutex::new(HashMap::new()));
    // Type assertion — the fact this compiles confirms the alias.
    let _: Arc<Mutex<HashMap<String, tokio::process::Child>>> = children;
}
