# ADR-0011: Reconnect Re-Post of Pending Interactive Messages

**Status**: Accepted
**Date**: 2026-02-12
**Phase**: 14 (Polish & Cross-Cutting Concerns), Task T095

## Context

The Slack Socket Mode WebSocket connection can drop due to network
interruptions, server-side disconnects, or mobile network transitions.
`slack-morphism` handles reconnection automatically via its built-in
backoff and heartbeat configuration, but any interactive messages
posted *before* the drop may no longer be actionable if the
Slack client missed button-press events during the disconnect window.

Pending approval requests and continuation prompts that were in-flight
at disconnect time would leave the agent blocked indefinitely, waiting
for an operator response that will never arrive through the lost message.

## Decision

Hooked into the Socket Mode `hello` event callback, which fires on every
new WebSocket connection (including reconnections after a drop):

1. On each `hello` event, query the DB for pending approval requests
   (`ApprovalRepo::list_pending`) and pending continuation prompts
   (`PromptRepo::list_pending`).
2. For each pending record, re-post a new interactive message to the
   configured Slack channel with fresh action buttons and a
   "[Re-posted after reconnect]" prefix.
3. The original `oneshot` senders remain valid in the in-memory
   `pending_approvals` and `pending_prompts` maps, so button presses
   on the re-posted messages resolve the waiting tool calls as normal.

The re-post runs asynchronously via the existing message queue (rate-
limited) so it does not block the `hello` event acknowledgment.

## Consequences

### Positive

- Eliminates indefinite agent blocking after WebSocket reconnection.
- Operator sees fresh actionable messages in the channel after any
  network interruption.
- Leverage existing `list_pending()` queries — no new persistence code.

### Negative

- On initial startup (first `hello`), the function also runs but
  typically finds no pending records, resulting in a harmless no-op.
- If the WebSocket reconnects rapidly, multiple re-posts of the same
  pending record could appear in the channel. Only the first button
  press resolves the oneshot; subsequent presses on duplicate messages
  see "no pending oneshot found" and are harmless.

### Risks

- High-frequency reconnections (flapping network) could flood the
  channel with duplicate re-posts. Mitigated by Slack's rate limiting
  and the exponential backoff in the message queue.
