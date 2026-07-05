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
use std::sync::{atomic::AtomicU64, Arc};

use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::driver::{AgentDriver, PermissionOption};
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

/// Tracks a pending standard `session/request_permission` (ADR-0016) and the
/// options offered so the operator decision can be echoed as an `optionId`.
#[derive(Debug, Clone)]
struct PendingPermission {
    session_id: String,
    options: Vec<PermissionOption>,
    /// The original JSON-RPC `id` value, echoed verbatim in the `result` reply
    /// to preserve its type (numeric ids from conformant agents must not be
    /// stringified).
    raw_id: Value,
}

/// Shared map type alias for session writer channels.
type WriterMap = Arc<Mutex<HashMap<String, mpsc::Sender<Value>>>>;

/// Shared map type alias for agent-assigned session IDs.
type AgentSessionIdMap = Arc<Mutex<HashMap<String, String>>>;

/// Shared map type alias for per-session outbound sequence counters.
type SeqCounterMap = Arc<Mutex<HashMap<String, Arc<AtomicU64>>>>;

// ── AcpDriver ─────────────────────────────────────────────────────────────────

/// ACP protocol driver — routes operator actions to the correct agent stream.
///
/// Maintains five shared maps protected by async mutexes:
///
/// - `stream_writers`: `session_id` → [`mpsc::Sender<Value>`] registered when
///   an ACP session connects.
/// - `agent_session_ids`: intercom `session_id` → ACP `agent_session_id`
///   (from the `session/new` handshake result).
/// - `seq_counters`: `session_id` → [`Arc<AtomicU64>`] shared with the
///   [`run_writer`] task for monotonic outbound sequence numbers (ES-008).
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
    /// Intercom session ID → ACP agent-assigned session ID mapping.
    agent_session_ids: AgentSessionIdMap,
    /// Per-session outbound sequence counters shared with each `run_writer` task.
    seq_counters: SeqCounterMap,
    /// Pending clearance requests: `request_id` → owning session metadata.
    pending_clearances: Arc<Mutex<HashMap<String, PendingClearance>>>,
    /// Pending prompt-forward requests: `prompt_id` → owning session metadata.
    pending_prompts_acp: Arc<Mutex<HashMap<String, PendingPromptAcp>>>,
    /// Pending standard permission requests: `request_id` → session + options.
    pending_permissions: Arc<Mutex<HashMap<String, PendingPermission>>>,
}

impl AcpDriver {
    /// Create a new `AcpDriver` with empty maps.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stream_writers: Arc::new(Mutex::new(HashMap::new())),
            agent_session_ids: Arc::new(Mutex::new(HashMap::new())),
            seq_counters: Arc::new(Mutex::new(HashMap::new())),
            pending_clearances: Arc::new(Mutex::new(HashMap::new())),
            pending_prompts_acp: Arc::new(Mutex::new(HashMap::new())),
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new session's outbound writer channel.
    ///
    /// Creates a fresh [`AtomicU64`] sequence counter for the session, stores
    /// it in the driver's counter map, and returns it so the caller can pass
    /// it directly to [`run_writer`].
    ///
    /// If a session with the same `session_id` is already registered, the old
    /// sender and counter are replaced with the new ones.
    pub async fn register_session(
        &self,
        session_id: &str,
        tx: mpsc::Sender<Value>,
    ) -> Arc<AtomicU64> {
        let counter = Arc::new(AtomicU64::new(0));
        {
            self.stream_writers
                .lock()
                .await
                .insert(session_id.to_owned(), tx);
        }
        {
            self.seq_counters
                .lock()
                .await
                .insert(session_id.to_owned(), Arc::clone(&counter));
        }
        debug!(session_id, "acp driver: session writer registered");
        counter
    }

    /// Register the ACP agent-assigned session ID for an intercom session.
    ///
    /// Must be called after `session/new` handshake completes. The agent
    /// session ID is required for constructing `session/prompt` messages.
    pub async fn register_agent_session_id(&self, session_id: &str, agent_session_id: &str) {
        self.agent_session_ids
            .lock()
            .await
            .insert(session_id.to_owned(), agent_session_id.to_owned());
        debug!(
            session_id,
            agent_session_id, "acp driver: agent session ID registered"
        );
    }

    /// Remove a session's writer channel on disconnection or termination.
    ///
    /// Also removes the agent session ID mapping, sequence counter, and any
    /// pending clearance or prompt-forward entries owned by this session.
    /// Idempotent — removing an unknown `session_id` is a no-op.
    pub async fn deregister_session(&self, session_id: &str) {
        self.stream_writers.lock().await.remove(session_id);
        self.agent_session_ids.lock().await.remove(session_id);
        self.seq_counters.lock().await.remove(session_id);

        // F-09: remove any pending clearance / prompt entries owned by this
        // session so they do not accumulate as memory leaks after termination.
        self.pending_clearances
            .lock()
            .await
            .retain(|_, v| v.session_id != session_id);
        self.pending_prompts_acp
            .lock()
            .await
            .retain(|_, v| v.session_id != session_id);

        debug!(session_id, "acp driver: session deregistered");
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

    /// Register a pending standard permission request for response routing.
    ///
    /// Called by the event consumer when [`AgentEvent::PermissionRequested`] is
    /// received (ADR-0016), before posting the Slack approval message. Stores
    /// the offered `options` so [`AgentDriver::resolve_clearance`] can echo the
    /// selected `optionId` in the JSON-RPC `result` outcome.
    pub async fn register_permission(
        &self,
        session_id: &str,
        request_id: &str,
        options: Vec<PermissionOption>,
        raw_id: Value,
    ) {
        self.pending_permissions.lock().await.insert(
            request_id.to_owned(),
            PendingPermission {
                session_id: session_id.to_owned(),
                options,
                raw_id,
            },
        );
        debug!(
            session_id,
            request_id, "acp driver: permission request registered"
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
            // Bespoke clearance path: reply with a `clearance/response` message.
            let clearance_session = {
                let mut pending = self.pending_clearances.lock().await;
                pending.remove(&request_id).map(|e| e.session_id)
            };
            if let Some(session_id) = clearance_session {
                let msg = json!({
                    "method": "clearance/response",
                    "id": request_id,
                    "params": {
                        "status": if approved { "approved" } else { "rejected" },
                        "reason": reason,
                    }
                });
                return send_to_session(&self.stream_writers, &session_id, msg).await;
            }

            // Standard ACP permission path (ADR-0016): reply with a JSON-RPC
            // `result` carrying the selected option's `outcome`.
            let permission = {
                let mut pending = self.pending_permissions.lock().await;
                pending.remove(&request_id)
            };
            if let Some(PendingPermission {
                session_id,
                options,
                raw_id,
            }) = permission
            {
                let outcome = match select_option_id(&options, approved) {
                    Some(option_id) => json!({ "outcome": "selected", "optionId": option_id }),
                    None => json!({ "outcome": "cancelled" }),
                };
                let msg = json!({
                    "jsonrpc": "2.0",
                    "id": raw_id,
                    "result": { "outcome": outcome },
                });
                return send_to_session(&self.stream_writers, &session_id, msg).await;
            }

            Err(AppError::NotFound(format!(
                "no pending ACP clearance or permission for request_id '{request_id}'"
            )))
        })
    }

    /// Send a new prompt or instruction to the agent's ACP stream.
    ///
    /// Writes a `session/prompt` JSON-RPC message containing the agent-assigned
    /// session ID and the prompt text as an array element, matching the ACP
    /// protocol format.
    ///
    /// # Errors
    ///
    /// - [`AppError::NotFound`] if `session_id` is not registered or has no
    ///   agent session ID.
    /// - [`AppError::Acp`] if the writer channel is closed.
    fn send_prompt(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let session_id = session_id.to_owned();
        let prompt = prompt.to_owned();
        Box::pin(async move {
            let agent_sid = self
                .agent_session_ids
                .lock()
                .await
                .get(&session_id)
                .cloned();

            let Some(agent_sid) = agent_sid else {
                return Err(AppError::NotFound(format!(
                    "no ACP agent session ID registered for session '{session_id}'"
                )));
            };

            // F-13: generate a unique UUID-based correlation ID — no static
            // counter that would collide across sessions or server restarts.
            let req_id = format!("intercom-prompt-{}", Uuid::new_v4());

            let msg = json!({
                "jsonrpc": "2.0",
                "method": "session/prompt",
                "id": req_id,
                "params": {
                    "sessionId": agent_sid,
                    "prompt": [
                        { "type": "text", "text": prompt }
                    ]
                }
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
            // Clone the sender and drop the lock before awaiting (F-04).
            let tx = {
                let writers = self.stream_writers.lock().await;
                writers.get(&session_id).cloned()
            };
            let Some(tx) = tx else {
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
    /// Writes a `session/prompt` JSON-RPC message to the session's ACP stream
    /// containing the operator's instruction, or `"continue"` if none is
    /// provided.
    ///
    /// # Errors
    ///
    /// - [`AppError::NotFound`] if `session_id` is not registered or has no
    ///   agent session ID.
    /// - [`AppError::Acp`] if the writer channel is closed.
    fn resolve_wait(
        &self,
        session_id: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let session_id = session_id.to_owned();
        Box::pin(async move {
            let text = instruction.unwrap_or_else(|| "continue".to_owned());

            let agent_sid = self
                .agent_session_ids
                .lock()
                .await
                .get(&session_id)
                .cloned();

            let Some(agent_sid) = agent_sid else {
                return Err(AppError::NotFound(format!(
                    "no ACP agent session ID registered for session '{session_id}'"
                )));
            };

            // F-13: generate a unique UUID-based correlation ID.
            let req_id = format!("intercom-prompt-{}", Uuid::new_v4());

            let msg = json!({
                "jsonrpc": "2.0",
                "method": "session/prompt",
                "id": req_id,
                "params": {
                    "sessionId": agent_sid,
                    "prompt": [
                        { "type": "text", "text": text }
                    ]
                }
            });
            send_to_session(&self.stream_writers, &session_id, msg).await
        })
    }
}

// ── Private helper ────────────────────────────────────────────────────────────

/// Choose the `optionId` to echo in a permission `outcome`.
///
/// On approval, prefer an `allow_once` option, then `allow_always`, then the
/// first offered option. On rejection, prefer `reject_once`, then
/// `reject_always`, then the last offered option. Returns `None` when no
/// options are offered, signalling a `cancelled` outcome.
fn select_option_id(options: &[PermissionOption], approved: bool) -> Option<String> {
    if options.is_empty() {
        return None;
    }
    let preferred: [&str; 2] = if approved {
        ["allow_once", "allow_always"]
    } else {
        ["reject_once", "reject_always"]
    };
    for kind in preferred {
        if let Some(opt) = options.iter().find(|o| o.kind == kind) {
            return Some(opt.option_id.clone());
        }
    }
    if approved {
        options.first().map(|o| o.option_id.clone())
    } else {
        options.last().map(|o| o.option_id.clone())
    }
}

/// Look up the writer for `session_id` and send `msg` through it.
///
/// Clones the sender and drops the lock before awaiting the send to avoid
/// holding the mutex across an `.await` point (F-04).
///
/// Returns [`AppError::NotFound`] if no writer is registered for the session,
/// or [`AppError::Acp`] if the channel is closed (agent disconnected).
async fn send_to_session(writers: &WriterMap, session_id: &str, msg: Value) -> Result<()> {
    let tx = {
        let guard = writers.lock().await;
        guard.get(session_id).cloned()
    };
    let Some(tx) = tx else {
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
