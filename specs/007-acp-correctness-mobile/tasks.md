# Tasks: ACP Correctness Fixes and Mobile Operator Accessibility

**Input**: Design documents from `/specs/007-acp-correctness-mobile/`
**Prerequisites**: plan.md (required), spec.md (required), SCENARIOS.md, research.md, data-model.md

**Tests**: Required per Constitution Principle III (Test-First Development). Tests are
written first and must fail before implementation code is written.

**Organization**: Tasks are grouped by user story. Each story can be implemented and tested
independently. US3 (Workspace Routing) is excluded — F-08 is already fixed.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US5)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: No project initialization needed — all changes target existing modules.
This phase is empty for a correctness-fix feature.

**Checkpoint**: N/A — proceed directly to Phase 2.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: No shared foundational work needed. All four fixes (F-06, F-07, F-10, F-13)
are independent and target separate modules. Each user story phase is self-contained.

**Checkpoint**: N/A — user story phases can begin immediately.

---

## Phase 3: User Story 1 — Reliable Operator Steering Delivery (Priority: P1) 🎯 MVP

**Goal**: Fix steering message consumption so messages are only marked consumed after
successful delivery, preserving failed deliveries for retry.

**Independent Test**: Simulate a steering delivery failure and verify the message remains
unconsumed in the database; simulate success and verify consumption.

**Fix**: F-06 | **Scenarios**: S001–S007 | **FRs**: FR-001, FR-002

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T001 [P] [US1] Unit test: successful delivery marks consumed — verify `mark_consumed` called after `send_prompt` succeeds (S001) in `tests/unit/acp_reader_steering_delivery.rs`
- [x] T002 [P] [US1] Unit test: failed delivery preserves unconsumed status — mock driver returns error, verify `mark_consumed` NOT called (S002) in `tests/unit/acp_reader_steering_delivery.rs`
- [x] T003 [P] [US1] Unit test: partial failure — 3 messages, middle fails, verify only failed one stays unconsumed (S003) in `tests/unit/acp_reader_steering_delivery.rs`
- [x] T004 [P] [US1] Unit test: retry succeeds on next flush — previously failed message delivered on second call (S004) in `tests/unit/acp_reader_steering_delivery.rs`
- [x] T005 [P] [US1] Unit test: empty queue is no-op (S006) in `tests/unit/acp_reader_steering_delivery.rs`
- [x] T006 [P] [US1] Unit test: mark_consumed failure after successful send — warning logged, message stays unconsumed (S007) in `tests/unit/acp_reader_steering_delivery.rs`

### Implementation for User Story 1

- [x] T007 [US1] Fix `flush_queued_messages` in `src/acp/reader.rs` — restructure loop so `mark_consumed` is only called when `send_prompt` returns `Ok`; on error, log warning and continue to next message
- [x] T008 [US1] Verify all existing tests pass after F-06 fix — run `cargo test`

**Checkpoint**: Steering messages survive delivery failures and retry on next reconnect.

---

## Phase 4: User Story 2 — Accurate ACP Session Capacity Enforcement (Priority: P1)

**Goal**: Fix session capacity counting to include `created` state and filter by ACP protocol.

**Independent Test**: Create sessions in various states and protocols, verify only ACP
`active`+`created` sessions count toward the ACP capacity limit.

**Fix**: F-07 | **Scenarios**: S008–S015 | **FRs**: FR-003, FR-004

### Tests for User Story 2 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T009 [P] [US2] Unit test: `count_active_acp` counts both `active` and `created` ACP sessions (S010) in `tests/unit/session_repo_count_acp.rs`
- [x] T010 [P] [US2] Unit test: `count_active_acp` excludes MCP sessions from count (S011) in `tests/unit/session_repo_count_acp.rs`
- [x] T011 [P] [US2] Unit test: `count_active_acp` excludes paused and terminated sessions (S015) in `tests/unit/session_repo_count_acp.rs`
- [x] T012 [P] [US2] Contract test: ACP session start rejected at capacity including `created` sessions (S008, S010) in `tests/contract/acp_capacity_contract.rs`
- [x] T013 [P] [US2] Contract test: ACP session start succeeds when only MCP sessions are active (S011) in `tests/contract/acp_capacity_contract.rs`
- [x] T014 [P] [US2] Unit test: max_sessions = 0 rejects all starts (S014) in `tests/unit/session_repo_count_acp.rs`

### Implementation for User Story 2

- [x] T015 [US2] Add `count_active_acp()` method to `src/persistence/session_repo.rs` — query `WHERE (status = 'active' OR status = 'created') AND protocol_mode = 'acp'`
- [x] T016 [US2] Replace `repo.count_active()` with `repo.count_active_acp()` in `handle_acp_session_start` in `src/slack/commands.rs`
- [x] T017 [US2] Verify all existing tests pass after F-07 fix — run `cargo test`

**Checkpoint**: ACP capacity enforcement accurately counts initializing sessions and only ACP protocol.

---

## Phase 5: User Story 5 — Protocol Hygiene and Connection Safety (Priority: P2)

**Goal**: Remove legacy `channel_id` query parameter from MCP endpoint and replace
counter-based correlation IDs with UUIDs.

**Independent Test**: Connect to `/mcp` with only `workspace_id`; verify `channel_id` is
ignored. Generate thousands of correlation IDs and verify zero collisions.

**Fixes**: F-10, F-13 | **Scenarios**: S016–S026 | **FRs**: FR-007, FR-008

### Tests for User Story 5 — F-10 (channel_id removal) ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T018 [P] [US5] Unit test: `update_pending_from_uri` only extracts `session_id` and `workspace_id` (S018, S019) in `tests/unit/sse_workspace_only_routing.rs`
- [x] T019 [P] [US5] Unit test: `workspace_id` resolves channel from mapping (S016) in `tests/unit/sse_workspace_only_routing.rs`
- [x] T020 [P] [US5] Unit test: unknown workspace_id logs warning, no channel (S020) in `tests/unit/sse_workspace_only_routing.rs`
- [x] T021 [P] [US5] Contract test: `/mcp?channel_id=C_DIRECT` — channel_id silently ignored (S018) in `tests/contract/mcp_no_channel_id_contract.rs`
- [x] T022 [P] [US5] Update existing `workspace_mapping_tests.rs` — remove/update tests that reference `channel_id` as fallback param in `tests/unit/workspace_mapping_tests.rs`

### Tests for User Story 5 — F-13 (correlation ID uniqueness) ⚠️

- [x] T023 [P] [US5] Unit test: handshake IDs match `intercom-{purpose}-{uuid}` pattern (S022) in `tests/unit/correlation_id_uniqueness.rs`
- [x] T024 [P] [US5] Unit test: runtime prompt IDs match UUID pattern (S023) in `tests/unit/correlation_id_uniqueness.rs`
- [x] T025 [P] [US5] Unit test: 10,000 IDs with zero collisions (S024) in `tests/unit/correlation_id_uniqueness.rs`
- [x] T026 [P] [US5] Unit test: concurrent sessions produce distinct IDs (S026) in `tests/unit/correlation_id_uniqueness.rs`

### Implementation for User Story 5 — F-10

- [x] T027 [US5] Remove `channel_id` extraction from `update_pending_from_uri()` in `src/mcp/sse.rs` — only extract `session_id` and `workspace_id`
- [x] T028 [US5] Change `PendingParams` type from 3-tuple to 2-tuple `(Option<String>, Option<String>)` for `(session_id, workspace_id)` in `src/mcp/sse.rs`
- [x] T029 [US5] Remove `raw_channel` fallback branch from factory closure in `src/mcp/sse.rs` — resolve channel exclusively via workspace mappings
- [x] T030 [US5] Update module-level doc comment in `src/mcp/sse.rs` — document `workspace_id` as the only routing query parameter
- [x] T031 [US5] Update `resolve_channel_id()` signature in `src/config.rs` — remove `channel_id` fallback parameter
- [x] T032 [US5] Update `tests/unit/workspace_routing_tests.rs` — remove `channel_id` as second arg in `resolve_channel_id` calls
- [x] T033 [US5] Update `tests/integration/channel_override_tests.rs` — rewrite for workspace_id-only routing or remove channel_id-specific tests

### Implementation for User Story 5 — F-13

- [x] T034 [US5] Replace static `INIT_ID`, `SESSION_NEW_ID`, `PROMPT_ID` constants with UUID-based generation in `src/acp/handshake.rs` — use `format!("intercom-{purpose}-{}", Uuid::new_v4())`
- [x] T035 [US5] Remove `PROMPT_COUNTER` static and replace with UUID-based generation in `src/driver/acp_driver.rs` — all `resolve_clearance` and `resolve_prompt` calls use `Uuid::new_v4()`
- [x] T036 [US5] Verify all existing tests pass after F-10 and F-13 changes — run `cargo test`

**Checkpoint**: MCP endpoint accepts only `workspace_id`; all correlation IDs are UUID-based with zero collision risk.

---

## Phase 6: User Story 4 — Mobile Operator Approval Workflow (Priority: P2)

**Goal**: Research Slack modal behavior on iOS and conditionally implement thread-reply
input fallback.

**Independent Test**: Trigger approval and prompt interactions via Slack iOS; verify all
operator actions complete successfully.

**Fixes**: F-15, F-16 (conditional), F-17 (conditional) | **Scenarios**: S027–S036 | **FRs**: FR-009–FR-013

### F-15: Research Phase

- [x] T037 [US4] Research Slack `views.open` and `plain_text_input` behavior on iOS — consult API docs, Block Kit reference, community reports
- [x] T038 [US4] Document research findings in `specs/007-acp-correctness-mobile/research-f15-mobile-modals.md` — conclude with one of: (a) modals work, (b) input broken, (c) modals swallowed

**Gate**: If finding is (a), skip T039–T050. If (b) or (c), proceed.

### F-16/F-17: Thread-Reply Fallback (Conditional) ⚠️

> **These tasks are CONDITIONAL on F-15 research finding (b) or (c)**

#### Tests (conditional)

- [x] T039 [P] [US4] Unit test: modal failure triggers thread-reply fallback message (S029) in `tests/unit/thread_reply_fallback.rs`
- [x] T040 [P] [US4] Unit test: thread reply captured and routed to waiting oneshot (S030) in `tests/unit/thread_reply_fallback.rs`
- [x] T041 [P] [US4] Unit test: acknowledgment posted after reply capture (S031) in `tests/unit/thread_reply_fallback.rs`
- [x] T042 [P] [US4] Unit test: multiple replies — only first captured (S032) in `tests/unit/thread_reply_fallback.rs`
- [x] T043 [P] [US4] Unit test: unauthorized user reply rejected (S033) in `tests/unit/thread_reply_fallback.rs`
- [x] T044 [P] [US4] Integration test: full fallback flow for prompt refine (S029→S030→S031) in `tests/integration/thread_reply_integration.rs`

#### Implementation (conditional)

- [x] T045 [US4] Add `pending_thread_replies` map to `AppState` in `src/mcp/handler.rs`
- [x] T046 [US4] Create `src/slack/handlers/thread_reply.rs` — handler for detecting and routing thread replies to waiting interactions
- [x] T047 [US4] Add thread-reply fallback path to `handle_prompt_action` in `src/slack/handlers/prompt.rs` — when `open_modal` fails, post fallback message and register pending reply
- [x] T048 [US4] Add thread-reply fallback path to `handle_wait_action` in `src/slack/handlers/wait.rs` — mirror prompt fallback logic
- [x] T049 [US4] Add thread-reply fallback path to `handle_approval_action` in `src/slack/handlers/approval.rs` — for rejection reason input
- [x] T050 [US4] Subscribe to `message` events in Slack Socket Mode — route thread replies to `thread_reply::handle_thread_reply` in `src/main.rs`
- [x] T051 [US4] Verify all existing tests pass after F-16/F-17 changes — run `cargo test`

**Checkpoint**: Mobile operators can complete all approval and prompt interactions via Slack iOS.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final validation and documentation updates.

- [x] T052 [P] Run full quality gate: `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`
- [x] T053 [P] Run format check: `cargo fmt --all -- --check`
- [x] T054 Run full test suite and verify 959+ baseline tests still pass plus new tests: `cargo test --all-targets`
- [x] T055 Update `specs/007-acp-correctness-mobile/spec.md` — update FR-007 description to reflect `channel_id` removal instead of deprecation warning
- [x] T056 [P] Update `.context/backlog.md` — mark F-06, F-07, F-10, F-13 as complete

---

## Phase 8: Technical Debt — Fallback Hardening (Post-Review Deferred Items)

**Purpose**: Address the five MEDIUM/LOW findings deferred during the PR adversarial review.
These are all contained within the thread-reply fallback mechanism (F-16/F-17) and the
`deliver_queued_messages` pipeline (F-06). No new features — correctness and maintainability only.

### TQ-008 — Extract shared fallback helper

- [x] T057 [US4] Extract duplicated fallback logic from `prompt.rs`, `wait.rs`, and `approval.rs` into a shared `spawn_thread_reply_fallback` function in `src/slack/handlers/thread_reply.rs`. The helper should accept the resolution callback as a boxed async closure or via an enum discriminant. Reduces triplication and ensures timeout/zombie-guard/cleanup logic stays in sync across handlers.

### TQ-009 — Push_event integration tests for negative paths

- [x] T058 [US4] Add `tests/integration/push_events_thread_reply_tests.rs` (or extend existing) covering:
  - Unauthorized user reply is silently ignored (entry stays registered)
  - Timeout expiry: spawned task exits after `FALLBACK_REPLY_TIMEOUT` without panicking
  - Duplicate registration for same composite key: second `register_thread_reply_fallback` call on same key logs a warning and returns without overwriting, OR documents overwrite behavior explicitly

### LC-05 — StreamActivity emitted for failed deliveries

- [x] T059 [US1] In `src/acp/reader.rs`, have `deliver_queued_messages` return a count of successfully delivered messages. Change the `StreamActivity` emission loop (line ~507) to emit only for the count of successfully delivered messages, not for all queued messages. Add/update unit test in `tests/unit/acp_reader_steering_delivery.rs` to verify `StreamActivity` is NOT emitted for failed deliveries.

### LC-04 — Silent overwrite on duplicate fallback registration

- [x] T060 [US4] In `src/slack/handlers/thread_reply.rs`, change `register_thread_reply_fallback` to check for an existing entry before inserting. If a key already exists, log a `warn!` with the channel and thread_ts, drop the new sender (sends `RecvError` to the new `rx`), and return without overwriting. Add a unit test in `tests/unit/thread_reply_fallback.rs` verifying the original entry survives a duplicate registration attempt.

### CS-06 — Hardcoded SQL status strings

- [x] T061 [P] In `src/persistence/session_repo.rs`, replace the inline string literals `'active'`, `'created'`, `'paused'` in `count_active_acp` with `SessionStatus` enum `.as_str()` calls (or equivalent constant references) to match the pattern used elsewhere in the repo. Verify no other queries in `session_repo.rs` use raw string literals for status values; update those too.

**Checkpoint**: All five deferred items resolved, 1,032+ tests passing, clippy clean.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: Empty — no setup needed
- **Phase 2 (Foundational)**: Empty — no shared prerequisites
- **Phase 3 (US1 / F-06)**: Independent — can start immediately
- **Phase 4 (US2 / F-07)**: Independent — can start immediately
- **Phase 5 (US5 / F-10 + F-13)**: Independent — can start immediately
- **Phase 6 (US4 / F-15 + conditional)**: Independent — can start immediately; has internal gate at T038
- **Phase 7 (Polish)**: Depends on Phases 3–6 completion

### User Story Dependencies

- **US1 (P1)**: No dependencies — fully self-contained in `src/acp/reader.rs`
- **US2 (P1)**: No dependencies — self-contained in `session_repo.rs` + `commands.rs`
- **US5 (P2)**: No dependencies — F-10 and F-13 are in separate modules; can be done together or sequentially
- **US4 (P2)**: Internal dependency: T037→T038 gate determines if T039–T051 execute

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Implementation code makes tests pass
- Full cargo test verification after each story

### Parallel Opportunities

- **All four user stories can run in parallel** — they touch completely separate files
- Within US5: F-10 tests (T018–T022) and F-13 tests (T023–T026) are parallelizable
- Within US4: All conditional tests (T039–T044) are parallelizable

---

## Parallel Example: All User Stories

```text
# All user stories can be started simultaneously:
Phase 3 (US1 / F-06): T001→T006 (tests) → T007 (impl) → T008 (verify)
Phase 4 (US2 / F-07): T009→T014 (tests) → T015→T016 (impl) → T017 (verify)
Phase 5 (US5 / F-10+F-13): T018→T026 (tests) → T027→T035 (impl) → T036 (verify)
Phase 6 (US4 / F-15): T037→T038 (research) → [gate] → T039→T051 (conditional)
```

---

## Implementation Strategy

### MVP First (US1 + US2 — P1 Stories)

1. Complete Phase 3: US1 (F-06 steering fix) — highest data integrity impact
2. Complete Phase 4: US2 (F-07 capacity fix) — highest resource safety impact
3. **STOP and VALIDATE**: Run full test suite
4. These two fixes address the most critical correctness issues

### Incremental Delivery

1. US1 (F-06) → Test independently → Commit
2. US2 (F-07) → Test independently → Commit
3. US5 (F-10 + F-13) → Test independently → Commit
4. US4 (F-15) → Research → Gate decision → Conditional implementation → Commit
5. Polish (Phase 7) → Final validation → PR ready

---

## Task Summary

| Phase | User Story | Fix IDs | Task Count | Tests | Implementation |
|---|---|---|---|---|---|
| 3 | US1 — Steering Delivery | F-06 | 8 | T001–T006 | T007–T008 |
| 4 | US2 — Capacity Enforcement | F-07 | 9 | T009–T014 | T015–T017 |
| 5 | US5 — Protocol Hygiene | F-10, F-13 | 19 | T018–T026 | T027–T036 |
| 6 | US4 — Mobile Accessibility | F-15, F-16, F-17 | 15 | T039–T044 | T037–T038, T045–T051 |
| 7 | Polish | — | 5 | — | T052–T056 |
| **Total** | | | **56** | **25** | **31** |

**Notes**:
- T039–T051 (13 tasks) are conditional on F-15 research outcome
- If F-15 finding is (a) "modals work", effective total is 43 tasks
- All 25 test tasks are marked [P] (parallelizable within their phase)
