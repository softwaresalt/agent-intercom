//! ACP writer task.
//!
//! Receives outbound JSON messages from a tokio [`mpsc`] channel, stamps each
//! with a monotonically increasing `seq` field (ES-008, FR-040), serialises the
//! message to a single-line JSON string, and writes the NDJSON line to the
//! agent's `stdin` using [`tokio::io::AsyncWriteExt`].
//!
//! Each serialised message is terminated by a `\n` byte, producing valid
//! newline-delimited JSON (NDJSON) as required by the ACP wire format.
//!
//! On write failure (e.g., broken pipe / agent crash), the task logs `WARN`
//! with the `method`, `session_id`, and `seq` fields, marks the session as
//! `Interrupted` in the database, and returns an error (ES-008, FR-041).

use std::sync::{atomic::AtomicU64, Arc};

use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::models::session::SessionStatus;
use crate::persistence::session_repo::SessionRepo;
use crate::{AppError, Result};

/// ACP writer task — stamps, serialises, and writes outbound JSON messages.
///
/// Receives [`serde_json::Value`] objects from `msg_rx`, stamps each with the
/// next value of `counter` as a `"seq"` field, serialises the result to
/// compact single-line JSON, appends `\n`, and writes the bytes to `stdin`.
///
/// The task exits cleanly when:
/// - `cancel` is triggered (graceful shutdown), or
/// - `msg_rx` is closed (all senders dropped).
///
/// On write failure the task:
/// 1. Logs `WARN` with `session_id`, `method`, and `seq`.
/// 2. Calls [`SessionRepo::set_terminated`] to mark the session `Interrupted`.
/// 3. Returns [`AppError::Acp`]`("write failed: …")`.
///
/// # Type parameter
///
/// `W` — any [`tokio::io::AsyncWrite`] + [`Unpin`] + [`Send`] type.  In
/// production this is [`tokio::process::ChildStdin`]; in tests it can be a
/// [`tokio::io::DuplexStream`] or similar.
///
/// # Errors
///
/// - [`AppError::Acp`]`("failed to serialise outbound message: …")` if
///   serialisation fails (should not occur for `Value`).
/// - [`AppError::Acp`]`("write failed: …")` if the write to `stdin` fails.
pub async fn run_writer<W>(
    session_id: String,
    stdin: W,
    mut msg_rx: mpsc::Receiver<serde_json::Value>,
    cancel: CancellationToken,
    counter: Arc<AtomicU64>,
    db: Arc<sqlx::SqlitePool>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin + Send,
{
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
                    Some(mut value) => {
                        // Stamp sequence number before serialisation.
                        let seq = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let method = value
                            .get("method")
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown")
                            .to_owned();

                        if let serde_json::Value::Object(ref mut map) = value {
                            map.insert("seq".to_owned(), serde_json::json!(seq));
                        }

                        let mut bytes = serde_json::to_vec(&value).map_err(|e| {
                            AppError::Acp(format!(
                                "failed to serialise outbound message: {e}"
                            ))
                        })?;

                        // NDJSON: append the newline delimiter.
                        bytes.push(b'\n');

                        if let Err(e) = stdin.write_all(&bytes).await {
                            warn!(
                                session_id,
                                method,
                                seq,
                                error = %e,
                                "acp writer: write to stdin failed — marking session interrupted"
                            );
                            // Mark session Interrupted so the orchestrator and
                            // Slack handlers can surface the disconnection.
                            SessionRepo::new(Arc::clone(&db))
                                .set_terminated(
                                    &session_id,
                                    SessionStatus::Interrupted,
                                )
                                .await
                                .ok();
                            return Err(AppError::Acp(format!("write failed: {e}")));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
