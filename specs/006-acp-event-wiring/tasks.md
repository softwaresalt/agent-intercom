# Tasks: ACP Event Handler Wiring

**Input**: Design documents from `/specs/006-acp-event-wiring/`
**Prerequisites**: plan.md ✅, spec.md ✅, SCENARIOS.md ✅, data-model.md ✅, research.md ✅, quickstart.md ✅

**Tests**: Required — Constitution mandates TDD (write tests first, verify they FAIL, then implement).

**Organization**: Tasks grouped by user story. Each story is independently testable. All 56 scenarios from SCENARIOS.md mapped across 3 test tiers.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Exact file paths included in all descriptions

## Scenario Coverage Map

| Test Tier | File | Scenarios | Count |
|-----------|------|-----------|-------|
| Unit | tests/unit/acp_event_wiring.rs | S018–S023, S024–S029, S030–S035, S042–S046, S055–S056 | 25 |
| Contract | tests/contract/acp_event_contract.rs | S001–S009, S010–S017 | 17 |
| Integration | tests/integration/acp_event_integration.rs | S036–S041, S047–S054 | 14 |
| **Total** | | | **56** |

---

## Phase 1: Setup

**Purpose**: Branch creation and baseline verification

- [x] T001 Create and checkout feature branch `006-acp-event-wiring` from main
- [x] T002 Run quality gates to verify clean baseline: `cargo check && cargo clippy -- -D warnings && cargo fmt --all -- --check && cargo test`

---

## Phase 2: Foundational — Shared Block Builder Extraction

**Purpose**: Extract Slack block-building functions from MCP tool handlers to the shared `src/slack/blocks.rs` module (Design Decision D1). Both ACP event handlers depend on these shared builders.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T003 Write unit tests for `build_approval_blocks()` output structure (header, description section, risk badge, diff code block, Accept/Reject buttons), `build_prompt_blocks()` output structure (header with icon+label, prompt text section, Continue/Refine/Stop buttons), diff truncation at `INLINE_DIFF_THRESHOLD` (20 lines), prompt text truncation via `truncate_text()`, and MCP/ACP output equivalence in tests/unit/acp_event_wiring.rs (S042–S046)
- [x] T004 Extract `build_approval_blocks()`, `approval_buttons()`, and `INLINE_DIFF_THRESHOLD` from src/mcp/tools/ask_approval.rs (line ~438, ~24) and `build_prompt_blocks()`, `prompt_buttons()`, `prompt_type_label()`, `prompt_type_icon()` from src/mcp/tools/forward_prompt.rs (lines ~261, ~297, ~307) to src/slack/blocks.rs as `pub(crate)`; relocate `truncate_text()` from src/mcp/tools/util.rs (line ~13) to src/slack/blocks.rs as `pub(crate)`
- [x] T005 [P] Update src/mcp/tools/ask_approval.rs to remove local `build_approval_blocks` function and `INLINE_DIFF_THRESHOLD` constant; add `use crate::slack::blocks::{build_approval_blocks, INLINE_DIFF_THRESHOLD};`
- [x] T006 [P] Update src/mcp/tools/forward_prompt.rs to remove local `build_prompt_blocks`, `prompt_type_label`, `prompt_type_icon`; add `use crate::slack::blocks::{build_prompt_blocks, prompt_type_label, prompt_type_icon};`; update src/mcp/tools/util.rs to re-export `truncate_text` from `crate::slack::blocks` if other consumers exist, otherwise remove the local copy
- [x] T007 Run quality gates — verify shared block builder tests (S042–S046) pass and all existing MCP tool tests remain green: `cargo check && cargo clippy -- -D warnings && cargo fmt --all -- --check && cargo test`

**Checkpoint**: Shared block builders available at `crate::slack::blocks`. MCP tools unchanged in behavior. User story implementation can begin.

---

## Phase 3: User Story 1 — Operator Approves ACP File Operation (Priority: P1) 🎯 MVP

**Goal**: Wire the `ClearanceRequested` event handler to register pending clearances with `AcpDriver`, persist `ApprovalRequest` records to the database, and post interactive approval messages to Slack with Accept/Reject buttons.

**Independent Test**: Start an ACP session, trigger a clearance request from the agent, observe the Slack approval message with file path / risk level / diff content, click Accept, and verify the agent receives the approval response.

**Functional Requirements**: FR-001, FR-002, FR-003, FR-007, FR-009, FR-010, FR-011, FR-013

### Tests for User Story 1 ⚠️

> **TDD: Write these tests FIRST. Verify they compile and FAIL before proceeding to implementation (T010).**

- [x] T008 [P] [US1] Write unit tests in tests/unit/acp_event_wiring.rs for: (a) risk_level parse-or-default semantics — "low"→Low, "high"→High, "critical"→Critical, unknown string "extreme"→Low, empty ""→Low, mixed-case "High"/"LOW"→Low (S018–S023); (b) SHA-256 content hash computation — file exists→hex digest, file not found→"new_file" sentinel, empty file→SHA-256 of empty bytes, path traversal "../../etc/passwd"→rejected by path_safety, absolute path outside workspace→rejected, null bytes in path→rejected (S030–S035); (c) ClearanceRequested→ApprovalRequest field mapping — session_id direct copy, title direct copy, description→Some(description), diff→unwrap_or_default(), file_path direct copy, risk_level→parsed enum, original_hash→computed, status=Pending, consumed_at=None (S055)
- [x] T009 [P] [US1] Write contract tests in tests/contract/acp_event_contract.rs for ClearanceRequested handler pipeline with mock Slack/DB: standard flow with all fields and low risk (S001), None diff→empty diff_content (S002), high risk level (S003), critical risk level (S004), missing session→warn log+discard event+no side effects (S005), Slack unavailable→persist to DB+register with driver+skip Slack post (S006), DB persistence failure→warn+continue+driver still registered (S007), empty description string (S008), large diff >100KB→stored in full+Slack blocks truncated (S009)

### Implementation for User Story 1

- [x] T010 [US1] Implement `AgentEvent::ClearanceRequested` match arm in src/main.rs `run_acp_event_consumer` (replacing current no-op log)
- [x] T011 [US1] Run quality gates — verify all US1 unit tests (S018–S023, S030–S035, S055) and contract tests (S001–S009) pass: `cargo check && cargo clippy -- -D warnings && cargo fmt --all -- --check && cargo test`

**Checkpoint**: ClearanceRequested events produce interactive approval messages in Slack. User Story 1 independently testable.

---

## Phase 4: User Story 2 — Operator Responds to ACP Continuation Prompt (Priority: P1)

**Goal**: Wire the `PromptForwarded` event handler to register pending prompts with `AcpDriver`, persist `ContinuationPrompt` records to the database, and post interactive prompt messages to Slack with Continue/Refine/Stop buttons.

**Independent Test**: Start an ACP session, trigger a prompt forwarding event, observe the Slack prompt message with prompt type label and text, click a response button, and verify the agent receives the operator's decision.

**Functional Requirements**: FR-004, FR-005, FR-006, FR-009, FR-010, FR-012

### Tests for User Story 2 ⚠️

> **TDD: Write these tests FIRST. Verify they compile and FAIL before proceeding to implementation (T014).**

- [x] T012 [P] [US2] Write unit tests in tests/unit/acp_event_wiring.rs for: (a) prompt_type parse-or-default semantics — "continuation"→Continuation, "clarification"→Clarification, "error_recovery"→ErrorRecovery, "resource_warning"→ResourceWarning, unknown "custom_agent_query"→Continuation, empty ""→Continuation (S024–S029); (b) PromptForwarded→ContinuationPrompt field mapping — session_id direct copy, prompt_text direct copy, prompt_type→parsed enum, elapsed_seconds=None (ACP-specific), actions_taken=None (ACP-specific), decision=None, instruction=None, slack_ts=None (S056)
- [x] T013 [P] [US2] Write contract tests in tests/contract/acp_event_contract.rs for PromptForwarded handler pipeline with mock Slack/DB: standard continuation type (S010), clarification type with icon+label (S011), error_recovery type (S012), resource_warning type (S013), missing session→warn log+discard+no side effects (S014), Slack unavailable→persist+register+skip post (S015), DB persistence failure→warn+continue+driver registered (S016), empty prompt_text (S017)

### Implementation for User Story 2

- [x] T014 [US2] Implement `AgentEvent::PromptForwarded` match arm in src/main.rs `run_acp_event_consumer` (replacing current no-op log at line ~788): (1) look up session via `SessionRepo::find_by_id(session_id)`, if not found emit `warn!` with session_id and `continue`; (2) emit `info!` tracing span with session_id, event type "PromptForwarded", and prompt_id (FR-014); (3) parse `prompt_type` string to `PromptType` enum using case-sensitive matching per FR-012 — only lowercase `"continuation"`, `"clarification"`, `"error_recovery"`, `"resource_warning"` recognized, all others default to `PromptType::Continuation`; (4) construct `ContinuationPrompt::new(session_id, prompt_text, prompt_type, None, None)` with elapsed_seconds=None and actions_taken=None, persist via `PromptRepo::create(&prompt)` — on DB failure emit `warn!` and skip remaining steps for this event; (5) register with `AcpDriver::register_prompt_request(session_id, prompt.id)` using DB-generated prompt ID (consistent with T010 pattern); (6) build Slack blocks via `crate::slack::blocks::build_prompt_blocks(prompt_text, prompt_type, None, None, prompt.id)`; (7) if Slack configured and `thread_ts=None`, use `post_message_direct()` and save returned ts as session thread anchor (D2 conditional posting); if `thread_ts` exists, use `enqueue()`; if Slack not configured, emit `warn!` and skip; (8) wrap all fallible operations in warn-and-continue error handling (D3)
- [x] T015 [US2] Run quality gates — verify all US2 unit tests (S024–S029, S056) and contract tests (S010–S017) pass: `cargo check && cargo clippy -- -D warnings && cargo fmt --all -- --check && cargo test`

**Checkpoint**: PromptForwarded events produce interactive prompt messages in Slack. User Stories 1 AND 2 independently testable.

---

## Phase 5: User Story 3 — Session Thread Continuity (Priority: P2)

**Goal**: Ensure ACP clearance and prompt messages create new Slack threads when none exist for the session, and reply in existing threads when one is already established. First-message detection + `SessionRepo::set_thread_ts()` anchors all subsequent messages to the same thread.

**Independent Test**: Start an ACP session with no prior Slack thread, trigger a clearance request, verify it creates a new thread, trigger a second event, verify it appears in the same thread.

**Functional Requirements**: FR-007, FR-008

### Tests for User Story 3 ⚠️

> **TDD: Write these tests FIRST. Verify they compile and FAIL before proceeding to implementation (T017–T018).**

- [x] T016 [US3] Write integration tests in tests/integration/acp_event_integration.rs for thread management: clearance creates thread when session has thread_ts=None — message posted via post_message_direct, returned ts saved via set_thread_ts (S036); clearance replies in existing thread when thread_ts=Some — message posted with thread_ts, set_thread_ts is no-op (S037); prompt creates thread when first event and thread_ts=None — posted via post_message_direct not enqueue, ts saved (S038); prompt enqueues to existing thread when thread_ts=Some (S039); set_thread_ts is idempotent — SQL WHERE thread_ts IS NULL prevents overwrite (S040); clearance creates thread then prompt uses same thread — sequential events share thread anchor (S041)

### Implementation for User Story 3

- [x] T017 [US3] Add first-message thread_ts detection to ClearanceRequested handler in src/main.rs: after `post_message_direct` succeeds and returns `slack_ts`, check if `session.thread_ts.is_none()`; if so, call `SessionRepo::set_thread_ts(session_id, &slack_ts)` to anchor the session thread; the SQL UPDATE uses `WHERE thread_ts IS NULL` predicate for idempotent writes (S036–S037, S040)
- [x] T018 [US3] Add conditional posting logic to PromptForwarded handler in src/main.rs: replace unconditional `SlackService::enqueue` with branch — if `session.thread_ts.is_none()`, use `SlackService::post_message_direct` (not enqueue) to create the thread and save returned ts via `SessionRepo::set_thread_ts`; if `session.thread_ts.is_some()`, use `SlackService::enqueue` with existing thread_ts for async delivery (S038–S039, S041)
- [x] T019 [US3] Run quality gates — verify all US3 integration tests (S036–S041) pass and US1/US2 tests remain green: `cargo check && cargo clippy -- -D warnings && cargo fmt --all -- --check && cargo test`

**Checkpoint**: All three user stories independently functional. Thread continuity verified across sequential events.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Verify concurrent behavior, lifecycle edge cases, full scenario coverage, and manual end-to-end validation.

- [ ] T020 Write integration tests in tests/integration/acp_event_integration.rs for: (a) concurrent event processing — two clearance requests for same session in rapid succession create separate records and messages (S047), interleaved clearance+prompt for same session produce independent records with no cross-contamination (S048), events from multiple sessions processed independently with no shared state leakage (S049), AcpDriver registration and DB persistence are consistent under slow DB writes (S050); (b) event consumer lifecycle — normal dispatch loop receives and routes multiple event variants correctly (S051), cancellation token fires causing graceful consumer exit (S052), mpsc sender dropped causing consumer exit on channel close (S053), operator responds to clearance after ACP session terminated — driver returns error, Slack handler logs warning (S054); (c) full round-trip flow — emit ClearanceRequested event → handler registers + persists + posts to Slack → simulate operator Accept button click → verify resolve_clearance dispatches to agent stream with correct approval ID (S067 authorization guard verified, S068 thread_ts DB failure path) *(UF-23: ensures end-to-end wiring is correct)*
- [ ] T021 Verify all 56 SCENARIOS.md scenarios are covered by test assertions — cross-reference scenario IDs S001–S056 against test function names across tests/unit/acp_event_wiring.rs (25 scenarios), tests/contract/acp_event_contract.rs (17 scenarios), tests/integration/acp_event_integration.rs (14 scenarios)
- [ ] T022 Run full quality gate suite: `cargo check && cargo clippy -- -D warnings && cargo fmt --all -- --check && cargo test`
- [ ] T023 [P] Run quickstart.md manual validation scenarios: Test 1 (clearance request flow — trigger clearance, observe Slack approval message, click Accept, verify agent proceeds), Test 2 (prompt forwarding flow — trigger prompt, observe Slack message, click Continue, verify agent receives decision), Test 3 (thread continuity — first event creates thread, second event replies in same thread)
- [ ] T024 Commit all changes with conventional commit messages on feature branch `006-acp-event-wiring` — one coherent commit per phase (per constitution Development Workflow §5). Depends on T023 completion.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — **BLOCKS all user stories**
- **US1 (Phase 3)**: Depends on Phase 2 completion
- **US2 (Phase 4)**: Depends on Phase 3 completion — MUST follow Phase 3 (both modify `run_acp_event_consumer` in src/main.rs, parallel execution would cause merge conflicts)
- **US3 (Phase 5)**: Depends on Phase 3 + Phase 4 (adds thread management to both handlers)
- **Polish (Phase 6)**: Depends on all user stories complete

### User Story Dependencies

- **US1 (P1)**: Can start after Phase 2 — no dependencies on other stories
- **US2 (P1)**: Can start after Phase 2 — logically independent of US1 but modifies same file (src/main.rs); sequential after US1 avoids merge conflicts
- **US3 (P2)**: Depends on US1 + US2 completion — modifies both handler implementations to add thread management

### Within Each User Story

1. Tests MUST be written and verified to FAIL before implementation (TDD red phase)
2. Implementation satisfies failing tests (TDD green phase)
3. Quality gates verify no regressions across all tiers
4. Story checkpoint — independently testable increment

### Parallel Opportunities

| Phase | Parallel Tasks | Reason |
|-------|---------------|--------|
| Phase 2 | T005 ‖ T006 | Different MCP tool files (ask_approval.rs vs forward_prompt.rs) |
| Phase 3 | T008 ‖ T009 | Different test tiers (unit/ vs contract/) |
| Phase 4 | T012 ‖ T013 | Different test tiers (unit/ vs contract/) |
| Phase 6 | T023 → T024 | T024 depends on T023 completion (no commit before validation) |

---

## Parallel Execution Examples

### Phase 2: Shared Block Extraction

```text
Sequential: T003 → T004 → (T005 ‖ T006) → T007
                            ↑ parallel ↑
```

### Phase 3: User Story 1

```text
Parallel test writing: (T008 ‖ T009) → T010 → T011
                        ↑ parallel ↑    impl    gates
```

### Phase 4: User Story 2

```text
Parallel test writing: (T012 ‖ T013) → T014 → T015
                        ↑ parallel ↑    impl    gates
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational block extraction (**CRITICAL** — blocks all stories)
3. Complete Phase 3: User Story 1 — ClearanceRequested handler
4. **STOP and VALIDATE**: Trigger clearance request from ACP agent → Slack approval message appears
5. Deploy/demo if ready — agents can now request file operation approval

### Incremental Delivery

1. Setup + Foundational → Shared block builders ready
2. Add US1 → Test clearance flow independently → **MVP achievable** 🎯
3. Add US2 → Test prompt flow independently → Full ACP human-in-the-loop model
4. Add US3 → Test thread continuity → Polished operator experience
5. Polish → Concurrent + lifecycle tests → Production-ready

### File Change Summary

| File | Phase(s) | Change Type |
|------|----------|-------------|
| src/slack/blocks.rs | 2 | Add extracted block builder functions (`pub(crate)`) |
| src/mcp/tools/ask_approval.rs | 2 | Remove local builders; add import from `crate::slack::blocks` |
| src/mcp/tools/forward_prompt.rs | 2 | Remove local builders; add import from `crate::slack::blocks` |
| src/mcp/tools/util.rs | 2 | Re-export or remove `truncate_text` |
| src/main.rs | 3, 4, 5 | Implement ClearanceRequested + PromptForwarded handlers + thread mgmt |
| tests/unit/acp_event_wiring.rs | 2, 3, 4 | **New file**: 25 unit test scenarios |
| tests/contract/acp_event_contract.rs | 3, 4 | **New file**: 17 contract test scenarios |
| tests/integration/acp_event_integration.rs | 5, 6 | **New file**: 14 integration test scenarios |

---

## Notes

- **[P]** tasks target different files with no dependencies on incomplete tasks
- **[Story]** label maps each task to its user story for traceability
- Each user story is independently completable and testable at its checkpoint
- **TDD required**: Verify tests FAIL before implementing (Constitution principle III)
- **Error handling**: Warn-and-continue for all handler errors (Design Decision D3)
- **No new dependencies**: Uses existing rmcp, sqlx, slack-morphism, sha2, tokio crates
- **Quality gates per phase**: `cargo check` + `cargo clippy -- -D warnings` + `cargo fmt --all -- --check` + `cargo test`
- **Design decisions**: D1 (shared blocks in slack/blocks.rs), D2 (direct post for clearance, enqueue for prompt), D3 (log warn + continue), D4 (AcpDriver-only registration, no oneshot channels)
