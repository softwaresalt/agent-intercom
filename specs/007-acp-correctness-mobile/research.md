# Research: Feature 007 — ACP Correctness Fixes and Mobile Operator Accessibility

**Feature**: 007-acp-correctness-mobile
**Date**: 2026-03-08

## R-01: Steering Message Consumption Ordering (F-06)

**Question**: What is the correct ordering for `send_prompt` and `mark_consumed` in the
steering flush loop?

**Decision**: `mark_consumed` must only be called after `send_prompt` succeeds.

**Rationale**: The current code at `src/acp/reader.rs:455-464` calls `mark_consumed()`
unconditionally after `send_prompt()`, even when delivery fails. This means a transient
delivery error permanently loses the steering message. The fix inverts the control flow:
on `send_prompt` success, mark consumed; on failure, log a warning and continue to the
next message (leaving the failed message unconsumed for retry on next reconnect).

**Alternatives considered**:
- Add a retry loop around `send_prompt`: Rejected — the reconnect flush already provides
  natural retry semantics. A retry loop would add complexity and delay processing of other
  queued messages.
- Move to a dead-letter queue: Rejected — overengineered for the current scale. The
  unconsumed state in the existing DB table serves the same purpose.

## R-02: ACP Session Capacity Query (F-07)

**Question**: How should session capacity be counted for ACP sessions?

**Decision**: Add `count_active_acp()` that queries
`WHERE (status = 'active' OR status = 'created') AND protocol_mode = 'acp'`.

**Rationale**: The current `count_active()` method only counts `status = 'active'` sessions
and does not filter by protocol. This has two bugs: (1) sessions in `created` (initializing)
state are not counted, creating a race window where more sessions can start than the limit
allows; (2) MCP sessions count against the ACP limit. The new method fixes both.

**Alternatives considered**:
- Modify existing `count_active()`: Rejected — other callers may depend on the current
  behavior. Adding a new method is safer.
- Use `SELECT COUNT(*) WHERE status NOT IN ('terminated', 'interrupted')`: Too broad — would
  include paused sessions in the count.

## R-03: `channel_id` Query Parameter Removal (F-10)

**Question**: Should the `?channel_id=` query parameter on `/mcp` be deprecated or removed?

**Decision**: Remove entirely. Only `?workspace_id=` and `?session_id=` are accepted.

**Rationale**: The project has fully migrated to workspace-based routing. The `channel_id`
query parameter is a legacy artifact from pre-005 when channels were specified directly.
No external consumers use it. Keeping it adds dead code and untested fallback paths.

**Scope boundaries**:
- `[slack] channel_id` in config.toml (default channel) is KEPT — different concern
- `IntercomServer::with_channel_override()` internal API is KEPT — receives channels resolved
  from workspace mapping, not from URL parameters
- `resolve_channel_id()` in `config.rs` signature updated — `channel_id` fallback parameter removed

**Alternatives considered**:
- Add deprecation warning only: Rejected by operator — no reason to maintain dead code.
- Remove `resolve_channel_id()` entirely: Deferred — still useful for workspace-to-channel
  resolution, just without the `channel_id` fallback parameter.

## R-04: Prompt Correlation ID Strategy (F-13)

**Question**: How to ensure prompt correlation IDs are unique across sessions and restarts?

**Decision**: Use `Uuid::new_v4()` for all correlation IDs (handshake and runtime prompts).

**Rationale**: The current system has two ID schemes that collide:
- `src/acp/handshake.rs:46` defines `PROMPT_ID = "intercom-prompt-1"` (static string)
- `src/driver/acp_driver.rs:54` starts `PROMPT_COUNTER` at 1 and generates IDs like
  `"intercom-prompt-{N}"` where N starts at 1 — identical to the handshake constant

UUIDs eliminate all collision risk without coordination. The `uuid` crate (v1.7) is already
a workspace dependency.

**ID format**: `"intercom-{purpose}-{uuid}"` where purpose is `init`, `sess`, or `prompt`.

**Alternatives considered**:
- Start counter at 1000: Simpler but still has collision risk across server restarts
  (two server instances could generate the same counter value).
- Use `{session_id}-{counter}`: Session-scoped uniqueness but verbose and still
  counter-based within a session.

## R-05: Mobile Modal Behavior (F-15)

**Question**: Do Slack modals (views.open) with `plain_text_input` work on iOS?

**Decision**: Desk research first; HITL verification post-build.

**Research status**: PENDING — to be completed during implementation Phase 5.

**Known data points** (pre-research):
- Slack Block Kit documentation does not explicitly list mobile limitations for modals
- Slack community reports suggest modals may have reduced functionality on mobile
- The agent-intercom server uses `views.open` (not `views.push`) for all modal interactions
- Three modal interactions exist: Refine (prompt), Rejection reason (approval), Resume with
  instructions (wait)

**Output**: `specs/007-acp-correctness-mobile/research-f15-mobile-modals.md`
