# Session Memory — 006-acp-event-wiring Phase 4

**Date**: 2026-03-07
**Spec**: 006-acp-event-wiring
**Phase**: 4 — User Story 2: Operator Responds to ACP Continuation Prompt
**Status**: COMPLETE (982 tests passing)

---

## Task Overview

Phase 4 wired the `AgentEvent::PromptForwarded` event through the ACP event consumer pipeline:
- Parse `prompt_type` string to `PromptType` enum (lenient, defaults to Continuation)
- Persist `ContinuationPrompt` record to SQLite via `PromptRepo`
- Register with `AcpDriver::register_prompt_request()` for response routing
- Post interactive Slack message with Continue/Refine/Stop buttons

### Tasks Completed
- **T012** [P]: Unit tests for `parse_prompt_type` (S024–S029) and field mapping (S056)
- **T013** [P]: Contract tests for PromptForwarded pipeline (S010–S017)
- **T014**: `PromptForwarded` match arm + `handle_prompt_forwarded()` implementation
- **T015**: Quality gates — 982 tests pass, clippy clean, fmt clean

---

## Current State

### Files Modified
- `src/models/prompt.rs` — Added `pub fn parse_prompt_type(s: &str) -> PromptType`
- `src/main.rs` — Updated `PromptForwarded` match arm; added `handle_prompt_forwarded()` (~115 lines, `#[allow(clippy::too_many_lines)]`)
- `tests/unit/acp_event_wiring.rs` — Added S024–S029 (parse_prompt_type) + S056 (field mapping) — 8 new tests
- `tests/contract/acp_event_contract.rs` — Added S010–S017 (PromptForwarded pipeline) — 8 new tests
- `specs/006-acp-event-wiring/tasks.md` — T012–T015 marked `[x]`

### Test Counts
- Before Phase 4: 967 tests
- After Phase 4: 982 tests (+15)
- All passing, 0 failures

---

## Important Discoveries

### 1. `parse_prompt_type` uses 2-arm match (not 4)
- `"clarification"`, `"error_recovery"`, `"resource_warning"` have explicit arms
- `_` wildcard covers `"continuation"` AND all unknown/empty values
- Cannot add explicit `"continuation"` arm — `clippy::match_same_arms` fires (identical body to wildcard)
- This is the same pattern as `parse_risk_level` in Phase 3

### 2. `prompt_id` used as `prompt.id` (ACP JSON-RPC correlation)
- `ContinuationPrompt::new()` generates a UUID; we override `.id = prompt_id.to_owned()`
- This ensures: `prompt_repo.get_by_id(prompt_id)` works in Slack prompt handler
- `register_prompt_request(session_id, &prompt_db_id)` uses correct key
- Pattern is identical to Phase 3's `approval.id = request_id.to_owned()`

### 3. `slack_ts` NOT stored on prompt record (intentional asymmetry vs. clearance)
- `PromptRepo` has no `update_slack_ts()` method
- Slack button-replacement handler extracts ts from the **event payload** (`message.origin.ts`)
- This is different from `ApprovalRepo.update_slack_ts()` — approvals use DB-stored ts for re-posting
- Adversarial review confirmed this asymmetry is correct by design

### 4. D2 Conditional Posting (different from clearance)
- `thread_ts=None` → `post_message_direct()` → capture ts → `session_repo.set_thread_ts()`
- `thread_ts=Some` → `enqueue()` (rate-limited queue, ordered delivery)
- `handle_clearance_requested` always uses `post_message_direct()` (captures ts for `update_slack_ts`)
- Prompt handler uses enqueue for threaded messages — avoids out-of-order delivery

### 5. Clippy pedantic fixes in tests
- `doc_markdown`: `decision=None`, `slack_ts=None` etc. in doc comments need backticks
- `needless_bool`: `if db_failed { false } else { true }` → `!db_failed`
- `cargo fmt` reformatted assert! macro calls with longer messages to multi-line

### 6. Session recovery after firewall disconnect
- Phase 4 code was complete but uncommitted when session ended
- All 982 tests still passed on recovery — no state corruption
- Adversarial review ran post-recovery and found 0 critical/high issues

---

## Next Steps (Phase 5)

**Phase 5: Session Thread Continuity (T016–T020)**

Goal: Wire the `SessionThreaded` event (or equivalent) to ensure that when an ACP session already has a `thread_ts`, all subsequent messages (status updates, prompts, clearances) are posted into that thread.

Tasks to implement:
- T016: Write unit tests for thread_ts propagation logic
- T017: Write contract tests for thread continuity scenarios
- T018: Implement thread_ts anchoring on first-message events
- T019: Verify debounced StatusUpdated flush uses thread_ts
- T020: Quality gates

**Open questions for Phase 5**:
- Does the `StatusUpdated` flush in `flush_text_to_slack()` already use `thread_ts`? Check `flush_text_to_slack` signature.
- Does the `SessionTerminated` handler post to thread?
- Does `handle_clearance_requested` need to be updated now that Phase 4 introduced the `enqueue` path?

**Key files to read at Phase 5 start**:
- `src/main.rs` around `flush_text_to_slack` definition
- `src/main.rs` `SessionTerminated` match arm (line ~819)
- `specs/006-acp-event-wiring/tasks.md` Phase 5 task descriptions

---

## Context to Preserve

- Orchestrator Slack thread_ts: `1772909738.562799`
- Branch: `006-acp-event-wiring`
- Latest commit (Phase 3): `00b611f`
- Phase 4 changes are uncommitted at memory-write time (commit follows immediately)
- `parse_prompt_type` is in `src/models/prompt.rs` (public, lenient)
- `parse_risk_level` is in `src/models/approval.rs` (public, lenient) — Phase 3
- Both have private strict variants in their respective `_repo.rs` files for DB deserialization
- `ContinuationPrompt.id` is `pub String` — can be overridden post-construction
- `ApprovalRequest.id` is `pub String` — same pattern
