---
title: "ACP-Only Remote Controller — Convergence + Reliability Implementation Plan"
type: plan
date: 2026-07-01
source: docs/decisions/2026-06-30-acp-only-remote-controller-spike.md
linked_stash: ["F8974357", "EE76674F"]
related_backlog: ["012-F"]
status: reviewed
---

## Problem Frame

`agent-intercom` today ships as a **dual-protocol** binary: MCP is the `#[default]`
(`src/mode.rs:15-21`, `--mode` defaults to `mcp` in `src/main.rs`, `--transport`
defaults to `both`), and ACP is the secondary streaming mode. The epic
(`F8974357`) asks to converge on **ACP as the sole remote-control surface** and
harden it to "go-to remote" reliability. The accepted spike
(`docs/decisions/2026-06-30-acp-only-remote-controller-spike.md`, conclusion:
*proceed*, confidence: *medium*) established that this is a **convergence +
hardening + one protocol-conformance decision**, not a greenfield build:

* A protocol-agnostic `AgentDriver` trait already decouples Slack/orchestrator
  from transport (`src/driver/mod.rs`, ADR-0014) with `McpDriver` and `AcpDriver`
  impls (`src/driver/{mcp_driver,acp_driver}.rs`).
* A mature ACP stream stack exists: `src/acp/{codec,handshake,reader,writer,spawner}.rs`
  (NDJSON via `tokio_util::codec::LinesCodec`, LSP-style handshake, child-process
  spawner).
* Reliability scaffolding exists but is partial and MCP-shaped: stall detection
  (`src/orchestrator/stall_consumer.rs`, ADR-0005), reconnect re-post (ADR-0011),
  child monitoring (`src/orchestrator/child_monitor`).
* MCP surface to retire: `rmcp = 0.13` + `transport-streamable-http-server` /
  `transport-io` features (`Cargo.toml:23`), `src/mcp/{handler,sse,transport}.rs`
  (heaviest coupling in `src/mcp/sse.rs`, 49 `rmcp` refs), `McpDriver`, and the
  MCP-oriented `AppState` pending maps in `mcp::handler`.

`EE76674F` (numbered-queue slash command) is an ACP-model operator-UX feature; the
spike (§6) folds it under this epic. The Slack slash router (`src/slack/commands.rs`)
already dispatches `/acom` (MCP) and `/arc` (ACP) prefixes and gates ACP-only
subcommands behind `ServerMode::Acp`, so the numbered queue lands as new `/arc`
subcommands over a new `.intercom` numbered-queue persistence surface (distinct
from the existing steering queue in `src/persistence/steering_repo.rs`).

## Requirements Trace

| Source requirement | Implementation units |
|---|---|
| Converge on ACP as sole remote-control surface (F8974357) | Phase F.5 (default flip + MCP retirement) |
| Harden reliability: reconnection, stall/session recovery, error handling | Phases F.2 (correctness) + F.3 (controller hardening) |
| Improve remote-work UX for steering agents | Phase F.2 (mobile modal fallback) + F.4 (numbered queue) |
| Wire-protocol conformance decision (spike open Q1, blocking) | Phase F.1 |
| `.intercom` numbered-queue slash command (EE76674F) | Phase F.4 |
| Resolve rmcp/RUSTSEC-2026-0189 relationship (cross-item) | Phase F.5 removal task subsumes 012-F |

## Implementation Units

Units are grouped into five phase-features under the covering feature. Each unit
targets < 3 files, < 5 functions, < 4 test scenarios (2-hour rule) and a single
width domain.

### F.1 — ACP wire-protocol conformance decision (BLOCKING GATE)

Spike open question 1 is the highest-leverage decision and gates the shape of all
downstream ACP work. This phase is investigative (spike posture) + one ADR.

* **F.1-T1** Map the bespoke NDJSON dialect (`session/new`, `session/prompt`,
  `session/interrupt`, custom `clearance/*`, `prompt/forward`) against the public
  Agent Client Protocol (JSON-RPC 2.0: `session/request_permission`,
  `session/update`, `session/cancel`). Files: read-only `src/acp/*`. Output: gap
  table. Posture: spike.
* **F.1-T2** Test-drive one real ACP-capable agent (Claude Code or Gemini CLI)
  against the current stream; capture handshake/method incompatibilities. Posture:
  spike (read-only investigation; no production code).
* **F.1-T3** Author an ADR recording "conform to ACP standard" vs "formalize
  bespoke dialect" with migration implications. Files: `docs/adrs/`. Posture: docs.

### F.2 — ACP correctness fixes (spec 007 + 008)

Documented, still-open reliability gaps independent of the F.1 wire decision.

* **F.2-T1** Steering delivery: mark-consumed **only** on successful delivery +
  retry queue (spec 007 US1). Files: `src/acp/reader.rs`,
  `src/persistence/steering_repo.rs`. Posture: test-first.
* **F.2-T2** Fix ACP session capacity counting (initializing sessions + non-ACP
  connections mis-counted) (US2). Posture: test-first.
* **F.2-T3** Prompt correlation-ID uniqueness across server restarts (US5).
  Posture: test-first.
* **F.2-T4** Mobile modal-in-thread silent-failure → trigger thread-reply fallback
  on silent client-side swallow, not only on API failure (spec 008). Files:
  `src/slack/handlers/modal.rs`, `src/slack/handlers/thread_reply.rs`. Posture:
  test-first.

### F.3 — Controller-mode reliability hardening

Spike open questions 2 & controller-of-child-processes hardening (Phase 3 of the
spike sequencing).

* **F.3-T1** Agent crash-detect → respawn → session resume. Files:
  `src/orchestrator/child_monitor`, `src/orchestrator/spawner`. Posture:
  test-first.
* **F.3-T2** Durable steering queue persistence across restarts. Posture:
  migration-first (schema for durable queue state).
* **F.3-T3** Correlation-ID + pending-state persistence across restarts (what
  state must survive to resume mid-task). Posture: test-first.
* **F.3-T4** Define the reconnection/resume-state **contract** for stdio-attached
  child agents (which agent session ID must be re-bound on respawn) and wire T2/T3
  persistence into the resume path. Does **not** re-implement the steering-queue or
  pending-state persistence owned by T2/T3 — it consumes them. Posture:
  characterization-first.

### F.4 — `.intercom` numbered-queue slash command (EE76674F)

* **F.4-T1** `.intercom` numbered-queue persistence model + repo: add/list numbered
  items in the `.intercom` folder. Files: new `src/persistence/*` module. Posture:
  test-first.
* **F.4-T2** `/arc queue add` + `/arc queue list` subcommands in the slash router.
  Files: `src/slack/commands.rs`. Posture: code (with unit tests in acceptance).
* **F.4-T3** `/arc queue replace <n>` edit command. Files: `src/slack/commands.rs`.
  Posture: code.
* **F.4-T4** `/arc queue transfer <n>` → hand off a numbered item to **backlogit**
  (the installed backlog tool of record) via a thin single-call transfer seam. No
  multi-tool plugin framework (YAGNI — backlog-md abstraction deferred until a
  second tool is actually required). Include ≥1 unit test scenario. Files:
  `src/slack/commands.rs` + minimal transfer helper. Posture: code.

### F.5 — Make ACP default & retire MCP (the "only")

Irreversible removal work; keep MCP shippable until F.1–F.3 land (spike guidance).
**Intra-phase ordering (post plan-review fix):** extract `AppState` FIRST (T2),
decide `AgentDriver` fate (T3), THEN delete `src/mcp/*` (T4) and the `rmcp` dep
(T5). Deleting `src/mcp/*` before `AppState` moves out would not compile —
`AppState` and its core type aliases physically live in `src/mcp/handler.rs` and
are imported app-wide (`main.rs:23-25`, `src/slack/commands.rs:28`).

* **F.5-T1** Flip `ServerMode` default to `Acp`; update `--mode`/`--transport`
  defaults and binary identity/description. Files: `src/mode.rs`, `src/main.rs`,
  `Cargo.toml`. Posture: config/code.
* **F.5-T2** Extract `AppState` + its core type aliases (`PendingApprovals`,
  `PendingPrompts`, `PendingWaits`, `StallDetectors`) out of `src/mcp/handler.rs`
  into a protocol-neutral module (e.g. `src/state/mod.rs`); repoint imports in
  `src/main.rs` and `src/slack/commands.rs`. **Must precede T4.** Posture: code.
* **F.5-T3** Decide `AgentDriver` keep-vs-collapse (spike open Q3/Q4) in a short
  ADR/decision note; if collapse is chosen, adjust the `driver: Arc<dyn AgentDriver>`
  field/type at call sites. Isolated so its blast radius is bounded. Posture: docs
  (+ minimal code if collapse chosen).
* **F.5-T4** Remove `McpDriver` and `src/mcp/*` modules; retire the now-dead
  `ServerMode::Mcp` variant and the MCP-only `Transport{Stdio,Sse,Both}` enum /
  branches in `src/main.rs`. Depends on T2 (and T3 if collapse chosen). Posture:
  code.
* **F.5-T5** Remove the `rmcp` dependency + HTTP/SSE transport, retire the orphan
  `rmcp-upgrade` feature gate (`Cargo.toml:97-99`), and delete the
  `RUSTSEC-2026-0189` ignore from `.cargo/audit.toml`; `cargo audit` must pass
  without it. **This subsumes 012-F** (removal instead of upgrade — see Decisions).
  Depends on T4. Files: `Cargo.toml`, `.cargo/audit.toml`. Posture: config/code.
* **F.5-T6** Collapse ADR-0015 dual-Slack-app / mode-suffix / IPC-pipe-suffix
  machinery to a single app. Posture: code/config.
* **F.5-T7** Supersede ADR-0014/0015 with a new ACP-only ADR. Files: `docs/adrs/`.
  Posture: docs.

> **Cross-phase note (plan-review P2):** F.4-T2..T4 and F.5-T2 both edit
> `src/slack/commands.rs`. Sequence F.5-T2's `AppState` import repoint after F.4's
> `/arc queue` subcommands land, or accept a known merge on that file. Ship should
> not run F.4 and F.5-T2 truly concurrently on `commands.rs`.

## Dependency Graph

```
Covering Feature (epic F8974357)
  F.1 (protocol decision, BLOCKING) ──▶ informs F.2, F.3, F.5 wire-format work
  F.2 (correctness)      ──▶ F.5 (retire only after reliability core lands)
  F.3 (hardening)        ──▶ F.5
  F.4 (numbered queue)   ── independent of F.1 wire decision; can proceed in parallel
  F.5 (default + retire) ── depends on F.1..F.3; F.5-T5 subsumes 012-F
```

* F.1 is the blocking gate (`blocks` F.5; `informs` F.2/F.3).
* F.5 depends on F.2 and F.3 (keep MCP shippable until the reliability core lands).
* F.4 is independent and may run in parallel after F.1's `.intercom`/`/arc`
  conventions are confirmed.
* 012-F is **superseded** by F.5-T5.

## Decisions and Rationale

1. **Model the epic as a covering feature + 5 phase-features + tasks.** backlogit
   has no `epic` WIT type (types: deliberation/feature/shipment/review/task/bug/
   subtask). Feature is L1, task L2, subtask L3. The epic therefore harvests as a
   covering feature with phase-features as children and 2-hour tasks beneath.
2. **F.1 protocol decision gates before deep F.2/F.5 wire work.** The spike names
   this the single highest-leverage open decision; conforming to the public ACP
   standard is what lets real ACP agents be driven as first-class clients.
3. **012-F (rmcp 0.13→1.4 upgrade) is subsumed by F.5-T5 removal, not executed as
   an upgrade.** RUSTSEC-2026-0189 lives in the `transport-streamable-http-server`
   feature (`src/mcp/sse.rs`). ACP-only removes that transport and the `rmcp`
   dependency entirely, eliminating the vulnerable surface — making the breaking
   upgrade wasted effort. The advisory is already risk-accepted and ignored in
   `.cargo/audit.toml` (pre-release, not publicly deployed), so the transition
   window carries no new exposure. **Recommendation to operator:** hold 012-F; do
   not start the rmcp upgrade; when F.5-T3 lands, close 012-F as *won't-fix —
   superseded by removal*. Keep 012-F as a fallback only if the ACP-only cut slips
   materially. Stage records the `supersedes` link + a comment on 012-F and does
   **not** modify 012-F's scope unilaterally.
4. **Keep MCP shippable until F.4/default-flip.** Sequencing removal last de-risks
   the convergence (spike guidance).

## Risks and Caveats

* **Irreversible removal (F.5).** Deleting `src/mcp/*`, `McpDriver`, and the `rmcp`
  dep is a one-way door. Mitigation: gate F.5 behind F.1–F.3 completion; retain
  git history; land as a discrete, reviewable slice.
* **Security-advisory window.** RUSTSEC-2026-0189 stays ignored until F.5-T3.
  Mitigation: pre-release, not publicly deployed; ignore already risk-accepted;
  do not add new public HTTP exposure during the window.
* **Protocol-decision rework risk.** If F.1 chooses "conform," some F.2 fixes may
  need reshaping. Mitigation: F.2 targets documented correctness bugs that exist
  regardless of wire format; sequence F.1 first.
* **Scope size.** This is a genuinely large epic (~22 tasks). Mitigation:
  dependency edges enforce phase order; Ship may claim in waves (recommend Ship
  start with F.1, then F.4 in parallel, then F.2/F.3, then F.5).

## Plan Hardening Signals (REQUIRED)

* Public API / schema / contract change — **PRESENT**: wire-protocol conformance
  decision (F.1) and `AppState` reshaping (F.5-T4) change agent-facing contracts;
  new `.intercom` numbered-queue schema (F.4-T1, F.3-T2).
* Security / auth / compliance-sensitive behavior — **PRESENT**: removes the
  RUSTSEC-2026-0189-affected transport (F.5-T5); advisory ignore removal.
* Migration / backfill / destructive / irreversible step — **PRESENT**: removal of
  `rmcp`, `src/mcp/*`, `McpDriver` (F.5); durable-queue schema migration (F.3-T2).
* External integration / operator checkpoint / external dependency — **PRESENT**:
  test-driving a real external ACP agent (F.1-T2); backlog-tool transfer adapter
  (F.4-T4); operator go/no-go on 012-F disposition.
* High runtime / rollout / rollback risk — **PRESENT**: default-mode flip (F.5-T1)
  changes the binary's primary runtime surface.

**Requires plan hardening: yes**

## Runtime Verification and Closure

| Unit(s) | Runtime surface | Verification before "absorbed" | Closure artifact |
|---|---|---|---|
| F.2-T1..T4 | ACP stream + Slack modals | Steering delivered exactly-once under induced disconnect; modal fallback fires on silent swallow (mobile/thread) | Reliability checklist appended to spec 007/008 |
| F.3-T1..T4 | Child-process controller | Kill agent mid-task → auto respawn + session resume with pending state intact | Recovery runbook + rollback trigger |
| F.4-T1..T4 | `/arc` slash commands | `add`/`list`/`replace`/`transfer` round-trip against a live `.intercom` queue | Command reference doc |
| F.5-T1 | `--mode` default | ACP starts by default; MCP reachable only via explicit opt-in during window | Migration note in README/CHANGELOG |
| F.5-T5 | HTTP transport removal | `cargo audit` passes with the ignore removed; no `rmcp` in `Cargo.lock` | Security closure: advisory retired |

## Plan Hardening

(Authored per P-006 because `Requires plan hardening: yes`.)

### Risky / irreversible actions

| Action | Risk class | Guardrail | Rollback |
|---|---|---|---|
| Remove `rmcp` dep + `src/mcp/*` + `McpDriver` (F.5-T4, F.5-T5) | Destructive, irreversible | Gate behind F.1–F.3 done; extract `AppState` (F.5-T2) first; single reviewable PR; MCP kept shippable until F.5 | Revert the removal commit; `rmcp` pin restorable from `Cargo.lock` history |
| Remove `RUSTSEC-2026-0189` ignore (F.5-T5) | Security-gate change | Only after transport removal verified; `cargo audit` must pass without ignore | Re-add ignore entry if removal reverted |
| Flip `ServerMode` default to `Acp` (F.5-T1) | Runtime default change | Land after reliability core (F.2/F.3); document opt-back-in flag | Flip default back to `Mcp` |
| Durable-queue schema migration (F.3-T2) | Data/migration | Additive migration; forward-compatible read path | Migration down-script; additive columns are safe to ignore |
| Test-drive external ACP agent (F.1-T2) | External integration, read-only | Investigation only; no production wiring in F.1 | N/A (no production change) |

### Operator checkpoints

* **CP-1 (before F.5 execution):** Operator confirms F.1 ADR outcome and that
  reliability core (F.2/F.3) is accepted.
* **CP-2 (012-F disposition):** Operator decides close-as-superseded vs keep-as-
  fallback for 012-F once F.5-T5 lands.

**Hardening conclusion:** All PRESENT signals have a guardrail + rollback path and
two explicit operator checkpoints. Plan is hardened and ready for the review gate.

<!-- plan-review-attempt: 2 -->

## Plan Review

Multi-persona plan-review gate (personas: Scope Boundary Auditor, Architecture
Strategist). Plan hardening was **required** (`Requires plan hardening: yes`) and
the `## Plan Hardening` section is present and satisfied.

### Cycle 1 — FAIL (P1 findings)

* **[P1] Arch — F.5 intra-phase ordering backwards.** `AppState` + core type
  aliases live in `src/mcp/handler.rs` and are imported app-wide (`main.rs:23-25`,
  `slack/commands.rs:28`); deleting `src/mcp/*` before extracting `AppState` will
  not compile.
* **[P1] Arch — F.5-T4 under-scoped.** `AppState` extraction is a cross-cutting
  refactor and was bundled with the `AgentDriver` keep/collapse decision; must be
  split.
* **[P1] Scope — F.4-T4 multi-tool transfer abstraction (YAGNI).** backlogit is the
  installed tool of record; pluggable backlog-md support is a single-use
  abstraction and exceeds EE76674F's framing.

### Revisions applied

* F.5 re-ordered and split: **F.5-T2** extract `AppState` to a protocol-neutral
  module (must precede removal); **F.5-T3** isolate the `AgentDriver` keep/collapse
  decision; **F.5-T4** remove `src/mcp/*`/`McpDriver` + retire dead
  `ServerMode::Mcp`/`Transport` branches; **F.5-T5** remove `rmcp` + `rmcp-upgrade`
  gate + RUSTSEC ignore (subsumes 012-F). Dependency ordering T2→T3→T4→T5 recorded.
* F.4-T4 narrowed to a thin backlogit-only transfer seam (no plugin framework).
* F.3-T4 narrowed to a resume-state contract that consumes T2/T3 rather than
  re-persisting them (resolves P2 overlap).
* Cross-phase `commands.rs` collision between F.4 and F.5-T2 called out with a
  sequencing instruction for Ship (P2).

### Residual advisory findings (P2/P3 — not blocking)

* **[P2] Arch** — `AppState` cohesion boundary is larger than three pending maps
  (also owns ACP fields, stall detectors, policy, audit); F.5-T2 should move the
  whole struct/module home. *Carried as an implementation note for Ship.*
* **[P3] Arch** — F.3-T2 and F.4-T1 both add durable-queue schema under
  `src/persistence`; warrant a shared persistence-boundary review.
* **[P3] Scope** — F.4-T3/T4 posture is `code`; each now carries ≥1 unit-test
  scenario in acceptance to match the test-first siblings.

### Cycle 2 — Gate decision: **PASS**

All P1 findings resolved via revision; no P0/P1 remain. Residual items are P2/P3
advisory and recorded as implementation notes carried into the backlog. The
012-F "subsumed by removal, not upgraded" determination was independently
confirmed correct by both personas. Plan is cleared for harvest.

## Operator Decisions (2026-07-02)

Recorded by the orchestrator after presenting the Stage handoff. These resolve
the three checkpoints raised at the end of staging.

1. **012-F disposition — CLOSE (won't-fix, superseded).** The rmcp 0.13→1.4
   security upgrade (RUSTSEC-2026-0189) is **not** to be performed. It is
   superseded by removal: F.5 task `013.005.005-T` deletes `rmcp` and the MCP
   Streamable HTTP transport (`src/mcp/*`) entirely, eliminating the vulnerable
   surface. `012-F` is archived; links `013.005.005-T --supersedes--> 012-F` and
   `013-F --related_to--> 012-F` are recorded. The `RUSTSEC-2026-0189` ignore in
   `.cargo/audit.toml` stays (risk-accepted; pre-release, not publicly deployed)
   until `013.005.005-T` lands, then is removed.

2. **MCP removal — APPROVED.** Rationale: "GHCP mobile now effectively does what
   MCP mode did — redundant functionality." Phase F.5 (`013.005-F`) is authorized
   to make ACP the default and remove the MCP surface. **Guardrail (CP-1):** F.5
   is the irreversible phase — it must not execute until the F.1 conformance ADR
   (`013.004-F`) is merged and F.2 (`013.001-F`) / F.3 (`013.003-F`) are accepted.

3. **Execution cadence — SHIP IN WAVES.** Order: F.1 gate → F.4 in parallel →
   F.2/F.3 → F.5 last. Note: F.1 is a **spike/ADR gate** (read-only investigation
   + one ADR), not a build wave; its middle task `013.004.002-T` (test-drive a
   real ACP agent) requires a live external ACP agent and operator involvement.
