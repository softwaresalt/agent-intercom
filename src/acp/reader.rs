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
//! # Reconnect flush
//!
//! When an optional [`ReconnectFlushContext`] is supplied, `run_reader` sets
//! the session's connectivity status to `Online`, delivers any queued steering
//! messages via the ACP driver, and optionally posts a Slack notification
//! before entering the stream loop.  This ensures that operator messages sent
//! while the agent was `Offline` or `Stalled` are not lost.
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

use std::sync::Arc;

use futures_util::StreamExt;
use serde::Deserialize;
use slack_morphism::prelude::{SlackChannelId, SlackTs};
use tokio::io::AsyncRead;
use tokio::sync::mpsc;
use tokio_util::codec::FramedRead;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::acp::codec::AcpCodec;
use crate::driver::{AgentDriver, AgentEvent};
use crate::models::progress::ProgressItem;
use crate::models::session::ConnectivityStatus;
use crate::models::steering::SteeringMessage;
use crate::persistence::db::Database;
use crate::persistence::session_repo::SessionRepo;
use crate::persistence::steering_repo::SteeringRepo;
use crate::slack::client::{SlackMessage, SlackService};
use crate::{AppError, Result};

// ── Rate limiting (T143–T145, FR-044) ────────────────────────────────────────

/// Rate-limit decision for a single inbound ACP message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitDecision {
    /// Message is within the allowed rate; process normally.
    Allow,
    /// Rate limit exceeded; log a warning and skip this message.
    Throttle,
    /// Sustained flood detected; terminate the session.
    Terminate,
}

/// Token-bucket rate limiter for inbound ACP messages (FR-044).
///
/// Refills `max_rate` tokens per second; tokens cannot exceed `max_rate`.
/// When the bucket is empty:
/// - `Throttle` is returned and `consecutive_throttle` is incremented.
/// - After `TERMINATE_THRESHOLD` consecutive throttles, `Terminate` is returned.
///
/// One token is consumed per call to [`check`].
pub struct TokenBucketRateLimiter {
    max_rate: f64,
    tokens: f64,
    last_refill: std::time::Instant,
    consecutive_throttle: u32,
}

/// Number of consecutive throttle events that trigger `Terminate`.
const TERMINATE_THRESHOLD: u32 = 50;

impl TokenBucketRateLimiter {
    /// Create a new limiter with the given maximum rate in messages per second.
    #[must_use]
    pub fn new(max_rate: u32) -> Self {
        let rate = f64::from(max_rate);
        Self {
            max_rate: rate,
            tokens: rate,
            last_refill: std::time::Instant::now(),
            consecutive_throttle: 0,
        }
    }

    /// Consume one token and return the rate-limit decision.
    ///
    /// Refills the bucket proportionally to elapsed wall-clock time before
    /// deciding whether to allow or throttle the message.
    pub fn check(&mut self) -> RateLimitDecision {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;

        // Refill tokens proportional to elapsed time, capped at max_rate.
        self.tokens = (self.tokens + elapsed * self.max_rate).min(self.max_rate);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            self.consecutive_throttle = 0;
            RateLimitDecision::Allow
        } else {
            self.consecutive_throttle += 1;
            if self.consecutive_throttle >= TERMINATE_THRESHOLD {
                RateLimitDecision::Terminate
            } else {
                RateLimitDecision::Throttle
            }
        }
    }
}

// ── Inbound message types ─────────────────────────────────────────────────────

/// Top-level ACP message envelope (agent → server).
#[derive(Debug, Deserialize)]
struct AcpEnvelope {
    /// Message type identifier (e.g., `clearance/request`).  Optional because
    /// JSON-RPC result messages carry only `id` + `result`/`error`.
    method: Option<String>,
    /// Optional correlation ID; required for request/response pairs.
    id: Option<String>,
    /// Method-specific payload.  Defaults to `null` for result messages.
    #[serde(default)]
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

/// Parameters for the ACP `session/update` method (streaming content).
///
/// Uses `serde_json::Value` for the `update` field because the shape varies
/// significantly by `sessionUpdate` type (e.g., `agent_message_chunk` has a
/// single `content` object while `tool_call_update` has a `content` array
/// plus `rawOutput`).
#[derive(Debug, Deserialize)]
struct SessionUpdateParams {
    update: serde_json::Value,
}

// ── Reconnect flush context ───────────────────────────────────────────────────

/// Context supplied to [`run_reader`] for flushing queued messages on reconnect.
///
/// When provided, the reader sets the session's connectivity status to `Online`
/// before entering the stream loop, delivers all unconsumed steering messages
/// via the ACP driver in FIFO order, and optionally notifies the operator in
/// Slack that the agent is back online.
pub struct ReconnectFlushContext {
    /// Database handle for session status updates and steering queue access.
    pub db: Arc<Database>,
    /// ACP driver for delivering queued steering messages to the agent.
    pub driver: Arc<dyn AgentDriver>,
    /// Optional Slack service for posting the "back online" notification.
    pub slack: Option<Arc<SlackService>>,
    /// Slack channel ID for the notification (required when `slack` is `Some`).
    pub channel_id: Option<String>,
    /// Thread timestamp for posting the notification as a reply.
    pub thread_ts: Option<String>,
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

    // JSON-RPC result/error messages have no `method` — skip them gracefully.
    let Some(ref method_str) = envelope.method else {
        debug!(session_id, "acp reader: skipping JSON-RPC result message");
        return Ok(None);
    };

    match method_str.as_str() {
        "clearance/request" => parse_clearance_request(session_id, envelope),
        "status/update" => parse_status_update(session_id, envelope),
        "prompt/forward" => parse_prompt_forward(session_id, envelope),
        "heartbeat" => parse_heartbeat(session_id, envelope),
        // ACP `session/update` — streaming content from the agent during prompt
        // execution.  Treated as a status update so the operator sees progress.
        "session/update" => parse_session_update(session_id, envelope),
        // Handshake response from the agent — silently accepted. The
        // `initialized` message is consumed by `handshake::wait_for_initialize_result`
        // before `run_reader` starts; if one slips through, skip it gracefully.
        "initialized" => Ok(None),
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
/// When `flush_ctx` is `Some`, the reader first sets the session's connectivity
/// status to `Online`, flushes any queued steering messages via the ACP driver
/// (in FIFO order), and optionally posts a Slack notification.
///
/// After each successfully parsed line, emits [`AgentEvent::StreamActivity`] so
/// the stall consumer can reset the inactivity timer (S063).
///
/// On clean EOF, sends [`AgentEvent::SessionTerminated`] with
/// `reason: "stream closed"` before returning.
///
/// Malformed or unrecognised lines are logged and skipped — they do **not**
/// terminate the reader task.
///
/// The `max_msg_rate` parameter (FR-044) sets the token-bucket refill rate in
/// messages per second.  Pass `0` to disable rate limiting.
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
    flush_ctx: Option<ReconnectFlushContext>,
    max_msg_rate: u32,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send,
{
    // ── Reconnect flush (T089/T090) ───────────────────────────────────────────
    if let Some(ctx) = flush_ctx {
        flush_queued_messages(&session_id, &ctx, &event_tx).await;
    }

    // ── Rate limiter (T144/T145, FR-044) ─────────────────────────────────────
    let mut rate_limiter = if max_msg_rate > 0 {
        Some(TokenBucketRateLimiter::new(max_msg_rate))
    } else {
        None
    };

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
                        // ── Rate limit check (FR-044) ─────────────────────
                        if let Some(ref mut limiter) = rate_limiter {
                            match limiter.check() {
                                RateLimitDecision::Allow => {}
                                RateLimitDecision::Throttle => {
                                    warn!(
                                        session_id,
                                        "acp reader: rate limit exceeded, dropping message"
                                    );
                                    continue;
                                }
                                RateLimitDecision::Terminate => {
                                    warn!(
                                        session_id,
                                        "acp reader: sustained message flood, terminating session"
                                    );
                                    send_terminated(
                                        &event_tx,
                                        &session_id,
                                        "rate limit: sustained message flood",
                                    )
                                    .await;
                                    break;
                                }
                            }
                        }

                        match parse_inbound_line(&session_id, &line) {
                            Ok(Some(event)) => {
                                // Emit StreamActivity before the main event so the
                                // stall consumer resets the timer regardless of
                                // whether the receiver is still listening (S063).
                                let activity = AgentEvent::StreamActivity {
                                    session_id: session_id.clone(),
                                };
                                let _ = event_tx.send(activity).await;

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

/// Deliver queued steering messages to the agent via the driver.
///
/// For each message in `messages`:
/// - Calls `driver.send_prompt` to deliver the message.
/// - On success, marks the message as consumed in `steering_repo`.
/// - On error, logs a warning and continues to the next message so that
///   temporary delivery failures do not block subsequent messages.
///
/// This function is the core delivery loop for the reconnect flush
/// (T089/T090 — S001–S004, S006–S007, F-06).
///
/// # Note
///
/// This function is intentionally `pub` to allow direct unit testing of the
/// delivery loop in isolation without exercising the full reader pipeline
/// (T001–T006 in `tests/unit/acp_reader_steering_delivery.rs`).
pub async fn deliver_queued_messages(
    session_id: &str,
    messages: &[SteeringMessage],
    driver: &dyn AgentDriver,
    steering_repo: &SteeringRepo,
) -> usize {
    let mut delivered: usize = 0;
    for msg in messages {
        match driver.send_prompt(session_id, &msg.message).await {
            Ok(()) => {
                // Only mark consumed when delivery succeeds (F-06 fix).
                if let Err(err) = steering_repo.mark_consumed(&msg.id).await {
                    warn!(
                        session_id,
                        %err,
                        message_id = %msg.id,
                        "acp reader: failed to mark queued message consumed"
                    );
                }
                delivered += 1;
            }
            Err(err) => {
                warn!(
                    session_id,
                    %err,
                    message_id = %msg.id,
                    "acp reader: failed to deliver queued message, continuing"
                );
            }
        }
    }
    delivered
}

/// Set connectivity to Online, deliver queued messages, and notify Slack.
///
/// This is the reconnect flush logic (T089/T090 — S059, S060, S062).
async fn flush_queued_messages(
    session_id: &str,
    ctx: &ReconnectFlushContext,
    event_tx: &mpsc::Sender<AgentEvent>,
) {
    let session_repo = SessionRepo::new(Arc::clone(&ctx.db));
    let steering_repo = SteeringRepo::new(Arc::clone(&ctx.db));

    // Mark the session as Online so future steering messages are delivered
    // directly instead of being queued.
    if let Err(err) = session_repo
        .set_connectivity_status(session_id, ConnectivityStatus::Online)
        .await
    {
        warn!(session_id, %err, "acp reader: failed to set connectivity Online");
    }

    // Fetch unconsumed steering messages in FIFO order.
    let queued = match steering_repo.fetch_unconsumed(session_id).await {
        Ok(msgs) => msgs,
        Err(err) => {
            warn!(session_id, %err, "acp reader: failed to fetch queued messages");
            return;
        }
    };

    if queued.is_empty() {
        return;
    }

    let count = queued.len();
    info!(
        session_id,
        count, "acp reader: delivering queued messages on reconnect"
    );

    // Deliver each queued message via the driver (F-06 fix: only marks consumed
    // on success — see `deliver_queued_messages`).
    let delivered =
        deliver_queued_messages(session_id, &queued, ctx.driver.as_ref(), &steering_repo).await;

    // Emit StreamActivity only for successfully delivered messages so the stall
    // detector knows the session is active during the flush (LC-05).
    for _ in 0..delivered {
        let _ = event_tx
            .send(AgentEvent::StreamActivity {
                session_id: session_id.to_owned(),
            })
            .await;
    }

    // Post "back online" notification to the Slack thread.
    if let (Some(ref slack), Some(ref channel_id)) = (&ctx.slack, &ctx.channel_id) {
        let thread_ts = ctx.thread_ts.as_deref().map(|s| SlackTs(s.to_owned()));
        let text = format!(
            "\u{1f7e2} Agent back online \u{2014} delivering {count} queued \
             message{s}",
            s = if count == 1 { "" } else { "s" }
        );
        let slack_msg = SlackMessage {
            channel: SlackChannelId(channel_id.clone()),
            text: Some(text),
            blocks: None,
            thread_ts,
        };
        if let Err(err) = slack.enqueue(slack_msg).await {
            warn!(session_id, %err, "acp reader: failed to post back-online notification");
        }
    }
}

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

/// Parse an ACP `session/update` envelope into [`AgentEvent::StatusUpdated`].
///
/// The `session/update` notification is the primary streaming mechanism in the
/// ACP protocol.  Text chunks from the agent are surfaced as status updates so
/// the operator can follow progress in the Slack thread.
///
/// Only `agent_message_chunk` updates with non-empty text content are emitted
/// as events.  All other update types (`tool_call`, `tool_call_update`,
/// `agent_thought_chunk`, etc.) are silently skipped.
fn parse_session_update(session_id: &str, env: AcpEnvelope) -> Result<Option<AgentEvent>> {
    let params: SessionUpdateParams = serde_json::from_value(env.params).map_err(|e| {
        AppError::Acp(format!(
            "missing required field: session/update params: {e}"
        ))
    })?;

    let update = &params.update;
    let update_type = update
        .get("sessionUpdate")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    // Extract text content from agent_message_chunk updates.
    if update_type == "agent_message_chunk" {
        if let Some(text) = update
            .get("content")
            .and_then(|c| c.get("text"))
            .and_then(serde_json::Value::as_str)
        {
            if !text.is_empty() {
                return Ok(Some(AgentEvent::StatusUpdated {
                    session_id: session_id.to_owned(),
                    message: text.to_owned(),
                }));
            }
        }
    }

    debug!(
        session_id,
        update_type, "acp reader: skipping non-text session/update"
    );
    Ok(None)
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
