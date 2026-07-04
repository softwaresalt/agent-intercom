---
title: "ACP-Native Tool-Serving Re-Home — Implementation Plan"
type: plan
date: 2026-07-04
source: task 013.005.008-T (F.5 prerequisite)
related_adrs: ["docs/adrs/0016-conform-acp-wire-protocol-to-standard.md"]
related_backlog: ["013.005-F", "013.005.003-T"]
status: draft-for-review
---

## Problem Frame

Today the intercom serves its **agent-facing tools** (approval, diff-apply, policy
check, heartbeat, prompt-forward, broadcast, recover, mode-switch, standby) over an
**HTTP MCP endpoint** (rmcp) under `src/mcp/`. Spawned agents — **including ACP-mode
agents** — call these tools over HTTP (`main.rs:365-368`, HITL-003/FR-032). This
couples the tool API to the MCP surface, so `src/mcp/*` cannot be deleted (F.5-T4 /
`013.005.003-T`) without first giving ACP its **own native tool path**.

Per operator direction and ADR-0016: MCP and ACP each own their endpoint; ACP must
carry its tool interactions over the ACP stdio stream (standard messages where they
exist, clearly-labeled local extensions where they do not) — never by reaching
through MCP-path code.

## Current State (from the tool catalogue)

Nine agent-facing tools live in `src/mcp/tools/*`. Each is a **thin MCP wrapper**
(`ToolCallContext<IntercomServer>` → `CallToolResult`) around **shared core logic**
that already lives in protocol-neutral modules: `persistence/*` (repos),
`slack/*` (blocks + client), `policy/*` (loader + evaluator), `diff/*` (patcher +
writer). The MCP wrapper contributes: input deserialization, the
oneshot-channel blocking pattern (`pending_approvals`/`pending_prompts`/
`pending_waits`), and JSON response shaping.

The ACP stream stack already exists: `src/acp/reader.rs` parses inbound NDJSON and
emits `AgentEvent`s; `src/acp/writer.rs::run_writer` writes outbound messages
(seq-stamped); `AcpDriver` already implements `resolve_clearance` / `resolve_prompt`
/ `resolve_wait` / `send_prompt` / `interrupt` by **writing back over the stream**.
The reader currently handles **bespoke** methods (`clearance/request`,
`prompt/forward`, `status/update`, `heartbeat`, plus one `session/update`).

**ADR-0016 gap (blocking-adjacent):** the standard `session/request_permission`
handler is **absent** — a real conformant agent's permission request is silently
dropped. This must be added as part of (or before) this work.

## Design

**Approach: an ACP-native tool/handler layer that reuses the existing shared core.**
No shared logic is rewritten — only the *protocol wrapper* changes from
MCP(HTTP/rmcp) to ACP(stdio/JSON-RPC). New module: `src/acp/tools/` (mirrors the
role of `src/mcp/tools/` but ACP-native), dispatched from `reader.rs` and replying
via the `AcpDriver`/`writer.rs` stream.

**Operator decisions (2026-07-04) shape the interaction model:**
- **(D1) Drop `accept_diff`** — Copilot mobile already surfaces live PR/diff info;
  intercom must not duplicate diff tooling. Not re-homed.
- **(D2) No Slack approval buttons** — all operator decisions are **plain text
  replies** to the agent waiting in ACP mode. Requests are relayed as text to the
  Slack thread; the operator's **text reply** is parsed and returned to the agent
  over the ACP stream. This removes Slack `blocks.rs` buttons and the
  oneshot-button-callback path from the ACP tool layer. The existing
  `src/slack/handlers/thread_reply.rs` (`register_thread_reply_fallback`,
  `parse_thread_decision`, `message_is_in_thread`) becomes the **primary** operator
  path (no longer a fallback), delivering results via
  `AcpDriver::resolve_clearance/resolve_prompt/resolve_wait` over the stream.
- **(D3) Drop `set_operational_mode`** — ACP mode is remote-by-design in this
  scenario, so an agent-initiated routing-mode switch is unnecessary. Not re-homed.

The three operator-decision tools (approval, forward-prompt, standby) therefore
converge onto **one text-reply mechanism**: relay request → operator text reply in
thread → parse → respond to agent over the ACP stream.

### Per-tool ACP-native mapping (revised per operator decisions)

| Op name / MCP tool | ACP-native target (revised) | Class |
|---|---|---|
| check_clearance / `ask_approval` | Standard **`session/request_permission`**; relay as **text**; operator **text reply** → JSON-RPC `result` `outcome` over the stream | (a) standard + text-reply |
| auto_check / `check_auto_approve` | Fold into permission flow: auto-respond from workspace policy; relay to operator (text) only when policy does not auto-approve | (a) absorbed |
| transmit / `forward_prompt` | **Text-relay**: post prompt as text; operator **text reply** → `resolve_prompt` → `session/prompt` | (b) text-reply |
| standby / `wait_for_instruction` | **Text-relay**: agent pauses; operator **text reply** → `resolve_wait` → `session/prompt` | (b) text-reply |
| broadcast / `remote_log` | Standard **`session/update`** (`agent_message_chunk`) → Slack (one-way) | (a) standard |
| ping / `heartbeat` | **Local extension** atop `session/update` (progress snapshot + steering pickup + stall reset); liveness is transport-level | (b) local ext |
| reboot / `recover_state` | **Local extension**: agent requests its pending permission/prompt/checkpoint/inbox state after restart | (b) local ext |
| check_diff / `accept_diff` | **DROPPED (D1)** — Copilot mobile handles diffs/PRs | (c) drop |
| switch_freq / `set_operational_mode` | **DROPPED (D3)** — ACP is remote-by-design; agent-initiated mode switch unnecessary | (c) drop |

Legend: (a) standard ACP message · (b) local ACP extension · (c) dropped.

## Implementation Units (proposed subtasks of 013.005.008-T)

Each unit targets < 3 files / < 4 test scenarios and reuses shared core. Scope is
reduced by D1 (drop `accept_diff`) and D2 (text-reply, no Slack buttons).

* **T8.1 — Wire conformance: `session/request_permission` handler (ADR-0016).**
  Add the standard permission-request parser to `src/acp/reader.rs` and reply with
  JSON-RPC `result` carrying `outcome`; wire into `AcpDriver::resolve_clearance`.
  Files: `src/acp/reader.rs`, `src/acp/writer.rs` (reply shaping), tests. Posture:
  test-first. *(Foundational — unblocks the approval path.)*
* **T8.2 — ACP tool scaffolding + text-reply dispatch.** Create `src/acp/tools/mod.rs`
  and a dispatcher in `reader.rs`; promote `thread_reply` (text parsing) to the
  **primary** operator-response path for ACP (relay-as-text → parse text reply →
  `resolve_*`). Posture: test-first.
* **T8.3 — Approval + policy (check_clearance + auto_check), text-reply.** ACP-native
  handler reusing `ApprovalRepo`, `PolicyLoader`/`PolicyEvaluator`; policy
  auto-response, else relay as text and parse the operator's text reply → JSON-RPC
  `result`. **No Slack buttons.** Posture: test-first.
* **T8.4 — Status path (broadcast + ping).** `session/update` → Slack; heartbeat
  progress/steering/stall-reset local extension. Posture: test-first.
* **T8.5 — Operator-relay path (transmit + standby), text-reply.** forward_prompt +
  wait as text-relay local extensions reusing `PromptRepo`, `thread_reply`,
  `resolve_prompt`/`resolve_wait`. **No Slack buttons.** Posture: test-first.
* **T8.6 — Recovery (recover_state).** Local-extension handler reusing the recovery
  repos. (`accept_diff` dropped per D1; `set_operational_mode` dropped per D3.)
  Posture: test-first.
* **T8.7 — Cutover: stop ACP mode depending on the HTTP endpoint.** Verify (ideally a
  live ACP tool round-trip) that no ACP path calls the HTTP MCP endpoint; make
  `main.rs` not start the HTTP transport for ACP tool access. Acceptance gate that
  unblocks `013.005.003-T`. Posture: integration/characterization.

## Dependency Graph

```
T8.1 (session/request_permission) ─▶ T8.3 (approval+policy)
T8.2 (scaffolding + text-reply)   ─▶ T8.3, T8.4, T8.5, T8.6
T8.3, T8.4, T8.5, T8.6            ─▶ T8.7 (cutover / acceptance gate)
T8.7                              ─▶ unblocks 013.005.003-T (delete src/mcp/*)
```

## Risks and Open Questions (decision points for review)

1. **`accept_diff` — RESOLVED (D1):** dropped. Copilot mobile surfaces live PR/diff
   info; not re-homed. (Confirm whether `src/diff/*` has any *other* consumer before
   it is later removed — it is used by the accept_diff tool today.)
2. **Slack buttons — RESOLVED (D2):** operator decisions are text replies over the
   ACP thread; the ACP tool layer uses no Slack blocks/buttons or oneshot-button
   callbacks. Button-based Slack handlers become dead for ACP and are cleaned up when
   `src/mcp/*` is removed (013.005.003-T) / in the single-Slack-app task (013.005.006-T).
3. **`set_operational_mode` — RESOLVED (D3):** dropped. ACP mode is remote-by-design
   in this scenario, so an agent-initiated routing-mode switch is unnecessary. The
   tool is not re-homed.
4. **How ACP agents are configured to reach tools today.** The catalogue confirms
   the ACP spawner sets only `INTERCOM_SESSION_ID`; the host CLI (e.g. `copilot
   --acp`) must currently be reaching the HTTP endpoint via its own MCP config. T8.7
   must confirm the exact wiring so cutover is verifiable (ideally a live round-trip
   with a real agent — an external checkpoint).
5. **Overlap with F.2 (013.001-F, done).** F.2 shipped ACP correctness fixes but
   `session/request_permission` remained absent per ADR-0016. T8.1 closes that gap;
   confirm no conflict with shipped F.2 code.
6. **Scope size.** This is effectively a small feature (7 subtasks). Recommend
   harvesting T8.1–T8.7 as subtasks under 013.005.008-T (or promoting it) before build.

## Related work (out of scope for this task)

**Operator CLI-command pass-through** (raised 2026-07-04): the operator needs to send
agent CLI/slash commands (`/mcp`, `/agent`, model selectors, etc.) remotely to the
ACP agent. This is a **new, additive ACP capability** — an operator→agent command
channel that builds on the same steering / `session/prompt` path this task
establishes, but is a **distinct feature**. It is **not** part of the tool re-home
and is **not a blocker** for the `src/mcp/*` deletion. Tracked separately as its own
backlog item for later planning.

## Runtime Verification and Closure

| Unit | Verification before "absorbed" | Closure artifact |
|---|---|---|
| T8.1 | Live/mocked `session/request_permission` → operator approve → JSON-RPC `result` round-trip | reader/writer contract note |
| T8.3–T8.6 | Each tool exercised via ACP stream (no HTTP) with shared-core assertions | ACP tool reference |
| T8.7 | Full ACP session drives all needed tools with the HTTP transport OFF; `013.005.003-T` unblocked | cutover verification + ADR update |
