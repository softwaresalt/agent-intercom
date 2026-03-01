//! ACP agent process spawner.
//!
//! Spawns headless agent processes for ACP sessions with:
//! - `kill_on_drop(true)` so processes are cleaned up automatically.
//! - `env_clear()` + a safe variable allowlist to prevent Slack tokens and
//!   other secrets from leaking into the child's environment (FR-029, S075).
//! - A configurable startup timeout: if the agent does not emit its ready
//!   signal (first stdout line) within the window, the process is killed and
//!   `AppError::Acp("startup timeout")` is returned.

use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
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
    /// Host CLI binary (e.g., `claude`, `gh`, `python`).
    pub host_cli: String,
    /// Default arguments passed to the host CLI before the prompt.
    pub host_cli_args: Vec<String>,
    /// Workspace root directory; the child process starts in this directory.
    pub workspace_root: PathBuf,
    /// Maximum time to wait for the agent's ready signal (first stdout line).
    ///
    /// If no line is received within this window the spawner kills the
    /// process and returns `AppError::Acp("startup timeout …")`.
    pub startup_timeout: Duration,
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

/// Spawn an ACP agent process and wait for its ready signal.
///
/// The spawner:
/// 1. Validates that `session_id` is non-empty.
/// 2. Builds a `tokio::process::Command` with `env_clear()` and only the
///    variables listed in [`ALLOWED_ENV_VARS`].
/// 3. Passes `INTERCOM_SESSION_ID` as an explicit environment variable.
/// 4. Waits up to `config.startup_timeout` for the first line of stdout
///    (the agent's ready signal).
/// 5. On timeout: kills the process and returns `AppError::Acp`.
///
/// The initial prompt is **not** passed as a CLI argument. Instead, the caller
/// must send it via `handshake::send_prompt` after the `initialize` /
/// `initialized` exchange completes (FR-030).
///
/// # Errors
///
/// - `AppError::Acp("failed to spawn agent: …")` — OS spawn failure.
/// - `AppError::Acp("startup timeout …")` — no ready line within the window.
/// - `AppError::Acp("agent process exited before ready signal")` — early EOF.
pub async fn spawn_agent(config: &SpawnConfig, session_id: &str) -> Result<AcpConnection> {
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

    let mut reader = BufReader::new(stdout_raw);
    let mut line = String::new();

    match tokio::time::timeout(config.startup_timeout, reader.read_line(&mut line)).await {
        Ok(Ok(n)) if n > 0 => {
            info!(
                session_id,
                ready_line = line.trim(),
                "agent emitted ready signal"
            );
        }
        Ok(Ok(_)) => {
            // n == 0 means EOF — process exited before sending anything.
            return Err(AppError::Acp(
                "agent process exited before ready signal".into(),
            ));
        }
        Ok(Err(err)) => {
            return Err(AppError::Acp(format!(
                "failed to read agent ready signal: {err}"
            )));
        }
        Err(_elapsed) => {
            // Kill the process before returning the error.
            child.kill().await.ok();
            return Err(AppError::Acp(format!(
                "startup timeout: agent did not emit ready signal within {:?}",
                config.startup_timeout
            )));
        }
    }

    Ok(AcpConnection {
        session_id: session_id.to_owned(),
        child,
        stdin,
        stdout: reader,
    })
}

// ── Exit monitor ─────────────────────────────────────────────────────────────

/// Spawn a background task that awaits child-process exit and emits
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
