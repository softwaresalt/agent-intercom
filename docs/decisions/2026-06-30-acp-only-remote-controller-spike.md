---
title: "How should agent-intercom converge to an ACP-only, reliable remote controller?"
type: spike
date: 2026-06-30
time_box: "4h"
conclusion: "proceed"
confidence: "medium"
linked_parent_work_item: "stash:F8974357"
promoted_to: ["queue"]
tags:
  - acp
  - architecture
  - reliability
  - remote-control
---

## Goal

Determine how to refactor `agent-intercom` from its current dual-protocol
(MCP-default + ACP-secondary) design into an **ACP-only remote controller**
that is robust and reliable enough to be the operator's primary method for
working with agents remotely.

Restated as an answerable question:

> What concrete convergence path, sequencing, and reliability work are required
> to make ACP the sole remote-control surface — and is that path viable given
> the current codebase?

## Success Criteria

- A grounded assessment of the current MCP/ACP split and how deeply MCP is
  coupled into the core.
- Identification of what "ACP-only" concretely removes, keeps, and reshapes.
- An enumerated set of reliability gaps that stand between today's ACP and a
  "go-to remote" bar, each backed by code/spec evidence.
- A recommendation (proceed / pivot / defer / abandon) with a phased approach
  and the open questions that a follow-on execution spike or impl-plan must
  answer.

## Scope Constraints

- **Read-only investigation.** No production code changed by this spike.
- Codebase-grounded. External protocol-spec conformance research (the public
  Agent Client Protocol standard) is flagged as a follow-up because web/doc
  research tooling was unavailable during this session.
- Does not produce implementation code; any convergence work is planned
  separately via `impl-plan`.

## Investigation Approach

1. Inventory the existing MCP and ACP surfaces (modules, drivers, transports,
   dependencies, CLI/config knobs).
2. Read the driver abstraction and mode-selection design (ADR-0014, ADR-0015,
   `mode.rs`, `main.rs`) to gauge MCP coupling depth.
3. Read the ACP stack (`src/acp/`, `AcpDriver`) to gauge maturity and existing
   reliability mechanisms.
4. Collect documented reliability/correctness/UX gaps from recent specs
   (005–008) and ADRs (0005 stall, 0011 reconnect).
5. Synthesize a convergence direction, sequencing, and open questions.

## Findings

### What Was Discovered

**ACP is already implemented and substantially functional — this is a
convergence/hardening effort, not a greenfield build.**

- Full ACP stream stack exists: `src/acp/{codec,handshake,reader,writer,spawner}.rs`
  (`src/acp/mod.rs`). NDJSON framing via `tokio_util::codec::LinesCodec` with a
  1 MiB per-line limit; LSP-style `initialize`/`initialized` handshake; a
  spawner that launches headless agent child processes with stdio capture.
- A protocol-agnostic `AgentDriver` trait (`src/driver/mod.rs`, ADR-0014)
  already decouples Slack handlers/orchestrator from transport. Two impls:
  `McpDriver` and `AcpDriver` (`src/driver/{mcp_driver,acp_driver}.rs`).
- `AcpDriver` routes operator actions to the correct agent stream via
  per-session `mpsc` writer channels, tracks agent-assigned session IDs, and
  maintains per-session monotonic outbound sequence counters (ES-008) plus
  pending-clearance / pending-prompt maps.
- `AppError::Acp(String)` is a first-class error variant (`src/errors.rs:38`).
  (Note: the workspace instruction file's `AppError` list omits `Acp` — minor
  doc drift.)

**MCP is still the default and the "primary" identity of the binary.**

- `ServerMode::Mcp` is the `#[default]` (`src/mode.rs:15-21`); the `--mode`
  flag defaults to `mcp` (`src/main.rs:87-88`); `--transport` defaults to
  `both` (stdio + HTTP/SSE).
- The binary still describes itself as an "MCP remote agent server"
  (`src/main.rs:3-6`, `Cargo.toml` description).
- MCP surface to retire/reshape under ACP-only: the `rmcp = 0.13` dependency
  and its `transport-streamable-http-server` / `transport-io` features
  (`Cargo.toml`), `src/mcp/{handler,sse,transport}.rs`, `McpDriver`, and the
  MCP-oriented `AppState` primitives (`PendingApprovals`, `PendingPrompts`,
  `PendingWaits` oneshot maps live in `mcp::handler`).
- ADR-0015 chose **separate Slack apps for MCP and ACP** (Option A) precisely
  because both protocols coexist. Going ACP-only removes that reason and lets
  the dual-app / mode-suffixed-credential / IPC-pipe-suffix machinery collapse
  to a single app — a real simplification opportunity.

**Reliability mechanisms already exist but are partial and MCP-shaped.**

- Slack reconnect re-post of pending interactive messages on Socket Mode
  `hello` (ADR-0011) — but it drives the MCP in-memory oneshot maps.
- Stall detection architecture (ADR-0005, `orchestrator/stall_consumer.rs`)
  with `AgentEvent::StreamActivity` resetting the inactivity timer (S063).
- Child-process monitoring (`orchestrator/child_monitor`) and
  `AgentEvent::SessionTerminated`.

**Documented, still-open reliability/correctness/UX gaps (the "not yet go-to"
bar).** Spec 007 (`docs/product-specs/007-acp-correctness-mobile/`, status Draft) targets
exactly the epic's reliability theme:

- US1 — steering messages can be silently lost; need mark-consumed-only-on-
  successful-delivery + retry queue.
- US2 — ACP session capacity counting is inaccurate (initializing sessions and
  non-ACP connections mis-counted).
- US3 — live workspace routing for new sessions (noted already fixed, F-08).
- US5 — protocol hygiene + prompt correlation-ID uniqueness, including across
  server restarts.
- Spec 008 (`docs/product-specs/008-slack-ui-testing/`) documents a **modal-in-thread
  silent failure**: `views.open` reports success but the modal never renders in
  Slack (notably mobile/threads). The thread-reply fallback only triggers on
  API failure, not on silent client-side swallowing — a direct hit on the
  "operate from my phone" remote-UX reliability goal.

**Naming ambiguity is itself a finding.** The code/docs inconsistently call ACP
both "Agent Client Protocol" (`src/acp/mod.rs`, `errors.rs`) and "Agent
Communication Protocol" (`mode.rs:19`, ADR-0015). The wire dialect is bespoke
NDJSON with a mix of standard-looking (`session/new`, `session/prompt`,
`session/interrupt`) and custom (`clearance/*`, `prompt/forward`) methods. The
epic explicitly names **Agent Client Protocol**. Whether to align the wire
format with the public ACP standard (JSON-RPC 2.0; `session/request_permission`,
`session/update`, `session/cancel`) or keep the bespoke dialect is the single
highest-leverage open decision, because standard conformance is what lets real
ACP-capable agents (e.g., Claude Code, Gemini CLI, Copilot CLI) be driven as
first-class clients.

### What Was Tried and Failed

- Attempted external verification of the public Agent Client Protocol wire
  spec to compare against the bespoke dialect. Web/doc research tools
  (tavily, context7, web_search) were unavailable this session, so
  standard-conformance analysis is deferred rather than completed.

### Remaining Unknowns

1. **Standard vs bespoke ACP.** Align to the public Agent Client Protocol, or
   formalize the existing dialect? (Needs external spec review + a test against
   at least one real ACP agent.)
2. **Reconnection/resume semantics for a controller-of-child-processes.** For
   stdio-attached child agents, "reconnect" means crash-detect → respawn →
   session resume. What state must persist to resume mid-task (correlation IDs,
   pending clearances, steering queue, agent session ID)?
3. **AppState reshaping.** How much of the MCP-oriented `AppState` (pending
   maps in `mcp::handler`) must move to a protocol-neutral home once `McpDriver`
   is removed?
4. **Keep or collapse `AgentDriver`.** With one protocol, is the trait worth
   keeping for future extensibility, or does it become dead abstraction?
5. **Migration/compat.** Is a deprecation window for MCP required, or is a clean
   cut acceptable for this private binary?

## Recommendation

**Conclusion**: proceed
**Confidence**: medium

The ACP-only direction is viable and the codebase is well-positioned: the
`AgentDriver` abstraction, a mature `src/acp/` stack, and stall/reconnect/child-
monitor scaffolding already exist. Convergence is primarily *removal +
hardening + one protocol-conformance decision*, not new architecture.

Recommended phased sequencing (to be turned into an `impl-plan`):

1. **Decide the wire protocol (blocking).** Run a focused execution spike to
   compare the bespoke dialect against the public Agent Client Protocol and
   test-drive one real ACP agent. Output: "conform" or "formalize bespoke."
   Everything downstream depends on this.
2. **Land the open correctness/reliability fixes** already specified in
   spec 007 (steering retry, capacity counting, correlation-ID uniqueness) and
   the spec 008 mobile modal-fallback fix. These are the concrete "reliable
   enough to be my go-to" blockers.
3. **Reliability hardening for controller mode:** agent crash-detect →
   respawn → session resume; durable steering queue; correlation-ID and
   pending-state persistence across restarts.
4. **Make ACP the default** (`ServerMode` default flip, binary identity/docs).
5. **Retire MCP:** remove `McpDriver`, `src/mcp/*`, the `rmcp` dependency and
   HTTP/SSE transport, and collapse ADR-0015's dual-Slack-app / mode-suffix /
   IPC-pipe-suffix machinery to a single app. Supersede ADR-0014/0015 with a
   new ADR recording the ACP-only decision.
6. **Fold in the `.intercom` numbered-queue slash command** (stash `EE76674F`),
   which is already scoped in the ACP model and belongs under this epic.

Treat steps 1–3 as the reliability core; steps 4–5 are the "only" in
"ACP-only." Keep MCP shippable until step 4 to de-risk.

## Next Steps

- Create a backlog spike work item linked to epic `F8974357` (done — see
  References).
- When ready to execute, run the protocol-conformance execution spike (open
  question 1) first; it gates the impl-plan.
- Harvest epic `F8974357` and task `EE76674F` together so the numbered-queue
  command is planned as part of this convergence.

## References

- `Cargo.toml` — `rmcp` dep + features; binary description.
- `src/main.rs:3-6,40-89` — MCP default identity, `--mode`/`--transport` flags.
- `src/mode.rs` — `ServerMode { Mcp (default), Acp }`.
- `src/driver/mod.rs` — `AgentDriver` trait + `AgentEvent`.
- `src/driver/acp_driver.rs` — per-session routing, seq counters (ES-008).
- `src/acp/mod.rs` — NDJSON codec, handshake, reader/writer, spawner.
- `src/errors.rs:37-38` — `AppError::Acp`.
- `docs/adrs/0014-agent-driver-trait-protocol-abstraction.md`
- `docs/adrs/0015-separate-slack-apps-for-mcp-and-acp.md`
- `docs/adrs/0011-reconnect-repost-pending-messages.md`
- `docs/adrs/0005-stall-detector-architecture.md`
- `docs/product-specs/005-intercom-acp-server/`, `docs/product-specs/006-acp-event-wiring/`,
  `docs/product-specs/007-acp-correctness-mobile/`, `docs/product-specs/008-slack-ui-testing/`
- Stash: `F8974357` (epic), `EE76674F` (numbered-queue slash command).
