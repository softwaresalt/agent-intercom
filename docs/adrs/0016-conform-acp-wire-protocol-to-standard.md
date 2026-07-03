# ADR-0016: Conform the ACP Wire Protocol to the Standard; Terminology Mapping Lives in the Operator UX

**Status**: Accepted
**Date**: 2026-07-02
**Phase**: 013-F ACP-Only Remote Controller, F.1 (blocking gate)
**Supersedes intent of**: the bespoke NDJSON dialect assumptions in ADR-0014, ADR-0015

## Context

Epic 013-F converges `agent-intercom` onto ACP as the sole remote-control
surface. The highest-leverage open question (F.1) was whether to **conform** the
current wire protocol to the public Agent Client Protocol (ACP; JSON-RPC 2.0) or
to **formalize** the existing bespoke NDJSON dialect as our own spec.

Two evidence artifacts were produced:

- **F.1-T1 gap analysis** (`docs/decisions/2026-07-02-acp-wire-protocol-gap-analysis.md`):
  of 16 wire elements, 6 match standard ACP exactly, 5 are semantic-but-renamed,
  and 5 are bespoke-only. The permission flow is inverted (bespoke
  `clearance/request|response` vs standard `session/request_permission`), replies
  reuse the request `id` as new method-messages instead of JSON-RPC
  `result`/`error`, and `session/interrupt` diverges from `session/cancel`.
- **F.1-T2 live test-drive** (`docs/decisions/2026-07-02-acp-live-test-drive-copilot-acp.md`):
  against a real conformant agent (`copilot --acp`), the bespoke methods
  (`prompt/forward`, `clearance/response`, `session/interrupt`) are rejected with
  JSON-RPC `-32601 Method not found`; the real `initialize` returns
  `agentCapabilities`/`authMethods`; and `session/new` is gated behind an auth
  handshake we do not perform.

**Operator design intent (decisive):** the purpose of agent-intercom's "dialect"
was **never** to deviate from ACP on the wire. It was to map product-specific,
operator-friendly terminology onto ACP concepts so a human can steer the agent
intuitively from Slack. The wire-level divergence found above is therefore
**drift from that intent**, not a design to be blessed.

## Decision

**Conform the wire protocol to the public ACP standard. Keep operator-facing
terminology as an explicit presentation-layer mapping, not as wire divergence.**

Three principles:

1. **Wire = standard ACP.** The stdio transport speaks conformant JSON-RPC 2.0
   ACP: standard `initialize` (with `clientCapabilities`, consuming
   `agentCapabilities`), `authenticate` per advertised `authMethods`,
   `session/new`, `session/prompt`, `session/request_permission`,
   `session/update`, `session/cancel`, with replies as JSON-RPC `result`/`error`.
   NDJSON framing is retained (already compatible).

2. **Terminology = presentation mapping.** Operator-friendly terms live in the
   Slack/UX layer and are translated to/from standard ACP concepts through a
   single documented mapping table (below). The mapping is intentional and
   centralized — never re-encoded as new wire method names.

3. **Genuinely custom features ride on standard extension points.** Capabilities
   with no ACP method (forwarding a prompt to the operator's phone, heartbeat
   liveness) are expressed through standard ACP where a fit exists
   (`session/update` variants, `_meta`) or kept as clearly-labeled local
   extensions layered above conformance — never as replacements for standard
   methods.

### Operator terminology → ACP mapping (presentation layer)

| Operator-facing term (Slack/UX) | Standard ACP concept (wire) |
|---|---|
| "Approve / reject this action" | `session/request_permission` → permission `outcome` in `result` |
| "Clearance" | permission request/response lifecycle |
| "Forward prompt to my phone" | operator relay atop `session/update` / local extension (not a wire method) |
| "Steer / send instruction" | `session/prompt` |
| "Stop / interrupt" | `session/cancel` (notification with `sessionId`) |
| "Status / activity" | `session/update` (`agent_message_chunk`, tool-call updates) |
| "Heartbeat / is it alive" | liveness atop transport / local extension (not a wire method) |

*(This table is the authoritative source for the mapping; implementation phases
must not introduce operator terms that bypass it.)*

## Migration implications (inputs to F.2/F.3/F.5)

- **`initialize`**: send standard params (`protocolVersion`, `clientCapabilities`);
  consume and store `agentCapabilities`; stop sending `processId`/`workspaceFolders`
  and the bespoke `initialized` notification.
- **Auth**: implement the `authenticate` handshake driven by the agent's
  advertised `authMethods` before `session/new`.
- **Permission flow (highest risk)**: add a `session/request_permission` handler
  (currently absent — a real agent's request is silently dropped) and reply with
  a proper JSON-RPC `result` carrying the permission `outcome`. Retire
  `clearance/request|response` from the wire; keep "clearance" as an operator UX term.
- **Replies**: emit JSON-RPC `result`/`error` for agent-initiated requests; stop
  reusing the request `id` as a new method-message.
- **Cancel**: replace `session/interrupt` with `session/cancel` (notification,
  includes `sessionId`).
- **Streaming**: consume the full `session/update` variant set and the prompt-turn
  `stopReason` (currently ignored) for turn completion.
- **Remove bespoke-only wire methods**: `prompt/forward`, `prompt/response`,
  `status/update`, `heartbeat`, `seq`, `initialized` — re-expressed per principle 3.

## Consequences

### Positive

- **Interoperability**: any conformant ACP agent (Copilot, Claude Code, Gemini,
  Zed) can be driven — the point of "ACP-only as the primary remote."
- **Correctness**: fixes the silently-dropped permission request and the
  reply-shape mismatch that would deadlock a real agent.
- **Clarity**: the operator terminology becomes a single reviewed mapping rather
  than accidental protocol drift.

### Negative / Risks

- **Breaking wire change**: the bespoke dialect and any client bound to it must
  be migrated; this is scoped as F.2 (correctness) → F.3 (hardening) → F.5
  (make ACP default, remove MCP). F.5 is irreversible and gated on this ADR
  (see 013-F operator decision, 2026-07-02).
- **Auth surface**: implementing `authenticate` adds a credential path per agent
  vendor.
- **Live end-to-end validation still pending**: F.1-T2 confirmed the handshake and
  method surface without completing auth; a full authed `session/prompt` +
  `session/request_permission` round-trip should be validated during F.2.

## References

- `docs/decisions/2026-07-02-acp-wire-protocol-gap-analysis.md` (F.1-T1)
- `docs/decisions/2026-07-02-acp-live-test-drive-copilot-acp.md` (F.1-T2)
- `docs/decisions/2026-06-30-acp-only-remote-controller-spike.md` (epic spike)
- `docs/plans/2026-07-01-acp-only-remote-controller-plan.md` (013-F plan)
