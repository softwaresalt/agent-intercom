//! ACP writer task.
//!
//! Receives outbound JSON messages from a tokio [`mpsc`] channel, serialises
//! each value to a single-line JSON string, and writes the NDJSON line to the
//! agent's `stdin` using [`tokio::io::AsyncWriteExt`].
//!
//! Each serialised message is terminated by a `\n` byte, producing valid
//! newline-delimited JSON (NDJSON) as required by the ACP wire format.

use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::{AppError, Result};

/// ACP writer task — serialises outbound JSON messages and writes to `stdin`.
///
/// Receives [`serde_json::Value`] objects from `msg_rx`, serialises each to a
/// compact single-line JSON string, appends `\n`, and writes the resulting
/// bytes to the agent process's `stdin`.
///
/// The task exits cleanly when:
/// - `cancel` is triggered (graceful shutdown), or
/// - `msg_rx` is closed (all senders dropped).
///
/// # Errors
///
/// - [`AppError::Acp`]`("failed to serialise outbound message: …")` if
///   [`serde_json::to_vec`] fails (should not occur for `Value`).
/// - [`AppError::Acp`]`("write failed: …")` if the write to `stdin` fails
///   (e.g. the agent process has exited).
pub async fn run_writer(
    session_id: String,
    stdin: ChildStdin,
    mut msg_rx: mpsc::Receiver<serde_json::Value>,
    cancel: CancellationToken,
) -> Result<()> {
    let mut stdin = stdin;

    loop {
        tokio::select! {
            biased;

            () = cancel.cancelled() => {
                debug!(session_id, "acp writer: cancellation received, stopping");
                break;
            }

            msg = msg_rx.recv() => {
                match msg {
                    None => {
                        debug!(session_id, "acp writer: message channel closed, stopping");
                        break;
                    }
                    Some(value) => {
                        let mut bytes = serde_json::to_vec(&value).map_err(|e| {
                            AppError::Acp(format!(
                                "failed to serialise outbound message: {e}"
                            ))
                        })?;

                        // NDJSON: append the newline delimiter.
                        bytes.push(b'\n');

                        stdin.write_all(&bytes).await.map_err(|e| {
                            warn!(session_id, error = %e, "acp writer: write to stdin failed");
                            AppError::Acp(format!("write failed: {e}"))
                        })?;
                    }
                }
            }
        }
    }

    Ok(())
}
