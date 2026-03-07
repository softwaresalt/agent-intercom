# Session Memory: 006-acp-event-wiring Phase 3

**Date**: 2026-03-07  
**Phase**: 3 — User Story 1: Operator Approves ACP File Operation  
**Branch**: 006-acp-event-wiring  
**Baseline**: 943 tests → **967 tests** (+24)

---

## Task Overview

Phase 3 wires the `ClearanceRequested` ACP event to the full operator approval pipeline:
- TDD unit tests (S018–S023, S030–S035, S055)
- Contract tests (S001–S009)
- `parse_risk_level()` added to `models/approval.rs`
- `update_slack_ts()` added to `persistence/approval_repo.rs`
- `handle_clearance_requested()` helper implemented in `src/main.rs`
- Adversarial review found and fixed a HIGH severity ACP correlation bug

---

## Current State

### Tasks Completed
- [x] T008 — Unit tests: risk_level parsing (S018–S023), SHA-256 hash (S030–S035), field mapping (S055)
- [x] T009 — Contract tests: ClearanceRequested pipeline (S001–S009)
- [x] T010 — Implementation: ClearanceRequested handler in main.rs + parse_risk_level + update_slack_ts
- [x] T011 — Quality gates: 967 tests pass, clippy clean, fmt clean

### Files Modified/Created
- `src/models/approval.rs` — added `pub fn parse_risk_level(s: &str) -> RiskLevel`
- `src/persistence/approval_repo.rs` — added `pub async fn update_slack_ts(id, slack_ts)`
- `src/main.rs` — added `handle_clearance_requested()` + updated `ClearanceRequested` match arm
- `tests/unit/acp_event_wiring.rs` — appended 18 new tests (S018–S023, S030–S035, S055)
- `tests/contract/acp_event_contract.rs` — NEW FILE with 13 contract tests (S001–S009)
- `tests/contract.rs` — added `mod acp_event_contract;`
- `specs/006-acp-event-wiring/tasks.md` — T008–T011 marked [x]

### Test Results
- 37 (contract) + 236 (integration) + 257 (unit) + 431 (unit) + 6 (doc) = **967 tests, 0 failures**

---

## Important Discoveries

### Adversarial Review Finding (HIGH): ACP Request ID Correlation Bug
**Problem**: Original implementation used `approval.id` (server-generated UUID) as both the:
- `AcpDriver::register_clearance` key  
- Slack button `value`

This caused `clearance/response` to send `"id": approval_uuid` to the agent, but the agent sent `"id": request_id` and expects to receive back its own `request_id` for JSON-RPC correlation.

**Fix**: Override `approval.id = request_id.to_owned()` after construction. The agent's `request_id` (from `ClearanceRequested.request_id` = ACP envelope `id` field) is now used as:
1. The `ApprovalRequest.id` in the DB
2. The `register_clearance` key in `AcpDriver::pending_clearances`
3. The Slack button value
4. The `id` field in `clearance/response` sent back to the agent

This ensures the existing `approval_repo.get_by_id(request_id)` in the Slack approval handler also finds the record correctly.

### Clippy Issues Encountered
- `match_same_arms`: `parse_risk_level` had explicit "low" arm matching wildcard → removed explicit "low" arm
- `too_many_lines` (108/100): `run_acp_event_consumer` → `#[allow(clippy::too_many_lines)]`
- `too_many_lines` (106/100): `handle_clearance_requested` → same
- `too_many_arguments` (8/7): `handle_clearance_requested` → `#[allow(clippy::too_many_arguments)]`
- `map_unwrap_or`: `map().unwrap_or_else()` → `map_or_else()`
- `unnecessary_literal_unwrap`: `None.unwrap_or_default()` → direct `String::new()`
- `deprecated`: `TempDir::into_path()` → `dir.path()` (keep TempDir alive in scope)
- `doc_markdown`: `ClearanceRequested`, `ApprovalRequest`, `AcpDriver` in doc comments → backticks

### parse_risk_level Semantics (FR-011)
Case-sensitive matching (lowercase only):
- `"high"` → `RiskLevel::High`
- `"critical"` → `RiskLevel::Critical`
- all others (including `"low"`, mixed-case, empty) → `RiskLevel::Low` (default)
This is correct per spec: only `"high"` and `"critical"` need explicit handling since Low is the safe default.

### DB approval_repo.parse_risk_level vs models::approval::parse_risk_level
Two separate functions exist:
- `persistence/approval_repo.rs` has a private `parse_risk_level` → `Result<RiskLevel>` (strict, for DB deserialization)
- `models/approval.rs` has `pub parse_risk_level` → `RiskLevel` (lenient, for ACP event parsing with default)
These serve different purposes and must not be confused.

### Slack Post Pattern
`ClearanceRequested` handler uses `post_message_direct()` (not `enqueue()`) to capture the returned `SlackTs`. This matches the `ask_approval.rs` pattern. After posting:
1. If session had no `thread_ts`, store approval post ts as thread root
2. Store `slack_ts` on approval record via `update_slack_ts()` (for button replacement on operator decision)

---

## Next Steps (Phase 4)

**Phase 4**: User Story 2 — Operator Responds to ACP Continuation Prompt

Remaining tasks:
- T012: Unit tests in `tests/unit/acp_event_wiring.rs` for:
  - prompt_type parse-or-default semantics (S024–S029)
  - PromptForwarded→ContinuationPrompt field mapping (S056)
- T013: Contract tests in `tests/contract/acp_event_contract.rs` for PromptForwarded pipeline (S010–S017)
- T014: Implement `PromptForwarded` match arm in `src/main.rs`
- T015: Quality gates

**Key implementation points for T014**:
- `parse_prompt_type` function needed in `models/prompt.rs` (parallel to `parse_risk_level`)
- `AcpDriver::register_prompt_request(session_id, prompt_id)` at `src/driver/acp_driver.rs:181`
- Use `PromptRepo::create()` from `persistence/prompt_repo.rs` (check exists)
- Use agent's `prompt_id` (from event) as the `ContinuationPrompt.id` (same pattern as request_id→approval.id)
- The `PromptForwarded` event has `prompt_id: String` — this IS already available in the event (unlike `approval_id`)
- Actually, re-check: `AgentEvent::PromptForwarded` has `prompt_id: String` — this is the agent's correlation ID

**Pending questions for Phase 4**:
- Does `PromptRepo` exist? If not, need to create it. Check `src/persistence/prompt_repo.rs`.
- What's the field name for prompt ID in `ContinuationPrompt`? Check `src/models/prompt.rs`.
- Does `ContinuationPrompt::new()` accept an optional custom ID, or do we also need `prompt.id = prompt_id.to_owned()`?

---

## Context to Preserve

### Architecture References
- `src/main.rs` `run_acp_event_consumer`: `ClearanceRequested` arm now calls `handle_clearance_requested()` (full implementation)
- `src/main.rs` `handle_clearance_requested()`: 8-parameter function, ~110 lines
- `src/driver/mod.rs`: `AgentEvent::ClearanceRequested` fields: `request_id, session_id, title, description, diff: Option<String>, file_path, risk_level`
- `src/driver/acp_driver.rs:164`: `register_clearance(session_id, request_id)` — use agent's request_id
- `src/driver/acp_driver.rs:181`: `register_prompt_request(session_id, prompt_id)` — for Phase 4
- `src/slack/blocks.rs`: `build_approval_blocks(title, description, diff, file_path, risk_level)` + `approval_buttons(request_id)`
- `src/slack/client.rs`: `SlackService::post_message_direct()` → returns `SlackTs`
- `src/persistence/session_repo.rs:647`: `set_thread_ts(session_id, ts)` for thread anchoring
- `src/models/approval.rs:80`: `ApprovalRequest::new(session_id, title, description, diff_content, file_path, risk_level, original_hash)`
- `src/mcp/tools/util.rs:22`: `compute_file_hash(path: &Path) -> Result<String, io::Error>` — returns "new_file" for missing

### Orchestrator Thread
- Top-level thread_ts: `1772909738.562799` (Phase 3 progress) 
- Latest broadcast ts: `1772912272.187609`

### Phase Status
- [x] Phase 1: Setup (commit `5c34d89`)
- [x] Phase 2: Shared Block Builder Extraction (commit `3b886ff`)
- [x] Phase 3: User Story 1 — ClearanceRequested handler (commit pending)
- [ ] Phase 4: User Story 2 — PromptForwarded handler
- [ ] Phase 5: Session Thread Continuity
- [ ] Phase 6: Polish
