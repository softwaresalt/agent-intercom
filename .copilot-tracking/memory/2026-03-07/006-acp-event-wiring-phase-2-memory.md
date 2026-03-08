# Session Memory: 006-acp-event-wiring Phase 2 — Shared Block Builder Extraction

**Date**: 2026-03-07
**Spec**: specs/006-acp-event-wiring/
**Phase**: 2 of 6
**Status**: COMPLETE

## Task Overview

Phase 2 extracted Slack block-building functions from MCP tool handlers to
`src/slack/blocks.rs` so both MCP and ACP paths share identical rendering logic.

## Current State

### Tasks Completed
- [x] T003: 18 TDD tests written (red) then PASS (green) in tests/unit/acp_event_wiring.rs
- [x] T004: Extracted to slack/blocks.rs: `build_approval_blocks`, `build_prompt_blocks`,
            `prompt_type_icon`, `prompt_type_label`, `truncate_text`, `INLINE_DIFF_THRESHOLD`
- [x] T005: ask_approval.rs — removed local function, uses `blocks::build_approval_blocks()`,
            `blocks::INLINE_DIFF_THRESHOLD`; removed unused `SlackBlock` import
- [x] T006: forward_prompt.rs — removed local functions, uses `blocks::build_prompt_blocks()`,
            `blocks::prompt_type_label()`, `blocks::truncate_text()`;
            util.rs now re-exports `truncate_text` from `crate::slack::blocks::truncate_text`
            so wait_for_instruction.rs continues to work unchanged
- [x] T007: Quality gates PASS — clippy ✓, fmt ✓, 943 tests pass (0 failures)

### Files Modified
- `tests/unit.rs` — added `mod acp_event_wiring;`
- `tests/unit/acp_event_wiring.rs` — NEW (18 unit tests, S042–S046)
- `src/slack/blocks.rs` — added imports (RiskLevel, PromptType); appended 6 functions +
  INLINE_DIFF_THRESHOLD constant
- `src/mcp/tools/ask_approval.rs` — removed INLINE_DIFF_THRESHOLD const + local
  build_approval_blocks fn; updated call sites to blocks::*
- `src/mcp/tools/forward_prompt.rs` — removed truncate_text import + 3 local functions;
  updated 3 call sites to blocks::*
- `src/mcp/tools/util.rs` — replaced truncate_text body with `pub use crate::slack::blocks::truncate_text`

### Test Baseline Delta
- Baseline: 919 tests
- Phase 2: 943 tests (+18 new unit tests, +6 slow integration tests)
- All 943 PASS

## Important Discoveries

- `wait_for_instruction.rs` also imports `util::truncate_text` — keeping the re-export in
  util.rs avoids touching that file in Phase 2 scope.
- Clippy caught unused `SlackBlock` import in the test file and "missing backticks" in a doc
  comment (`elapsed_seconds` → `` `elapsed_seconds` ``). Both fixed before committing.
- cargo fmt auto-corrected spacing in the new blocks.rs functions — always run fmt after edits.
- The build-approval-blocks function in ask_approval.rs called `blocks::text_section` and
  `blocks::diff_section` — the extracted version uses the same (no new dependencies).

## Next Steps (Phase 3)

Phase 3: **User Story 1 — Operator Approves ACP File Operation** (4 tasks)

Key work:
1. T008: Unit tests for `handle_clearance_requested` function
2. T009: Contract tests for ClearanceRequested event handler
3. T010: Integration test for full clearance flow
4. T011: Implement `handle_clearance_requested` in src/main.rs — wires event to:
   - `state.driver.register_clearance(request_id, session_id)`
   - `ApprovalRepo::create(approval_record)`
   - `blocks::build_approval_blocks() + approval_buttons()`
   - `SlackService::post_message_direct()` (always — to capture ts)
   - `SessionRepo::set_thread_ts()` if session has no thread anchor

Files expected to change:
- `src/main.rs` — implement ClearanceRequested handler in run_acp_event_consumer
- `tests/unit/acp_event_wiring.rs` — add unit tests for handler
- `tests/contract/acp_event_contract.rs` — new file for contract tests
- `tests/integration/acp_event_integration.rs` — new file for integration tests

## Context to Preserve

- Agent-intercom active. Phase 2 broadcast thread_ts: 1772908777.239049 (closed)
- Orchestrator top-level thread_ts: 1772908029.746989
- 943 tests baseline going into Phase 3
- Design Decision D2: clearance requests ALWAYS use `post_message_direct()` regardless
  of thread state (to capture ts for approval record). Only prompts use conditional direct/enqueue.
- Design Decision D3: Event handlers are background tokio tasks — errors emit warn! and continue.
  If DB persistence fails, SKIP driver registration (unaudited state is worse than no state).
- Design Decision D5: Use `state.driver` trait object (`Arc<dyn AgentDriver>`) — methods
  `register_clearance` and `register_prompt_request` are on the `AgentDriver` trait.
- FR-013: Validate file paths via `path_safety` before computing SHA-256 hash.
  Paths outside workspace → reject with `AppError::PathViolation` → use "new_file" sentinel.
- FR-007 + FR-008: Clearance requests always direct post; prompts direct post only for first
  thread, then enqueue for subsequent messages.
