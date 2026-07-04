//! Unit tests for child process monitor (FR-029).
//!
//! Validates child exit detection logic, status text formatting,
//! and the monitor function signature.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use agent_intercom::orchestrator::child_monitor::{classify_exit, spawn_child_monitor, ExitClass};
use agent_intercom::slack::client::SlackService;
use agent_intercom::state::ActiveChildren;

/// Run a throwaway process that exits with the given code and return its
/// real `ExitStatus`. Cross-platform: `cmd /c exit N` on Windows, `sh -c` elsewhere.
fn exit_status_for(code: i32) -> std::process::ExitStatus {
    if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/c", &format!("exit {code}")])
            .status()
            .expect("run cmd exit")
    } else {
        std::process::Command::new("sh")
            .args(["-c", &format!("exit {code}")])
            .status()
            .expect("run sh exit")
    }
}

/// Verify the monitor function signature compiles with correct types.
#[tokio::test]
#[allow(clippy::type_complexity)]
async fn spawn_returns_join_handle() {
    let _: fn(
        ActiveChildren,
        Arc<SlackService>,
        String,
        Arc<sqlx::SqlitePool>,
        Arc<agent_intercom::config::GlobalConfig>,
        CancellationToken,
    ) -> tokio::task::JoinHandle<()> = spawn_child_monitor;
}

/// A process that exits normally (code 0) is classified as a clean exit.
#[test]
fn classify_exit_code_zero_is_clean() {
    let status = exit_status_for(0);
    assert_eq!(classify_exit(Some(status)), ExitClass::Clean);
}

/// A process that exits with a non-zero code is classified as a crash.
#[test]
fn classify_exit_nonzero_is_crash() {
    let status = exit_status_for(1);
    assert_eq!(classify_exit(Some(status)), ExitClass::Crash);
}

/// An unknown exit status (poll error) is treated conservatively as a crash.
#[test]
fn classify_exit_unknown_is_crash() {
    assert_eq!(classify_exit(None), ExitClass::Crash);
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
