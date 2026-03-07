# Session Memory: 006-acp-event-wiring Phase 1 — Setup

**Date**: 2026-03-07
**Spec**: specs/006-acp-event-wiring/
**Phase**: 1 of 6
**Status**: COMPLETE

## Task Overview

Phase 1 establishes the feature branch and verifies a clean quality-gate baseline before
any implementation work begins.

## Current State

### Tasks Completed
- [x] T001: Create and checkout feature branch `006-acp-event-wiring` — branch already existed
- [x] T002: Quality gate baseline verified — all gates PASS

### Files Modified
- `specs/006-acp-event-wiring/tasks.md` — marked T001 and T002 as complete [x]

### Test Results (Baseline)
- `cargo check`: PASS
- `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`: PASS (0 warnings)
- `cargo fmt --all -- --check`: PASS
- `cargo test`: **919 tests PASS** — 37 (unit-fast) + 225 (unit) + 257 (contract) + 400 (integration)
- Baseline: CLEAN — no pre-existing failures

## Important Discoveries

- The feature branch `006-acp-event-wiring` was already created prior to this session; T001 was
  trivially satisfied by verifying `git branch --show-current` output.
- The `Out-File` piping from `cargo test` buffers until the command completes; reading the file
  mid-run will return only the first "Compiling" line. Wait for shell completion before reading.
- Compilation of all test binaries takes ~5–7 minutes on this machine due to the large integration
  test suite (33 modules). Factor this into subsequent phases.
- The checklists/requirements.md is 100% complete (16/16 items), so the constitution gate was
  satisfied before implementation began.

## Next Steps (Phase 2)

Phase 2: **Foundational — Shared Block Builder Extraction** (5 tasks)

Key work:
1. Write unit tests for extracted `build_approval_blocks()` and `build_prompt_blocks()` in
   `tests/unit/acp_event_wiring.rs` (tests first — TDD).
2. Extract both block builder functions (plus helpers: `prompt_type_label`, `prompt_type_icon`,
   `truncate_text`, `INLINE_DIFF_THRESHOLD`) from `mcp/tools/ask_approval.rs` and
   `mcp/tools/forward_prompt.rs` into `slack/blocks.rs` with `pub(crate)` visibility.
3. Update the MCP tool files to import from `slack/blocks.rs` instead of local definitions.

Files expected to change in Phase 2:
- `src/slack/blocks.rs` (primary target — adding exported functions)
- `src/mcp/tools/ask_approval.rs` (remove local builder, import from blocks)
- `src/mcp/tools/forward_prompt.rs` (remove local builder, import from blocks)
- `tests/unit/acp_event_wiring.rs` (new test file — TDD)

## Context to Preserve

- Agent-intercom is active (ping acknowledged). Use `thread_ts` from each phase's first broadcast
  for threading subsequent messages.
- Phase 1 broadcast `ts`: 1772908108.719409 (now closed — Phase 2 will open a new thread)
- Orchestrator top-level broadcast `ts`: 1772908029.746989 (use for orchestrator-level messages)
- Full build mode: 6 phases total, 24 tasks. Phase queue remaining: [2, 3, 4, 5, 6]
- 919 baseline tests — any regression in subsequent phases is a real bug.
- The plan.md Design Decision D2 is critical for Phase 3/4: clearance requests always use
  `post_message_direct()` (to capture `ts`); prompts use direct only for first thread creation.
- SC-003 vs D3 conflict: persistence failures emit `warn!` and continue rather than halting.
  The driver registration is skipped if DB persistence fails (prevents unaudited state).

## Unresolved Questions

- None for Phase 2. The extraction targets (`build_approval_blocks`, `build_prompt_blocks`)
  are clearly identified in D1 of the plan.
