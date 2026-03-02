//! ACP initialization handshake.
//!
//! Implements the Agent Client Protocol (ACP) JSON-RPC 2.0 handshake
//! for spawned agent processes (e.g., `copilot --acp`).
//!
//! The full startup sequence is:
//!
//! 1. **`send_initialize`** — writes an ACP `initialize` JSON-RPC request
//!    containing `protocolVersion`, `processId`, `clientInfo`, and
//!    `workspaceFolders`.
//! 2. **`wait_for_initialize_result`** — reads lines from stdout until the
//!    JSON-RPC result for the `initialize` request arrives (matching the
//!    correlation ID) or the timeout elapses.
//! 3. **`send_initialized`** — sends the `initialized` JSON-RPC notification
//!    to signal the client is ready.
//! 4. **`send_session_new`** — creates an ACP session via `session/new` and
//!    returns the agent-assigned `sessionId`.
//! 5. **`send_prompt`** — writes a `session/prompt` message so the initial
//!    user prompt reaches the agent (FR-030).
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
use tracing::{debug, info, warn};

use crate::{AppError, Result};

/// ACP protocol version supported by this client.
const PROTOCOL_VERSION: u32 = 1;

/// Correlation ID used for the `initialize` request.
const INIT_ID: &str = "intercom-init-1";

/// Correlation ID used for the `session/new` request.
const SESSION_NEW_ID: &str = "intercom-sess-1";

/// Correlation ID used for the `session/prompt` request.
const PROMPT_ID: &str = "intercom-prompt-1";

/// Send an ACP `initialize` JSON-RPC request to the agent over its stdin.
///
/// The message uses the ACP protocol format:
/// ```json
/// {
///   "jsonrpc": "2.0",
///   "method": "initialize",
///   "id": "intercom-init-1",
///   "params": {
///     "protocolVersion": 1,
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
    let uri = path_to_file_uri(workspace_path);

    let msg: Value = json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": INIT_ID,
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
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

/// Wait for the agent to respond with a JSON-RPC result for the `initialize`
/// request.
///
/// Reads lines from the agent's stdout one at a time.  Each line is parsed
/// as JSON; the function returns as soon as a JSON-RPC response with a
/// matching `id` and a `result` field is seen, or when `timeout` elapses.
/// JSON-RPC error responses with a matching `id` are treated as handshake
/// failures.
///
/// # Errors
///
/// - `AppError::Acp("handshake timeout …")` — no result received within
///   `timeout`.
/// - `AppError::Acp("handshake rejected: …")` — agent returned a JSON-RPC
///   error for the `initialize` request.
/// - `AppError::Acp("handshake io error: …")` — underlying I/O failure.
/// - `AppError::Acp("agent exited during handshake")` — EOF before response.
pub async fn wait_for_initialize_result(
    stdout: &mut BufReader<ChildStdout>,
    session_id: &str,
    timeout: Duration,
) -> Result<Value> {
    wait_for_result(stdout, session_id, INIT_ID, "initialize", timeout).await
}

/// Send the `initialized` JSON-RPC notification to the agent.
///
/// This notification signals that the client has processed the `initialize`
/// result and is ready to proceed with session creation.
///
/// # Errors
///
/// Returns `AppError::Acp` if the write to stdin fails.
pub async fn send_initialized(stdin: &mut ChildStdin, session_id: &str) -> Result<()> {
    let msg: Value = json!({
        "jsonrpc": "2.0",
        "method": "initialized"
    });

    write_json_line(stdin, &msg).await.map_err(|e| {
        AppError::Acp(format!(
            "failed to send initialized to session {session_id}: {e}"
        ))
    })?;

    debug!(session_id, "handshake: initialized notification sent");
    Ok(())
}

/// Create an ACP session via `session/new` and return the agent-assigned
/// session ID.
///
/// # Errors
///
/// - `AppError::Acp("session/new rejected: …")` — agent returned a JSON-RPC
///   error.
/// - `AppError::Acp("session/new: missing sessionId …")` — result does not
///   contain a `sessionId` field.
/// - Timeout and I/O errors propagated from [`wait_for_result`].
pub async fn send_session_new(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    session_id: &str,
    workspace_path: &Path,
    timeout: Duration,
) -> Result<String> {
    let cwd = workspace_path.to_string_lossy().replace('\\', "/");

    let msg: Value = json!({
        "jsonrpc": "2.0",
        "method": "session/new",
        "id": SESSION_NEW_ID,
        "params": {
            "cwd": cwd,
            "mcpServers": []
        }
    });

    write_json_line(stdin, &msg).await.map_err(|e| {
        AppError::Acp(format!(
            "failed to send session/new to session {session_id}: {e}"
        ))
    })?;

    debug!(session_id, "handshake: session/new sent");

    let result =
        wait_for_result(stdout, session_id, SESSION_NEW_ID, "session/new", timeout).await?;

    let agent_session_id = result
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AppError::Acp(format!(
                "session/new: missing sessionId in result for {session_id}"
            ))
        })?
        .to_owned();

    info!(
        session_id,
        agent_session_id, "handshake: agent session created"
    );

    Ok(agent_session_id)
}

/// Send the initial user prompt to the agent via `session/prompt`.
///
/// This must be called after `session/new` so that the agent has an active
/// session to receive the prompt. The prompt is delivered via the ACP stream
/// protocol rather than as a CLI argument (FR-030).
///
/// # Errors
///
/// - `AppError::Acp("prompt must not be empty")` — `prompt` is blank.
/// - `AppError::Acp("failed to send prompt …")` — write to stdin fails.
pub async fn send_prompt(
    stdin: &mut ChildStdin,
    session_id: &str,
    agent_session_id: &str,
    prompt: &str,
) -> Result<()> {
    if prompt.trim().is_empty() {
        return Err(AppError::Acp("prompt must not be empty".into()));
    }

    let msg: Value = json!({
        "jsonrpc": "2.0",
        "method": "session/prompt",
        "id": PROMPT_ID,
        "params": {
            "sessionId": agent_session_id,
            "prompt": [
                { "type": "text", "text": prompt }
            ]
        }
    });

    write_json_line(stdin, &msg).await.map_err(|e| {
        AppError::Acp(format!(
            "failed to send prompt to session {session_id}: {e}"
        ))
    })?;

    debug!(
        session_id,
        agent_session_id, "handshake: session/prompt sent"
    );
    Ok(())
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Wait for a JSON-RPC result matching `expected_id`.
///
/// Reads NDJSON lines from `stdout` until a JSON-RPC response with `id` equal
/// to `expected_id` is found.  Returns the `result` value on success.
///
/// # Errors
///
/// - Timeout if no matching response arrives within `timeout`.
/// - `AppError::Acp` if the agent returns a JSON-RPC error object.
/// - `AppError::Acp` on EOF or I/O failure.
async fn wait_for_result(
    stdout: &mut BufReader<ChildStdout>,
    session_id: &str,
    expected_id: &str,
    method_name: &str,
    timeout: Duration,
) -> Result<Value> {
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::Acp(format!(
                "handshake timeout: '{method_name}' result not received within {timeout:?} \
                 for session {session_id}"
            )));
        }

        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());

        let mut line = String::new();
        let n = tokio::time::timeout(remaining, stdout.read_line(&mut line))
            .await
            .map_err(|_| {
                AppError::Acp(format!(
                    "handshake timeout: '{method_name}' result not received within {timeout:?} \
                     for session {session_id}"
                ))
            })?
            .map_err(|e| AppError::Acp(format!("handshake io error: {e}")))?;

        if n == 0 {
            return Err(AppError::Acp(format!(
                "agent exited during {method_name} handshake for session {session_id}"
            )));
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<Value>(trimmed) {
            Ok(v) => {
                // Match by id — ACP responses carry the request's correlation id.
                let id = v.get("id").and_then(Value::as_str).unwrap_or("");

                if id == expected_id {
                    // Check for JSON-RPC error response.
                    if let Some(err) = v.get("error") {
                        let msg = err
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown error");
                        return Err(AppError::Acp(format!("{method_name} rejected: {msg}")));
                    }

                    if let Some(result) = v.get("result") {
                        info!(session_id, method_name, "handshake: result received");
                        return Ok(result.clone());
                    }

                    // Response with matching id but neither result nor error.
                    warn!(
                        session_id,
                        method_name, "handshake: response has matching id but no result or error"
                    );
                }

                // Not the response we're looking for — skip notifications and
                // unrelated messages.
                let method = v.get("method").and_then(Value::as_str).unwrap_or("");
                debug!(
                    session_id,
                    id, method, "handshake: skipping message while waiting for {method_name}"
                );
            }
            Err(e) => {
                debug!(
                    session_id,
                    error = %e,
                    raw = trimmed,
                    "handshake: non-JSON line, skipping"
                );
            }
        }
    }
}

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
