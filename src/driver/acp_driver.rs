//! ACP protocol implementation of [`AgentDriver`].
//!
//! Routes operator responses and new instructions to the correct agent process
//! via per-session [`mpsc`] writer channels. Each connected ACP session
//! registers a sender via [`AcpDriver::register_session`]; the driver methods
//! look up the channel by session or request ID and deliver serialised JSON
//! messages directly to the agent's stdin writer task.
//!
//! # Session lifecycle
//!
//! 1. Agent is spawned → [`AcpDriver::register_session`] registers the writer.
//! 2. Agent emits `clearance/request` → event consumer calls
//!    [`AcpDriver::register_clearance`] so `resolve_clearance` can route back.
//! 3. Agent emits `prompt/forward` → event consumer calls
//!    [`AcpDriver::register_prompt_request`].
//! 4. Agent disconnects → [`AcpDriver::deregister_session`] removes the writer.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, warn};

use crate::driver::AgentDriver;
use crate::{AppError, Result};

// ── Internal state types ──────────────────────────────────────────────────────

/// Tracks which session owns a pending clearance request.
#[derive(Debug, Clone)]
struct PendingClearance {
    session_id: String,
}

/// Tracks which session owns a pending prompt-forward request.
#[derive(Debug, Clone)]
struct PendingPromptAcp {
    session_id: String,
}

/// Shared map type alias for session writer channels.
type WriterMap = Arc<Mutex<HashMap<String, mpsc::Sender<Value>>>>;

// ── AcpDriver ─────────────────────────────────────────────────────────────────

/// ACP protocol driver — routes operator actions to the correct agent stream.
///
/// Maintains three shared maps protected by async mutexes:
///
/// - `stream_writers`: `session_id` → [`mpsc::Sender<Value>`] registered when
///   an ACP session connects.
/// - `pending_clearances`: `request_id` → owning `session_id`, populated by
///   the event consumer on `clearance/request` receipt.
/// - `pending_prompts_acp`: `prompt_id` → owning `session_id`, populated by
///   the event consumer on `prompt/forward` receipt.
///
/// All maps are `Arc<Mutex<…>>` so the driver can be cheaply cloned and shared
/// across Slack handlers, the orchestrator, and IPC handlers.
#[derive(Debug, Clone)]
pub struct AcpDriver {
    /// Per-session writer channels: `session_id` → outbound message sender.
    stream_writers: WriterMap,
    /// Pending clearance requests: `request_id` → owning session metadata.
    pending_clearances: Arc<Mutex<HashMap<String, PendingClearance>>>,
    /// Pending prompt-forward requests: `prompt_id` → owning session metadata.
    pending_prompts_acp: Arc<Mutex<HashMap<String, PendingPromptAcp>>>,
}

impl AcpDriver {
    /// Create a new `AcpDriver` with empty maps.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stream_writers: Arc::new(Mutex::new(HashMap::new())),
            pending_clearances: Arc::new(Mutex::new(HashMap::new())),
            pending_prompts_acp: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new session's outbound writer channel.
    ///
    /// Must be called after the ACP reader/writer tasks are spawned for the
    /// session.  If a session with the same `session_id` is already registered,
    /// the old sender is replaced with the new one.
    pub async fn register_session(&self, session_id: &str, tx: mpsc::Sender<Value>) {
        self.stream_writers
            .lock()
            .await
            .insert(session_id.to_owned(), tx);
        debug!(session_id, "acp driver: session writer registered");
    }

    /// Remove a session's writer channel on disconnection or termination.
    ///
    /// Idempotent — removing an unknown `session_id` is a no-op.
    pub async fn deregister_session(&self, session_id: &str) {
        self.stream_writers.lock().await.remove(session_id);
        debug!(session_id, "acp driver: session writer deregistered");
    }

    /// Register a pending clearance request for response routing.
    ///
    /// Called by the event consumer when [`AgentEvent::ClearanceRequested`] is
    /// received, before posting the Slack interactive approval message.  This
    /// enables [`AgentDriver::resolve_clearance`] to route the operator's
    /// decision back to the correct agent stream by `request_id` alone.
    pub async fn register_clearance(&self, session_id: &str, request_id: &str) {
        self.pending_clearances.lock().await.insert(
            request_id.to_owned(),
            PendingClearance {
                session_id: session_id.to_owned(),
            },
        );
        debug!(
            session_id,
            request_id, "acp driver: clearance request registered"
        );
    }

    /// Register a pending prompt-forward request for response routing.
    ///
    /// Called by the event consumer when [`AgentEvent::PromptForwarded`] is
    /// received, before posting the Slack interactive prompt message.
    pub async fn register_prompt_request(&self, session_id: &str, prompt_id: &str) {
        self.pending_prompts_acp.lock().await.insert(
            prompt_id.to_owned(),
            PendingPromptAcp {
                session_id: session_id.to_owned(),
            },
        );
        debug!(
            session_id,
            prompt_id, "acp driver: prompt request registered"
        );
    }
}

impl Default for AcpDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ── AgentDriver implementation ─────────────────────────────────────────────────

impl AgentDriver for AcpDriver {
    /// Resolve a pending clearance request by writing a `clearance/response`
    /// to the agent's ACP stream.
    ///
    /// Looks up the owning session via `request_id` from `pending_clearances`,
    /// then serialises the operator's decision and writes it to the session's
    /// writer channel.
    ///
    /// # Errors
    ///
    /// - [`AppError::NotFound`] if `request_id` has no registered pending entry.
    /// - [`AppError::Acp`] if the writer channel is closed (agent disconnected).
    fn resolve_clearance(
        &self,
        request_id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let request_id = request_id.to_owned();
        Box::pin(async move {
            let session_id = {
                let mut pending = self.pending_clearances.lock().await;
                pending.remove(&request_id).map(|e| e.session_id)
            };

            let Some(session_id) = session_id else {
                return Err(AppError::NotFound(format!(
                    "no pending ACP clearance for request_id '{request_id}'"
                )));
            };

            let msg = json!({
                "method": "clearance/response",
                "id": request_id,
                "params": {
                    "status": if approved { "approved" } else { "rejected" },
                    "reason": reason,
                }
            });

            send_to_session(&self.stream_writers, &session_id, msg).await
        })
    }

    /// Send a new prompt or instruction to the agent's ACP stream.
    ///
    /// Writes a `prompt/send` JSON message to the session's writer channel.
    ///
    /// # Errors
    ///
    /// - [`AppError::NotFound`] if `session_id` is not registered.
    /// - [`AppError::Acp`] if the writer channel is closed.
    fn send_prompt(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let session_id = session_id.to_owned();
        let prompt = prompt.to_owned();
        Box::pin(async move {
            let msg = json!({
                "method": "prompt/send",
                "params": { "text": prompt }
            });
            send_to_session(&self.stream_writers, &session_id, msg).await
        })
    }

    /// Request the agent to stop its current work.
    ///
    /// Writes a `session/interrupt` message to the agent's ACP stream.
    /// This operation is **idempotent** — if the session is already
    /// disconnected, the call returns `Ok(())` without error.
    fn interrupt(&self, session_id: &str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let session_id = session_id.to_owned();
        Box::pin(async move {
            let writers = self.stream_writers.lock().await;
            let Some(tx) = writers.get(&session_id) else {
                // Session already gone — idempotent, return Ok.
                debug!(
                    session_id,
                    "acp driver: interrupt on disconnected session — no-op"
                );
                return Ok(());
            };

            let msg = json!({
                "method": "session/interrupt",
                "params": { "reason": "Operator requested termination" }
            });

            tx.send(msg).await.map_err(|_| {
                warn!(
                    session_id,
                    "acp driver: interrupt send failed — stream closed"
                );
                AppError::Acp(format!(
                    "write failed: stream closed for session '{session_id}'"
                ))
            })
        })
    }

    /// Resolve a pending continuation prompt by writing a `prompt/response`
    /// to the agent's ACP stream.
    ///
    /// Looks up the owning session via `prompt_id` from `pending_prompts_acp`,
    /// then serialises the operator's decision.
    ///
    /// # Errors
    ///
    /// - [`AppError::NotFound`] if `prompt_id` has no registered pending entry.
    /// - [`AppError::Acp`] if the writer channel is closed.
    fn resolve_prompt(
        &self,
        prompt_id: &str,
        decision: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let prompt_id = prompt_id.to_owned();
        let decision = decision.to_owned();
        Box::pin(async move {
            let session_id = {
                let mut pending = self.pending_prompts_acp.lock().await;
                pending.remove(&prompt_id).map(|e| e.session_id)
            };

            let Some(session_id) = session_id else {
                return Err(AppError::NotFound(format!(
                    "no pending ACP prompt for prompt_id '{prompt_id}'"
                )));
            };

            let msg = json!({
                "method": "prompt/response",
                "id": prompt_id,
                "params": {
                    "decision": decision,
                    "instruction": instruction,
                }
            });

            send_to_session(&self.stream_writers, &session_id, msg).await
        })
    }

    /// Resolve a pending wait-for-instruction (standby) by sending an instruction.
    ///
    /// Writes a `prompt/send` message to the session's ACP stream containing
    /// the operator's instruction, or `"continue"` if none is provided.
    ///
    /// # Errors
    ///
    /// - [`AppError::NotFound`] if `session_id` is not registered.
    /// - [`AppError::Acp`] if the writer channel is closed.
    fn resolve_wait(
        &self,
        session_id: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let session_id = session_id.to_owned();
        Box::pin(async move {
            let text = instruction.unwrap_or_else(|| "continue".to_owned());
            let msg = json!({
                "method": "prompt/send",
                "params": { "text": text }
            });
            send_to_session(&self.stream_writers, &session_id, msg).await
        })
    }
}

// ── Private helper ────────────────────────────────────────────────────────────

/// Look up the writer for `session_id` and send `msg` through it.
///
/// Returns [`AppError::NotFound`] if no writer is registered for the session,
/// or [`AppError::Acp`] if the channel is closed (agent disconnected).
async fn send_to_session(writers: &WriterMap, session_id: &str, msg: Value) -> Result<()> {
    let writers = writers.lock().await;
    let Some(tx) = writers.get(session_id) else {
        return Err(AppError::NotFound(format!(
            "no ACP writer registered for session '{session_id}'"
        )));
    };

    tx.send(msg).await.map_err(|_| {
        warn!(session_id, "acp driver: send failed — stream closed");
        AppError::Acp(format!(
            "write failed: stream closed for session '{session_id}'"
        ))
    })
}
