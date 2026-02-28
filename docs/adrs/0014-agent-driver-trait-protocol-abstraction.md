# ADR-0014: AgentDriver Trait for Protocol-Agnostic Agent Communication

**Status**: Accepted
**Date**: 2026-02-28
**Feature**: 005-intercom-acp-server, Phase 1

## Context

`agent-intercom` began as an MCP-only server: agents communicate via the
Model Context Protocol and the server resolves clearances, prompts, and wait
calls through `tokio::sync::oneshot` channels stored in `AppState`. When the
ACP (Agent Client Protocol) mode was introduced, it required the same
clearance/prompt/wait resolution semantics but over a newline-delimited JSON
stdio stream instead of MCP notifications.

Without an abstraction layer, Slack handlers and the orchestrator would need
to branch on protocol mode for every operator action — doubling the code paths
and coupling the business logic to transport details.

## Decision

Introduce an `AgentDriver` trait in `src/driver/mod.rs` that provides a
uniform interface for all protocol-specific agent interactions:

- `resolve_clearance(request_id, approved, reason)` — approve or reject a
  pending clearance request
- `send_prompt(session_id, prompt)` — deliver a prompt/instruction to the agent
- `interrupt(session_id)` — signal the agent to stop current work (idempotent)
- `resolve_prompt(prompt_id, decision, instruction)` — respond to a forwarded
  continuation prompt
- `resolve_wait(session_id, instruction)` — unblock a standby wait

Each method returns `Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>`,
enabling async implementations without the `async_trait` dependency.

Two concrete implementations will follow in later phases:
- `McpDriver` — wraps the existing `PendingApprovals`/`PendingPrompts`/
  `PendingWaits` oneshot maps from `AppState`
- `AcpDriver` — routes messages to per-session stdio stream writers

## Consequences

**Positive**:
- Slack handlers and the orchestrator call a single `driver.resolve_clearance()`
  regardless of whether the session is MCP or ACP
- Adding a third protocol in the future requires only a new `AgentDriver` impl,
  not changes to every handler
- Clean separation between transport mechanics (`driver/`) and stream framing
  (`acp/`)

**Negative**:
- `Pin<Box<dyn Future>>` has a small heap allocation per call compared to
  monomorphized async functions. Acceptable overhead for human-in-the-loop
  interactions that are rare and inherently latency-tolerant.
- The trait cannot use `async fn` without `async_trait` (stabilized in Rust
  1.75 but not yet compatible with `dyn Trait`). The `Pin<Box>` pattern is the
  idiomatic workaround until `async fn in dyn trait` is fully stable.

**Risks**:
- Object safety depends on all method signatures being object-safe. Verified:
  no generic type parameters in method signatures, no `Self` return types.
