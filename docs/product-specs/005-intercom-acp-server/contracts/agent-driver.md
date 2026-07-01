# Contract: AgentDriver Trait

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## Overview

The `AgentDriver` trait defines the protocol-agnostic interface between the shared application core (Slack handlers, persistence, policy) and the agent communication protocol (MCP or ACP). All operator actions that affect the agent flow through this trait.

## Trait Definition

```rust
pub trait AgentDriver: Send + Sync {
    /// Resolve a pending clearance request (approve or reject).
    ///
    /// In MCP: Sends the response through the oneshot channel.
    /// In ACP: Writes a clearance/response message to the agent stream.
    fn resolve_clearance(
        &self,
        request_id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Send a new prompt or instruction to the agent.
    ///
    /// In MCP: Posts an MCP notification or is a no-op (IDE owns the prompt).
    /// In ACP: Writes a prompt/send message to the agent stream.
    fn send_prompt(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Interrupt/cancel the agent's current work.
    ///
    /// In MCP: Sends a cancellation signal via the MCP transport.
    /// In ACP: Writes a session/interrupt message and optionally kills the process.
    fn interrupt(
        &self,
        session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Resolve a pending continuation prompt.
    ///
    /// In MCP: Sends the response through the prompt oneshot channel.
    /// In ACP: Writes the decision back to the agent stream.
    fn resolve_prompt(
        &self,
        prompt_id: &str,
        decision: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Resolve a pending wait-for-instruction (standby).
    ///
    /// In MCP: Sends through the wait oneshot channel.
    /// In ACP: Writes a prompt/send message with the instruction.
    fn resolve_wait(
        &self,
        session_id: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}
```

## AgentEvent Enum

Events emitted by driver implementations into the shared `mpsc` channel:

```rust
pub enum AgentEvent {
    ClearanceRequested {
        request_id: String,
        session_id: String,
        title: String,
        description: String,
        diff: Option<String>,
        file_path: String,
        risk_level: String,
    },
    StatusUpdated {
        session_id: String,
        message: String,
    },
    PromptForwarded {
        session_id: String,
        prompt_id: String,
        prompt_text: String,
        prompt_type: String,
    },
    HeartbeatReceived {
        session_id: String,
        progress: Option<Vec<ProgressItem>>,
    },
    SessionTerminated {
        session_id: String,
        exit_code: Option<i32>,
        reason: String,
    },
}
```

## MCP Driver Behavior

| Method | Implementation |
|--------|---------------|
| `resolve_clearance` | Removes `oneshot::Sender<ApprovalResponse>` from `pending_approvals` map, sends response |
| `send_prompt` | Sends MCP notification `intercom/nudge` via the notification context |
| `interrupt` | Sends MCP cancellation or drops the session connection |
| `resolve_prompt` | Removes `oneshot::Sender<PromptResponse>` from `pending_prompts` map, sends response |
| `resolve_wait` | Removes `oneshot::Sender<WaitResponse>` from `pending_waits` map, sends response |

## ACP Driver Behavior

| Method | Implementation |
|--------|---------------|
| `resolve_clearance` | Looks up session_id from pending map key `(session_id, request_id)`, serializes `clearance/response` JSON, writes to the session's stream via `stream_writers[session_id]` channel |
| `send_prompt` | Looks up `stream_writers[session_id]`, serializes `prompt/send` JSON, writes to agent stream |
| `interrupt` | Looks up `stream_writers[session_id]`, serializes `session/interrupt` JSON, writes to agent stream, optionally kills process |
| `resolve_prompt` | Looks up session_id from pending map key `(session_id, prompt_id)`, serializes prompt decision JSON, writes to agent stream |
| `resolve_wait` | Looks up `stream_writers[session_id]`, serializes `prompt/send` JSON with instruction, writes to agent stream |

### AcpDriver Internal Structure

```rust
/// ACP driver managing multiple concurrent agent sessions.
pub struct AcpDriver {
    /// Per-session stream writers: session_id → sender channel
    stream_writers: Arc<Mutex<HashMap<String, mpsc::Sender<Value>>>>,
    /// Pending clearance requests keyed by (session_id, request_id)
    pending_clearances: Arc<Mutex<HashMap<(String, String), ClearanceState>>>,
    /// Pending prompt requests keyed by (session_id, prompt_id)
    pending_prompts: Arc<Mutex<HashMap<(String, String), PromptState>>>,
}
```

### Session Lifecycle Methods

```rust
impl AcpDriver {
    /// Register a new session's stream writer.
    pub fn register_session(&self, session_id: &str, writer: mpsc::Sender<Value>);
    /// Remove a session's stream writer on termination.
    pub fn deregister_session(&self, session_id: &str);
}
```

## Error Cases

| Scenario | Expected Behavior |
|----------|------------------|
| `resolve_clearance` with unknown `(session_id, request_id)` | Return `AppError::NotFound` |
| `send_prompt` to disconnected session | Return `AppError::Acp("stream closed")` |
| `send_prompt` to unknown session_id | Return `AppError::NotFound` |
| `interrupt` on already-terminated session | Return `Ok(())` (idempotent) |
| Stream write failure | Return `AppError::Acp` with the underlying I/O error |
| Action by non-owner user | Return `AppError::Unauthorized` (FR-031) |

## Test Contract

All driver implementations must pass these contract tests:

1. **resolve_clearance approved** — resolves pending request, event loop receives the approval
2. **resolve_clearance rejected** — resolves pending request, event loop receives the rejection with reason
3. **resolve_clearance unknown** — returns `NotFound` error
4. **send_prompt** — delivers prompt text to agent; for ACP, the stream contains the serialized message
5. **interrupt** — signals the agent to stop; for ACP, the stream contains the interrupt message
6. **concurrent resolution** — two clearance requests resolved concurrently, both succeed without data races
7. **session-scoped IDs** — two sessions both have `req-001`; resolving one does not affect the other
8. **owner verification** — action by non-owner user returns `Unauthorized`
9. **ACP prompt resolution** — resolve_prompt writes `prompt/response` to correct session stream
10. **ACP wait resolution** — resolve_wait writes `prompt/send` to correct session stream

## Owner Verification

All session-modifying methods (`resolve_clearance`, `send_prompt`, `interrupt`, `resolve_prompt`, `resolve_wait`) require that the caller provides the acting user's Slack ID. The driver (or the calling handler layer) MUST verify that this user matches the session's `owner_user_id` before proceeding. Non-owner actions MUST return `AppError::Unauthorized`.
