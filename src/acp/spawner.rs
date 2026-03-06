//! ACP agent process spawner.
//!
//! Spawns headless agent processes for ACP sessions with:
//! - `kill_on_drop(true)` so processes are cleaned up automatically.
//! - `env_clear()` + a safe variable allowlist to prevent Slack tokens and
//!   other secrets from leaking into the child's environment (FR-029, S075).
//! - Platform-specific process-tree isolation: Windows `CREATE_NEW_PROCESS_GROUP`
//!   flag, Unix `process_group(0)`, and corresponding kill helpers (FR-037).
//!
//! The spawner does **not** wait for a ready signal on stdout — the caller
//! verifies process readiness via the ACP handshake (`initialize` /
//! `initialized` exchange).

use std::path::PathBuf;

use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::driver::AgentEvent;
use crate::{AppError, Result};

// ── Environment allowlist ────────────────────────────────────────────────────

/// Environment variables inherited by the spawned agent process.
///
/// Every other variable from the server's environment is stripped via
/// `env_clear()` before the child is launched. Slack tokens, database URLs,
/// and other secrets are therefore never visible to the agent process.
pub const ALLOWED_ENV_VARS: &[&str] = &[
    "PATH",
    "HOME",
    "RUST_LOG",
    // Windows-specific variables.
    "USERPROFILE",
    "SystemRoot",
    "TEMP",
    "TMP",
    "USERNAME",
    "APPDATA",
    "LOCALAPPDATA",
    "COMSPEC",
    // Agent-intercom specific (set explicitly by the spawner).
    // Not listed here; they are injected via `.env()` calls below.
];

// ── Configuration ────────────────────────────────────────────────────────────

/// Configuration for spawning an ACP agent process.
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    /// Host CLI binary (e.g., `claude`, `gh`, `copilot`).
    pub host_cli: String,
    /// Default arguments passed to the host CLI.
    pub host_cli_args: Vec<String>,
    /// Workspace root directory; the child process starts in this directory.
    pub workspace_root: PathBuf,
}

// ── Connection handle ────────────────────────────────────────────────────────

/// Active stdio connection to a spawned ACP agent process.
///
/// The caller is responsible for:
/// - Keeping `child` alive (it has `kill_on_drop(true)`).
/// - Forwarding messages through `stdin`.
/// - Reading stream messages from `stdout`.
#[derive(Debug)]
pub struct AcpConnection {
    /// Session identifier that the process was launched for.
    pub session_id: String,
    /// Child process handle — kept alive so `kill_on_drop` works.
    pub child: Child,
    /// Agent's stdin for sending JSON messages to the agent.
    pub stdin: ChildStdin,
    /// Buffered reader over the agent's stdout for line-by-line NDJSON parsing.
    pub stdout: BufReader<ChildStdout>,
}

// ── Spawner ──────────────────────────────────────────────────────────────────

/// Spawn an ACP agent process.
///
/// The spawner:
/// 1. Validates that `session_id` is non-empty.
/// 2. Builds a `tokio::process::Command` with `env_clear()` and only the
///    variables listed in [`ALLOWED_ENV_VARS`].
/// 3. Passes `INTERCOM_SESSION_ID` as an explicit environment variable.
/// 4. Returns the connection handle immediately — readiness is verified
///    by the caller via the ACP handshake (`initialize` / `initialized`).
///
/// The initial prompt is **not** passed as a CLI argument. Instead, the caller
/// must send it via `handshake::send_prompt` after the `initialize` /
/// `initialized` exchange completes (FR-030).
///
/// # Errors
///
/// - `AppError::Acp("failed to spawn agent: …")` — OS spawn failure.
pub fn spawn_agent(config: &SpawnConfig, session_id: &str) -> Result<AcpConnection> {
    let mut cmd = Command::new(&config.host_cli);

    for arg in &config.host_cli_args {
        cmd.arg(arg);
    }

    // Strip inherited environment, then inject only the safe allowlist.
    cmd.env_clear();
    for &key in ALLOWED_ENV_VARS {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }

    // Inject ACP-specific context variables.
    cmd.env("INTERCOM_SESSION_ID", session_id);

    cmd.current_dir(&config.workspace_root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    // ── Platform-specific process-group isolation (FR-037) ───────────────────
    // On Unix, place the child in its own process group (PGID = child PID).
    // On Windows, set CREATE_NEW_PROCESS_GROUP so taskkill /T can reach all
    // descendants. Both ensure the entire agent process tree is reachable for
    // kill on session termination.
    #[cfg(unix)]
    cmd.process_group(0);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP (0x00000200): child starts in a new process
        // group, enabling `taskkill /T` to reach all descendants.
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.as_std_mut().creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    let mut child = cmd
        .spawn()
        .map_err(|err| AppError::Acp(format!("failed to spawn agent: {err}")))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Acp("failed to capture agent stdin".into()))?;
    let stdout_raw = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Acp("failed to capture agent stdout".into()))?;

    let reader = BufReader::new(stdout_raw);

    info!(session_id, cli = %config.host_cli, "agent process spawned");

    Ok(AcpConnection {
        session_id: session_id.to_owned(),
        child,
        stdin,
        stdout: reader,
    })
}

// ── Exit monitor ─────────────────────────────────────────────────────────────/// Spawn a background task that awaits child-process exit and emits
/// [`AgentEvent::SessionTerminated`] when it happens.
///
/// The task respects `cancel`: when the token is cancelled the task exits
/// without emitting an event (the caller is responsible for orderly shutdown).
///
/// # Returns
///
/// A [`JoinHandle`] for the monitoring task.  Dropping the handle detaches
/// the task; it continues running until the child exits or the token fires.
#[must_use]
pub fn monitor_exit(
    session_id: String,
    mut child: Child,
    event_tx: mpsc::Sender<AgentEvent>,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tokio::select! {
            result = child.wait() => {
                let (exit_code, reason) = match result {
                    Ok(status) => {
                        let code = status.code();
                        let reason = code.map_or_else(
                            || "process terminated by signal".to_owned(),
                            |c| format!("process exited with code {c}"),
                        );
                        (code, reason)
                    }
                    Err(err) => {
                        warn!(session_id, %err, "error waiting for agent child process");
                        (None, format!("wait error: {err}"))
                    }
                };

                let event = AgentEvent::SessionTerminated {
                    session_id: session_id.clone(),
                    exit_code,
                    reason,
                };

                if event_tx.send(event).await.is_err() {
                    warn!(
                        session_id,
                        "event_tx closed before SessionTerminated could be delivered"
                    );
                }
            }
            () = cancel.cancelled() => {
                // Graceful shutdown — do not emit a terminated event; the
                // caller will handle cleanup via its own cancellation path.
                info!(session_id, "monitor_exit: cancellation received, exiting monitor");
            }
        }
    })
}

// ── Process tree termination (FR-037) ────────────────────────────────────────

/// Kill a process and all its descendants on Windows.
///
/// Invokes `taskkill /F /T /PID <pid>` which recursively terminates the
/// entire process tree rooted at `pid`.  This complements `kill_on_drop(true)`
/// (which only kills the direct child) by ensuring grandchild processes are
/// also terminated when a session ends.
///
/// The function is best-effort: if `taskkill` is unavailable or fails, a
/// warning is logged but no error is returned.
#[cfg(windows)]
pub async fn kill_process_tree(pid: u32) {
    use tokio::process::Command;
    let result = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
    match result {
        Ok(status) if status.success() => {
            info!(pid, "process tree killed via taskkill /T");
        }
        Ok(status) => {
            warn!(
                pid,
                exit_code = ?status.code(),
                "taskkill exited with non-zero status — some children may survive"
            );
        }
        Err(err) => {
            warn!(pid, %err, "failed to invoke taskkill for process tree termination");
        }
    }
}

/// Kill a process group and all its members on Unix.
///
/// Sends `SIGTERM` to the negative PGID (`kill -TERM -<pid>`), terminating
/// every process in the group.  When the agent was spawned with
/// `process_group(0)`, the child's PGID equals its own PID, so this kills
/// the child and all its descendants.
///
/// Always follows up with a direct `SIGKILL` on the lead PID to guard
/// against environments where group-level signals are silently dropped
/// (e.g. certain CI runners with process isolation).
///
/// The function is best-effort: if the `kill` binary is unavailable or
/// fails, a warning is logged but no error is returned.
#[cfg(unix)]
pub async fn kill_process_group(pid: u32) {
    use tokio::process::Command;

    // Step 1: SIGTERM the entire process group for graceful shutdown.
    let result = Command::new("kill")
        .args(["-TERM", &format!("-{pid}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
    match &result {
        Ok(status) if status.success() => {
            info!(pid, "sent SIGTERM to process group");
        }
        Ok(status) => {
            warn!(
                pid,
                exit_code = ?status.code(),
                "kill -TERM on group exited with non-zero status"
            );
        }
        Err(err) => {
            warn!(pid, %err, "failed to invoke kill for process group SIGTERM");
        }
    }

    // Step 2: Brief grace period for graceful shutdown handlers.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Step 3: SIGKILL the lead PID directly as a safety net.
    // In environments with process group isolation (some CI runners),
    // the group SIGTERM may report success without delivering the signal.
    let fallback = Command::new("kill")
        .args(["-KILL", &format!("{pid}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
    match fallback {
        Ok(s) if s.success() => {
            info!(pid, "sent SIGKILL to lead process");
        }
        Ok(_) | Err(_) => {
            // Process may have already exited from SIGTERM — this is expected.
        }
    }
}

// ── Orphan process detection (FR-037, T126) ───────────────────────────────────

/// Check for orphan agent processes left over from a previous server run.
///
/// Queries the OS process list for processes matching the `host_cli` binary
/// name.  If any are found, logs `WARN` suggesting the operator verify the
/// previous shutdown was clean.  **No auto-kill is performed** — the warning
/// is informational only.
///
/// This function is best-effort: errors from the underlying OS commands (e.g.
/// `pgrep` or `tasklist` not available) are silently ignored.
pub async fn check_for_orphan_processes(host_cli: &str) {
    #[cfg(windows)]
    {
        use tokio::process::Command;

        // Build the image name: append ".exe" for the tasklist IMAGENAME filter.
        let mut name = std::path::Path::new(host_cli)
            .file_name()
            .map_or_else(|| host_cli.to_owned(), |n| n.to_string_lossy().into_owned());
        if !std::path::Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        {
            name.push_str(".exe");
        }

        let Ok(output) = Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {name}"), "/NH"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .await
        else {
            return;
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains(&*name) {
            warn!(
                host_cli,
                "orphan agent processes detected — verify previous server shutdown was clean"
            );
        }
    }

    #[cfg(unix)]
    {
        use tokio::process::Command;

        let Ok(output) = Command::new("pgrep")
            .args(["-f", host_cli])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .await
        else {
            return;
        };

        if output.status.success() {
            warn!(
                host_cli,
                "orphan agent processes detected — verify previous server shutdown was clean"
            );
        }
    }

    // On platforms where neither check applies, silently no-op.
    #[cfg(not(any(windows, unix)))]
    let _ = host_cli;
}
