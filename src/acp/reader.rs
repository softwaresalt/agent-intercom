//! ACP reader task.
//!
//! Reads newline-delimited JSON messages from an agent's stdout, parses each
//! line into an [`AgentEvent`], and forwards the events through a tokio
//! [`mpsc`] channel.
//!
//! The reader is driven by [`FramedRead`] backed by [`AcpCodec`], which
//! enforces the 1 MiB per-line limit before any heap allocation for JSON
//! parsing.
//!
//! # Known inbound methods
//!
//! | Method             | Maps to                                        |
//! |--------------------|------------------------------------------------|
//! | `clearance/request`| [`AgentEvent::ClearanceRequested`]             |
//! | `status/update`    | [`AgentEvent::StatusUpdated`]                  |
//! | `prompt/forward`   | [`AgentEvent::PromptForwarded`]                |
//! | `heartbeat`        | [`AgentEvent::HeartbeatReceived`]              |
//! | *(any other)*      | Skipped; logged at `DEBUG`                     |

use futures_util::StreamExt;
use serde::Deserialize;
use tokio::io::AsyncRead;
use tokio::sync::mpsc;
use tokio_util::codec::FramedRead;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::acp::codec::AcpCodec;
use crate::driver::AgentEvent;
use crate::models::progress::ProgressItem;
use crate::{AppError, Result};

// ── Inbound message types ─────────────────────────────────────────────────────

/// Top-level ACP message envelope (agent → server).
#[derive(Debug, Deserialize)]
struct AcpEnvelope {
    /// Message type identifier (e.g., `clearance/request`).
    method: String,
    /// Optional correlation ID; required for request/response pairs.
    id: Option<String>,
    /// Method-specific payload.
    params: serde_json::Value,
}

/// Parameters for the `clearance/request` method.
#[derive(Debug, Deserialize)]
struct ClearanceParams {
    title: String,
    description: Option<String>,
    diff: Option<String>,
    file_path: String,
    risk_level: String,
}

/// Parameters for the `status/update` method.
#[derive(Debug, Deserialize)]
struct StatusParams {
    message: String,
}

/// Parameters for the `prompt/forward` method.
#[derive(Debug, Deserialize)]
struct PromptForwardParams {
    text: String,
    /// Original field name in the wire format is `type`.
    #[serde(rename = "type")]
    prompt_type: String,
}

/// Parameters for the `heartbeat` method.
#[derive(Debug, Deserialize)]
struct HeartbeatParams {
    progress: Option<Vec<ProgressItem>>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse a single NDJSON line from an ACP agent stream into an [`AgentEvent`].
///
/// # Return value
///
/// - `Ok(Some(event))` — the line is a recognized, complete message.
/// - `Ok(None)` — the line is empty/whitespace or has an unknown `method`
///   (silently skipped; unknown methods are logged at `DEBUG` level).
/// - `Err(AppError::Acp(...))` — the line is not valid JSON, or a known
///   method has a missing required field.
///
/// # Errors
///
/// - [`AppError::Acp`]`("malformed json: …")` — not valid JSON.
/// - [`AppError::Acp`]`("missing required field: …")` — recognized method but
///   a required parameter field is absent.
pub fn parse_inbound_line(session_id: &str, line: &str) -> Result<Option<AgentEvent>> {
    if line.trim().is_empty() {
        return Ok(None);
    }

    let envelope: AcpEnvelope =
        serde_json::from_str(line).map_err(|e| AppError::Acp(format!("malformed json: {e}")))?;

    match envelope.method.as_str() {
        "clearance/request" => parse_clearance_request(session_id, envelope),
        "status/update" => parse_status_update(session_id, envelope),
        "prompt/forward" => parse_prompt_forward(session_id, envelope),
        "heartbeat" => parse_heartbeat(session_id, envelope),
        other => {
            debug!(
                method = other,
                session_id, "acp reader: skipping unknown inbound method"
            );
            Ok(None)
        }
    }
}

/// ACP reader task — reads NDJSON lines from `stdout` and emits [`AgentEvent`]s.
///
/// Drives a [`FramedRead`] over `stdout` using [`AcpCodec`] (1 MiB line limit).
/// Each decoded line is forwarded to [`parse_inbound_line`]; any resulting
/// [`AgentEvent`] is sent through `event_tx`.
///
/// On clean EOF, sends [`AgentEvent::SessionTerminated`] with
/// `reason: "stream closed"` before returning.
///
/// Malformed or unrecognised lines are logged and skipped — they do **not**
/// terminate the reader task.
///
/// # Cancellation
///
/// Respects `cancel`: when the token fires the reader exits cleanly without
/// emitting a termination event.
///
/// # Errors
///
/// Returns `Ok(())` on clean EOF or cancellation.  Unrecoverable I/O errors
/// (mapped via [`AcpCodec`]) emit `SessionTerminated` and return `Ok(())`.
pub async fn run_reader<R>(
    session_id: String,
    stdout: R,
    event_tx: mpsc::Sender<AgentEvent>,
    cancel: CancellationToken,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send,
{
    let mut framed = FramedRead::new(stdout, AcpCodec::new());

    loop {
        tokio::select! {
            biased;

            () = cancel.cancelled() => {
                debug!(session_id, "acp reader: cancellation received, stopping");
                break;
            }

            item = framed.next() => {
                match item {
                    None => {
                        // EOF — agent stdout closed cleanly.
                        debug!(session_id, "acp reader: EOF detected");
                        send_terminated(&event_tx, &session_id, "stream closed").await;
                        break;
                    }

                    Some(Err(AppError::Acp(ref msg))) => {
                        // Codec-level error (e.g. line too long) — log and continue.
                        warn!(
                            session_id,
                            error = msg.as_str(),
                            "acp reader: codec framing error, skipping"
                        );
                    }

                    Some(Err(e)) => {
                        // I/O error on the underlying stream — non-recoverable.
                        warn!(session_id, error = %e, "acp reader: IO error, stopping");
                        send_terminated(
                            &event_tx,
                            &session_id,
                            &format!("stream error: {e}"),
                        )
                        .await;
                        break;
                    }

                    Some(Ok(line)) => {
                        match parse_inbound_line(&session_id, &line) {
                            Ok(Some(event)) => {
                                if event_tx.send(event).await.is_err() {
                                    debug!(
                                        session_id,
                                        "acp reader: event_tx closed, stopping"
                                    );
                                    break;
                                }
                            }
                            Ok(None) => {
                                // Empty line or unknown method — silently skipped.
                            }
                            Err(e) => {
                                warn!(
                                    session_id,
                                    error = %e,
                                    raw_line = %line,
                                    "acp reader: parse error, skipping line"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Parse a `clearance/request` envelope into [`AgentEvent::ClearanceRequested`].
fn parse_clearance_request(session_id: &str, env: AcpEnvelope) -> Result<Option<AgentEvent>> {
    let request_id = env.id.ok_or_else(|| {
        AppError::Acp("missing required field: `id` in clearance/request envelope".into())
    })?;

    let params: ClearanceParams = serde_json::from_value(env.params).map_err(|e| {
        AppError::Acp(format!(
            "missing required field: clearance/request params: {e}"
        ))
    })?;

    Ok(Some(AgentEvent::ClearanceRequested {
        request_id,
        session_id: session_id.to_owned(),
        title: params.title,
        description: params.description.unwrap_or_default(),
        diff: params.diff,
        file_path: params.file_path,
        risk_level: params.risk_level,
    }))
}

/// Parse a `status/update` envelope into [`AgentEvent::StatusUpdated`].
fn parse_status_update(session_id: &str, env: AcpEnvelope) -> Result<Option<AgentEvent>> {
    let params: StatusParams = serde_json::from_value(env.params)
        .map_err(|e| AppError::Acp(format!("missing required field: status/update params: {e}")))?;

    Ok(Some(AgentEvent::StatusUpdated {
        session_id: session_id.to_owned(),
        message: params.message,
    }))
}

/// Parse a `prompt/forward` envelope into [`AgentEvent::PromptForwarded`].
fn parse_prompt_forward(session_id: &str, env: AcpEnvelope) -> Result<Option<AgentEvent>> {
    let prompt_id = env.id.ok_or_else(|| {
        AppError::Acp("missing required field: `id` in prompt/forward envelope".into())
    })?;

    let params: PromptForwardParams = serde_json::from_value(env.params).map_err(|e| {
        AppError::Acp(format!(
            "missing required field: prompt/forward params: {e}"
        ))
    })?;

    Ok(Some(AgentEvent::PromptForwarded {
        session_id: session_id.to_owned(),
        prompt_id,
        prompt_text: params.text,
        prompt_type: params.prompt_type,
    }))
}

/// Parse a `heartbeat` envelope into [`AgentEvent::HeartbeatReceived`].
fn parse_heartbeat(session_id: &str, env: AcpEnvelope) -> Result<Option<AgentEvent>> {
    let params: HeartbeatParams = serde_json::from_value(env.params)
        .map_err(|e| AppError::Acp(format!("missing required field: heartbeat params: {e}")))?;

    Ok(Some(AgentEvent::HeartbeatReceived {
        session_id: session_id.to_owned(),
        progress: params.progress,
    }))
}

/// Send [`AgentEvent::SessionTerminated`] through `event_tx`, logging on failure.
async fn send_terminated(event_tx: &mpsc::Sender<AgentEvent>, session_id: &str, reason: &str) {
    let event = AgentEvent::SessionTerminated {
        session_id: session_id.to_owned(),
        exit_code: None,
        reason: reason.to_owned(),
    };

    if event_tx.send(event).await.is_err() {
        debug!(
            session_id,
            "acp reader: event_tx closed before SessionTerminated could be delivered"
        );
    }
}
