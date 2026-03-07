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
| II. MCP Protocol Fidelity | ✅ Pass | ACP events mirror MCP tool behavior; Slack handlers dispatch via polymorphic `state.driver`. |
| III. Test-First Development | ✅ Pass | Tests written before implementation for each handler. |
| IV. Security Boundary Enforcement | ✅ Pass | File hash computation uses existing `path_safety.rs` for path validation. |
| V. Structured Observability | ✅ Pass | Event handlers emit tracing spans matching existing MCP tool patterns. |
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

**D2: Direct Post for Clearance, Queue for Prompt**

Mirror MCP behavior: `post_message_direct()` for clearance requests (captures Slack `ts` for thread anchoring), `enqueue()` for prompts (async delivery sufficient since thread already established).

*Rationale*: Consistency with MCP. Clearance requests must capture timestamp for thread management (FR-007, FR-008).

**D3: Error Handling — Log and Continue**

ACP event handlers run in a background tokio task; errors cannot be returned to caller. All errors emit `warn!` tracing spans and the handler skips to the next event. No panic, no unwrap.

*Rationale*: Event consumer must stay alive to process future events. One bad event must not take down the consumer.

**D4: No Oneshot Channels for ACP**

Unlike MCP tools (which register oneshots in `state.pending_approvals` and block), ACP handlers only register with `AcpDriver::register_clearance()`/`register_prompt_request()`. The Slack button handler resolves via `state.driver.resolve_clearance()` which dispatches to `AcpDriver` (polymorphic), which writes the response back to the agent's stream writer. No blocking.

*Rationale*: ACP agents don't block on a tool call — they continue streaming and the response arrives as a separate message on their NDJSON stream.

## Complexity Tracking

No constitution violations. No complexity items to track.
