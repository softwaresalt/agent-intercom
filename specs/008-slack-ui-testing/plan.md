# Implementation Plan: Slack UI Automated Testing

**Branch**: `008-slack-ui-testing` | **Date**: 2026-03-09 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/008-slack-ui-testing/spec.md`

## Summary

Add a three-tier automated testing framework for Slack UI interactions: Tier 1 (offline structural tests running in CI via `cargo test`), Tier 2 (live Slack API integration tests verifying message posting, threading, and interaction round-trips against a real workspace), and Tier 3 (browser-automated visual tests using Playwright to capture screenshots of actual Slack UI rendering, diagnose the modal-in-thread failure, and establish a visual regression baseline).

## Technical Context

**Language/Version**: Rust stable, edition 2021 (Tier 1 + Tier 2); TypeScript/Node.js (Tier 3 Playwright scripts)
**Primary Dependencies**: `slack-morphism` 2.17 (existing), `serde_json` (existing), `tokio` (existing); Playwright (new, Tier 3 only — Node.js external tool)
**Storage**: SQLite via sqlx (in-memory for tests — existing)
**Testing**: `cargo test` (Tier 1 + Tier 2 harness), Playwright Test (Tier 3)
**Target Platform**: Windows (primary dev), Linux (CI)
**Project Type**: Single workspace, two binaries + external test scripts
**Performance Goals**: Tier 1 < 30s added to `cargo test`; Tier 2 < 5 min per scenario set; Tier 3 < 10 min per visual suite
**Constraints**: Tier 1 must have zero external dependencies; Tiers 2–3 require test workspace credentials; Tier 3 requires Node.js + Playwright runtime
**Scale/Scope**: ~15 Block Kit builder functions, 6 interaction types, ~10 slash commands, 3 modal paths, ~30 Tier 1 tests, ~15 Tier 2 scenarios, ~10 Tier 3 visual scenarios

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Safety-First Rust | ✅ Pass | Tier 1 + 2 are pure Rust tests. Tier 3 is an external Node.js tool (not production code). No `unsafe`, no `unwrap`/`expect` in Rust test helpers. |
| II. MCP Protocol Fidelity | ✅ Pass | No MCP tool changes. Tests verify existing tool behavior. |
| III. Test-First Development | ✅ Pass | This feature IS the test infrastructure. Tests will be written first by definition. |
| IV. Security Boundary Enforcement | ✅ Pass | Tier 2–3 use a dedicated test workspace/channel. No production credentials in test code. |
| V. Structured Observability | ✅ Pass | Test results produce structured reports. No changes to tracing infrastructure. |
| VI. Single-Binary Simplicity | ⚠️ Note | Tier 3 adds an external Node.js/Playwright dependency. This is NOT bundled into the Rust binaries — it's a separate test tool in `tests/visual/`. Justified: browser automation cannot be done in Rust without disproportionate effort. |
| VII. CLI Workspace Containment | ✅ Pass | All test artifacts written within the workspace. |
| VIII. Destructive Terminal Command Approval | ✅ Pass | No destructive operations in test code. |

## Project Structure

### Documentation (this feature)

```text
specs/008-slack-ui-testing/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── contracts/           # Phase 1 output (test harness API)
└── tasks.md             # Phase 2 output (via /speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── slack/
│   ├── blocks.rs         # Existing — Block Kit builders (test target)
│   ├── client.rs         # Existing — SlackService (mock target for Tier 1)
│   ├── events.rs         # Existing — Event dispatcher (test target)
│   ├── commands.rs       # Existing — Slash command router (test target)
│   ├── push_events.rs    # Existing — Push event handler (test target)
│   └── handlers/         # Existing — Per-event handlers (test targets)
│       ├── approval.rs
│       ├── modal.rs
│       ├── prompt.rs
│       ├── wait.rs
│       ├── nudge.rs
│       ├── steer.rs
│       ├── task.rs
│       ├── thread_reply.rs
│       └── command_approve.rs

tests/
├── unit/
│   ├── blocks_tests.rs           # Existing — extend with comprehensive coverage
│   ├── blocks_approval_tests.rs  # New — approval Block Kit assertions
│   ├── blocks_prompt_tests.rs    # New — prompt Block Kit assertions
│   ├── blocks_session_tests.rs   # New — session lifecycle Block Kit assertions
│   ├── blocks_stall_tests.rs     # New — stall alert Block Kit assertions
│   ├── blocks_misc_tests.rs      # New — remaining builders (snippet, diff, command)
│   └── command_routing_tests.rs  # New — slash command routing/mode-gating
├── integration/
│   ├── test_helpers.rs           # Existing — extend with Slack test utilities
│   ├── slack_interaction_tests.rs    # New — synthetic interaction dispatch tests
│   ├── slack_modal_flow_tests.rs     # New — modal submission simulation
│   ├── slack_threading_tests.rs      # New — multi-session thread routing
│   └── slack_fallback_tests.rs       # New — thread-reply fallback flows
├── live/                         # New directory — Tier 2 live Slack tests
│   ├── mod.rs                    # Test module with feature gate
│   ├── live_helpers.rs           # Slack API test client, message verification
│   ├── live_message_tests.rs     # Post + verify messages via conversations.history
│   ├── live_interaction_tests.rs # Trigger buttons via synthetic payloads, verify state
│   ├── live_modal_tests.rs       # Modal open + verify via API (threaded vs top-level)
│   ├── live_threading_tests.rs   # Multi-session thread verification
│   └── live_command_tests.rs     # Slash command round-trip verification
└── visual/                       # New directory — Tier 3 Playwright tests
    ├── package.json              # Playwright + dependencies
    ├── playwright.config.ts      # Test configuration
    ├── auth/                     # Session persistence for Slack login
    │   └── .gitkeep
    ├── screenshots/              # Output directory for captured screenshots
    │   └── .gitkeep
    ├── reports/                  # HTML test reports
    │   └── .gitkeep
    ├── helpers/
    │   ├── slack-auth.ts         # Slack web client authentication
    │   ├── slack-nav.ts          # Channel/thread navigation helpers
    │   ├── slack-selectors.ts    # DOM selector strategies for Slack elements
    │   └── screenshot.ts         # Screenshot capture + naming utilities
    └── scenarios/
        ├── approval-flow.spec.ts     # Approval button interactions + screenshots
        ├── modal-in-thread.spec.ts   # THE critical test — modal rendering diagnosis
        ├── modal-top-level.spec.ts   # Modal baseline (non-threaded)
        ├── message-rendering.spec.ts # Block Kit visual verification
        ├── thread-reply-fallback.spec.ts  # Fallback mechanism visual verification
        └── button-replacement.spec.ts     # Button → status line transition
```

**Structure Decision**: Tier 1 tests go in `tests/unit/` and `tests/integration/` (matching existing patterns). Tier 2 live tests go in `tests/live/` (new directory, feature-gated). Tier 3 visual tests go in `tests/visual/` as a standalone Node.js project with Playwright, since browser automation is outside the Rust ecosystem.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Tier 3 adds Node.js/Playwright (Principle VI) | Browser automation against Slack web client requires a real browser engine. No mature Rust crate provides Playwright-equivalent Slack web UI automation. | Pure API testing (Tier 2 only) cannot capture screenshots or verify visual rendering — the core requirement for diagnosing the modal-in-thread issue. |
