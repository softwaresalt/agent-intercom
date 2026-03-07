# Implementation Plan: ACP Event Handler Wiring

**Branch**: `006-acp-event-wiring` | **Date**: 2026-03-07 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `specs/006-acp-event-wiring/spec.md`

## Summary

Wire the two no-op ACP event handlers (`ClearanceRequested`, `PromptForwarded`) in the event consumer loop to register pending requests with `AcpDriver`, persist records via `ApprovalRepo`/`PromptRepo`, and post interactive Slack messages with approval/prompt buttons. Extract shared Slack block-building logic from MCP tool handlers into `slack/blocks.rs` so both MCP and ACP codepaths use identical rendering.

## Technical Context

**Language/Version**: Rust stable, edition 2021
**Primary Dependencies**: rmcp 0.13, axum 0.8, slack-morphism 2.17, tokio 1.37, sqlx 0.8, sha2 0.10
**Storage**: SQLite via sqlx (bundled, file-based prod, in-memory tests)
**Testing**: cargo test (unit/, contract/, integration/)
**Target Platform**: Windows/macOS/Linux server
**Project Type**: Single workspace, two binaries (agent-intercom, agent-intercom-ctl)
**Performance Goals**: Event-to-Slack-post latency < 2 seconds under normal conditions
**Constraints**: No new dependencies; Clippy pedantic + zero warnings; no unsafe; no unwrap/expect
**Scale/Scope**: Handles concurrent ACP sessions (configurable, default max 5)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Safety-First Rust | ✅ Pass | All fallible ops use Result/AppError. No unsafe. Clippy pedantic. |
| II. MCP Protocol Fidelity | N/A | Feature wires ACP event handlers, not MCP tools. Slack output reuses MCP block builders for visual consistency. |
| III. Test-First Development | ✅ Pass | Tests written before implementation for each handler. Contract tier extended to ACP event handler input/output contracts (analogous to MCP tool contracts). |
| IV. Security Boundary Enforcement | ✅ Pass | File hash computation uses existing `path_safety.rs` for path validation. FR-013 mandates MUST-level path rejection. |
| V. Structured Observability | ✅ Pass | FR-014 mandates info-level tracing spans for each handler invocation (session_id, event type, request_id). Error paths emit warn! per D3. |
| VI. Single-Binary Simplicity | ✅ Pass | No new dependencies. Shared logic extracted within existing modules. |
| VII. CLI Workspace Containment | N/A | Server-side feature, not CLI. |
| VIII. Destructive Terminal Command Approval | N/A | Not about terminal commands. |

## Project Structure

### Documentation (this feature)

```text
specs/006-acp-event-wiring/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
└── tasks.md             # Phase 2 output (from /speckit.tasks)
```

### Source Code (files changed)

```text
src/
├── main.rs                         # Wire ClearanceRequested + PromptForwarded handlers
├── slack/
│   └── blocks.rs                   # Extract build_approval_blocks + build_prompt_blocks (pub(crate))
└── mcp/
    └── tools/
        ├── ask_approval.rs         # Replace private builder with import from slack/blocks
        └── forward_prompt.rs       # Replace private builder with import from slack/blocks

tests/
├── unit/
│   └── acp_event_wiring.rs         # Unit tests for event-to-record mapping
├── contract/
│   └── acp_event_contract.rs       # Contract tests for event handler outputs
└── integration/
    └── acp_event_integration.rs    # Integration tests for full event → Slack flow
```

**Structure Decision**: Changes stay within the existing single-project layout. No new modules — only function visibility changes and new handler logic in existing files.

## Design

### Architecture: Event Handler Flow

```text
ACP Agent Process
    │
    ▼ (NDJSON stream)
AcpReader (src/acp/reader.rs)
    │ parses AgentEvent
    ▼
mpsc::Sender<AgentEvent>
    │
    ▼
run_acp_event_consumer (src/main.rs)
    │
    ├── ClearanceRequested ──┬── AcpDriver::register_clearance()
    │                        ├── ApprovalRepo::create()
    │                        ├── blocks::build_approval_blocks() + approval_buttons()
    │                        ├── SlackService::post_message_direct() → capture ts
    │                        └── SessionRepo::set_thread_ts() (if first message)
    │
    └── PromptForwarded ─────┬── AcpDriver::register_prompt_request()
                             ├── PromptRepo::create()
                             ├── blocks::build_prompt_blocks()
                             └── SlackService::enqueue() (threaded)

Operator clicks Accept/Reject/Continue/Refine/Stop in Slack
    │
    ▼
Slack Button Handler (src/slack/handlers/) — already polymorphic
    │ calls state.driver.resolve_clearance() / resolve_prompt()
    ▼
AcpDriver sends response to agent stream writer
```

### Key Design Decisions

**D1: Shared Block Builders**

Extract `build_approval_blocks()` and `build_prompt_blocks()` (plus helpers: `prompt_type_label`, `prompt_type_icon`, `truncate_text`, `INLINE_DIFF_THRESHOLD`) from MCP tool files to `src/slack/blocks.rs` with `pub(crate)` visibility. Both MCP tools and ACP handlers import from there.

*Rationale*: Single source of truth for Slack message formatting. No duplication, no divergence risk.
*Alternative rejected*: Inline duplication in `main.rs` — creates maintenance burden and risks visual inconsistency.

**D2: Direct Post for Clearance, Conditional for Prompt**

Clearance requests always use `post_message_direct()` (captures Slack `ts` for approval record and thread anchoring). Prompts use `post_message_direct()` when creating the session's first Slack thread (`thread_ts=None`), and `enqueue()` when a thread already exists.

*Rationale*: Clearance requests must capture timestamp for thread management (FR-007, FR-008). Prompts that are the first event for a session must also capture `ts` to establish the thread anchor. Once a thread exists, prompts can use the async queue since `ts` capture is not needed.
*Alternative rejected*: Always direct post for prompts — unnecessary overhead when thread already exists.

**D3: Error Handling — Log and Continue**

ACP event handlers run in a background tokio task; errors cannot be returned to caller. All errors emit `warn!` tracing spans and the handler skips to the next event. No panic, no unwrap.

*Rationale*: Event consumer must stay alive to process future events. One bad event must not take down the consumer.

**D4: No Oneshot Channels for ACP**

Unlike MCP tools (which register oneshots in `state.pending_approvals` and block), ACP handlers only register with `AcpDriver::register_clearance()`/`register_prompt_request()`. The Slack button handler resolves via `state.driver.resolve_clearance()` which dispatches to `AcpDriver` (polymorphic), which writes the response back to the agent's stream writer. No blocking.

*Rationale*: ACP agents don't block on a tool call — they continue streaming and the response arrives as a separate message on their NDJSON stream.

## Complexity Tracking

| Item | Principle | Description | Resolution |
|---|---|---|---|
| D2/US3 conditional posting | II, VII | D2 defaults to enqueue for prompts, but T018 overrides to direct post when no thread exists. Two posting paths for the same event type. | Documented in D2 rationale. FR-007 expanded to cover both event types. |
| SC-003 vs D3 log-and-continue | III | SC-003 originally claimed 100% persistence. D3 allows skipping DB on failure. Adversarial review (UF-02) identified the conflict. | SC-003 amended to "attempted persistence." Handler skips driver registration if DB fails to prevent unaudited state. |
| Timeout deferral | N/A | US1.4 and US2.5 define timeout acceptance criteria but FR-015 defers implementation. | Documented in Assumptions. Timeout mechanism is a separate feature. |
| Secret redaction | IV | Diff content posted to Slack without sanitization (UF-06). Affects both MCP and ACP paths. | Documented in Threat Model Note. Recommended as dedicated security feature. |
