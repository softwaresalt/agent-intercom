//! ACP initialization handshake.
//!
//! After an agent process is spawned and emits its ready signal, the server
//! performs an LSP-style initialize/initialized exchange before entering the
//! main stream loop:
//!
//! 1. **`send_initialize`** — writes an `initialize` request to the agent's
//!    stdin containing the server's `processId`, `clientInfo`, and the list
//!    of `workspaceFolders`.
//! 2. **`wait_for_initialized`** — reads lines from the agent's stdout until
//!    an `initialized` response is received (matching the correlation ID) or
//!    the timeout elapses.
//! 3. **`send_prompt`** — writes a `prompt/send` message to stdin so the
//!    initial user prompt reaches the agent via the stream protocol rather
//!    than as a CLI argument (FR-030).
//!
//! All functions operate on raw `ChildStdin` / `BufReader<ChildStdout>`
//! handles obtained from the spawner.  The caller must not start the
//! `run_reader` / `run_writer` tasks until the handshake completes
//! successfully.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tracing::{debug, info};

use crate::{AppError, Result};

/// Correlation ID used for the initialize / initialized exchange.
const INIT_ID: &str = "intercom-init-1";

/// Send an `initialize` request to the agent over its stdin.
///
/// The message follows the LSP-style initialize protocol:
/// ```json
/// {
///   "method": "initialize",
///   "id": "intercom-init-1",
///   "params": {
///     "processId": 12345,
///     "clientInfo": { "name": "agent-intercom-acp", "version": "0.1.0" },
///     "workspaceFolders": [{ "uri": "file:///…", "name": "…" }]
///   }
/// }
/// ```
///
/// # Errors
///
/// Returns `AppError::Acp` if serialisation fails or the write to stdin fails.
pub async fn send_initialize(
    stdin: &mut ChildStdin,
    session_id: &str,
    workspace_path: &Path,
    workspace_name: &str,
) -> Result<()> {
    let process_id = std::process::id();

    // Convert the workspace path to a `file://` URI.
    let uri = path_to_file_uri(workspace_path);

    let msg: Value = json!({
        "method": "initialize",
        "id": INIT_ID,
        "params": {
            "processId": process_id,
            "clientInfo": {
                "name": "agent-intercom-acp",
                "version": env!("CARGO_PKG_VERSION")
            },
            "workspaceFolders": [
                { "uri": uri, "name": workspace_name }
            ]
        }
    });

    write_json_line(stdin, &msg).await.map_err(|e| {
        AppError::Acp(format!(
            "failed to send initialize to session {session_id}: {e}"
        ))
    })?;

    debug!(session_id, %uri, "handshake: initialize sent");
    Ok(())
}

/// Wait for the agent to respond with an `initialized` message.
///
/// Reads lines from the agent's stdout one at a time.  Each line is parsed
/// as JSON; unrecognised messages are logged at `DEBUG` and skipped.  The
/// function returns as soon as a message with `"method": "initialized"` and
/// a matching `id` field is seen, or when `timeout` elapses.
///
/// # Errors
///
/// - `AppError::Acp("handshake timeout …")` — no `initialized` message
///   received within `timeout`.
/// - `AppError::Acp("handshake io error: …")` — underlying I/O failure.
/// - `AppError::Acp("agent exited during handshake")` — EOF before `initialized`.
pub async fn wait_for_initialized(
    stdout: &mut BufReader<ChildStdout>,
    session_id: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::Acp(format!(
                "handshake timeout: 'initialized' not received within {timeout:?} for session \
                 {session_id}"
            )));
        }

        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());

        let mut line = String::new();
        let n = tokio::time::timeout(remaining, stdout.read_line(&mut line))
            .await
            .map_err(|_| {
                AppError::Acp(format!(
                    "handshake timeout: 'initialized' not received within {timeout:?} for session \
                     {session_id}"
                ))
            })?
            .map_err(|e| AppError::Acp(format!("handshake io error: {e}")))?;

        if n == 0 {
            return Err(AppError::Acp(format!(
                "agent exited during handshake for session {session_id}"
            )));
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<Value>(trimmed) {
            Ok(v) => {
                let method = v.get("method").and_then(Value::as_str).unwrap_or("");
                let id = v.get("id").and_then(Value::as_str).unwrap_or("");

                if method == "initialized" && (id == INIT_ID || id.is_empty()) {
                    info!(session_id, "handshake: 'initialized' received");
                    return Ok(());
                }

                debug!(
                    session_id,
                    method, "handshake: skipping unexpected message before 'initialized'"
                );
            }
            Err(e) => {
                debug!(session_id, error = %e, raw = trimmed, "handshake: non-JSON line, skipping");
            }
        }
    }
}

/// Send the initial user prompt to the agent via `prompt/send`.
///
/// This must be called after the `initialized` handshake so that the agent
/// is in the correct state to receive work. Sending the prompt via the stream
/// protocol rather than as a CLI argument satisfies FR-030.
///
/// # Errors
///
/// - `AppError::Acp("prompt must not be empty")` — `prompt` is blank.
/// - `AppError::Acp("failed to send prompt …")` — write to stdin fails.
pub async fn send_prompt(stdin: &mut ChildStdin, session_id: &str, prompt: &str) -> Result<()> {
    if prompt.trim().is_empty() {
        return Err(AppError::Acp("prompt must not be empty".into()));
    }

    let msg: Value = json!({
        "method": "prompt/send",
        "params": { "text": prompt }
    });

    write_json_line(stdin, &msg).await.map_err(|e| {
        AppError::Acp(format!(
            "failed to send prompt to session {session_id}: {e}"
        ))
    })?;

    debug!(session_id, "handshake: initial prompt sent via prompt/send");
    Ok(())
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Serialise `value` to a compact JSON string, append `\n`, and write to `stdin`.
async fn write_json_line(stdin: &mut ChildStdin, value: &Value) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(value).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("json serialisation failed: {e}"),
        )
    })?;
    bytes.push(b'\n');
    stdin.write_all(&bytes).await
}

/// Convert a filesystem path to a `file://` URI string.
///
/// On Windows, backslash separators are converted to forward slashes and the
/// drive letter is preserved: `C:\foo\bar` → `file:///C:/foo/bar`.
fn path_to_file_uri(path: &Path) -> String {
    let s = path.to_string_lossy();
    // Normalise Windows backslashes to forward slashes.
    let forward = s.replace('\\', "/");
    if forward.starts_with('/') {
        format!("file://{forward}")
    } else {
        // Windows drive letter: C:/… → file:///C:/…
        format!("file:///{forward}")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::path_to_file_uri;
    use std::path::Path;

    #[test]
    fn unix_path_to_file_uri() {
        let uri = path_to_file_uri(Path::new("/home/user/project"));
        assert_eq!(uri, "file:///home/user/project");
    }

    #[test]
    fn windows_path_to_file_uri() {
        let uri = path_to_file_uri(Path::new(r"D:\Source\GitHub\agent-intercom"));
        assert_eq!(uri, "file:///D:/Source/GitHub/agent-intercom");
    }
}
