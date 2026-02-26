# Tasks: Intercom Advanced Features

**Input**: Design documents from `/specs/004-intercom-advanced-features/`
**Prerequisites**: plan.md, spec.md, SCENARIOS.md, data-model.md, research.md, quickstart.md

**Tests**: TDD required per constitution Principle III. Tests written first, verified to fail, then implementation.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story (US1-US15)

---

## Phase 1: Setup

**Purpose**: Project initialization — new modules and schema

- [X] T001 Add `steering_message` and `task_inbox` DDL to `src/persistence/schema.rs`
- [X] T002 [P] Create `src/models/steering.rs` with `SteeringMessage` struct
- [X] T003 [P] Create `src/models/inbox.rs` with `TaskInboxItem` struct
- [X] T004 [P] Create `src/audit/mod.rs` with `AuditLogger` trait and `AuditEntry` struct
- [X] T005 [P] Create `src/audit/writer.rs` with `JsonlAuditWriter` (daily rotation)
- [X] T006 Register new modules in `src/models/mod.rs` and create `src/audit/` module

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Repository layer, compiled policy, AppState wiring — MUST complete before user stories

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T007 [P] Create `src/persistence/steering_repo.rs` — insert, fetch_unconsumed, mark_consumed, purge
- [X] T008 [P] Create `src/persistence/inbox_repo.rs` — insert, fetch_unconsumed_by_channel, mark_consumed, purge
- [X] T009 [P] Add `CompiledWorkspacePolicy` struct to `src/models/policy.rs` (wraps `WorkspacePolicy` + `RegexSet`)
- [X] T010 Update `src/policy/loader.rs` — `load()` returns `CompiledWorkspacePolicy` with pre-compiled `RegexSet`
- [X] T011 Add `slack_detail_level` field to `GlobalConfig` in `src/config.rs` (minimal/standard/verbose, default standard)
- [X] T012 Wire `PolicyCache` and `AuditLogger` into `AppState` in `src/mcp/handler.rs`
- [X] T013 Register new repos in `src/persistence/mod.rs`

**Checkpoint**: Foundation ready — user story implementation can begin

---

## Phase 3: User Story 1 — Operator Steering Queue (Priority: P1) 🎯 MVP

**Goal**: Operators can send messages to running agents proactively; delivered via `ping`

**Independent Test**: Send a Slack message, call `ping`, verify message appears in response

### Tests for US1

> **Write these tests FIRST, verify they FAIL before implementation**

- [X] T014 [P] [US1] Unit test for steering_repo CRUD in `tests/unit/steering_repo_tests.rs` (scenarios S001-S005, S010-S011)
- [X] T015 [P] [US1] Unit test for steering message routing by channel in `tests/unit/steering_repo_tests.rs` (scenario S007)
- [X] T016 [P] [US1] Contract test for extended `ping` response with `pending_steering` in `tests/contract/ping_contract_tests.rs` (scenarios S002-S003)
- [X] T017 [P] [US1] Integration test for end-to-end steering flow in `tests/integration/steering_flow_tests.rs` (scenarios S001-S009)

### Implementation for US1

- [X] T018 [US1] Update `src/mcp/tools/heartbeat.rs` — fetch unconsumed steering messages, include `pending_steering` in response, mark consumed
- [X] T019 [US1] Add `/intercom steer <text>` slash command handler in `src/slack/commands.rs`
- [X] T020 [US1] Create `src/slack/handlers/steer.rs` — steering message ingestion from Slack (app mentions)
- [X] T021 [US1] Add `steer` IPC command in `src/ipc/server.rs`
- [X] T022 [US1] Add `steer` subcommand to `ctl/main.rs`
- [X] T023 [US1] Wire steering handlers into `src/slack/events.rs` (app mention → steer handler)

**Checkpoint**: Steering queue fully functional — ping delivers operator messages

---

## Phase 4: User Story 2 — Server Startup Reliability (Priority: P1)

**Goal**: Server exits cleanly on port conflict; single-instance enforcement

**Independent Test**: Start two instances, verify second exits with clear error

### Tests for US2

- [ ] T024 [P] [US2] Integration test for bind failure in `tests/integration/startup_tests.rs` (scenarios S024-S026)
- [ ] T025 [P] [US2] Integration test for normal startup in `tests/integration/startup_tests.rs` (scenario S023)

### Implementation for US2

- [ ] T026 [US2] Update `src/main.rs` — if HTTP transport bind fails, log error and `std::process::exit(1)`
- [ ] T027 [US2] Update `src/main.rs` — shut down already-started services (Slack) before exit on bind failure

**Checkpoint**: No more zombie processes on port conflict

---

## Phase 5: User Story 3 — Task Inbox (Priority: P2)

**Goal**: Operators queue work items when no agent is running; delivered at session start via `reboot`

**Independent Test**: Queue task via Slack, start session, verify task appears in reboot response

### Tests for US3

- [ ] T028 [P] [US3] Unit test for inbox_repo CRUD in `tests/unit/inbox_repo_tests.rs` (scenarios S013-S014, S019-S020)
- [ ] T029 [P] [US3] Unit test for channel-scoped delivery in `tests/unit/inbox_repo_tests.rs` (scenario S017)
- [ ] T030 [P] [US3] Contract test for extended `reboot` response with `pending_tasks` in `tests/contract/reboot_contract_tests.rs` (scenarios S015-S016)
- [ ] T031 [P] [US3] Integration test for inbox flow in `tests/integration/inbox_flow_tests.rs` (scenarios S013-S022)

### Implementation for US3

- [ ] T032 [US3] Update `src/mcp/tools/recover_state.rs` — fetch unconsumed inbox items by channel, include `pending_tasks`, mark consumed
- [ ] T033 [US3] Add `/intercom task <text>` slash command handler in `src/slack/commands.rs`
- [ ] T034 [US3] Add `task` IPC command in `src/ipc/server.rs`
- [ ] T035 [US3] Add `task` subcommand to `ctl/main.rs`

**Checkpoint**: Task inbox operational — cold-start work queuing works

---

## Phase 6: User Story 4 — Slack Modal Instruction Capture (Priority: P2)

**Goal**: `standby` and `transmit` deliver real operator-typed instructions instead of placeholder strings

**Independent Test**: Press "Resume with Instructions", type text, submit; agent receives exact text

### Tests for US4

- [ ] T036 [P] [US4] Unit test for modal view builder in `tests/unit/blocks_tests.rs` (scenario S029)
- [ ] T037 [P] [US4] Contract test for `standby` with real instruction text in `tests/contract/wait_contract_tests.rs` (scenario S030)
- [ ] T038 [P] [US4] Contract test for `transmit` refine with real text in `tests/contract/prompt_contract_tests.rs` (scenario S032)

### Implementation for US4

- [ ] T039 [US4] Add modal view builder (text input block) in `src/slack/blocks.rs`
- [ ] T040 [US4] Update `src/slack/handlers/wait.rs` — extract `trigger_id`, call `views.open`, store session in `private_metadata`
- [ ] T041 [US4] Update `src/slack/handlers/prompt.rs` — same `trigger_id` → modal flow for "Refine"
- [ ] T042 [US4] Add `ViewSubmission` match arm in `src/slack/events.rs` — extract text, resolve oneshot
- [ ] T043 [US4] Thread `trigger_id` from `BlockActions` payload into handler functions

**Checkpoint**: No more placeholder strings — real operator instructions flow through

---

## Phase 7: User Story 5 — SSE Disconnect Session Cleanup (Priority: P2)

**Goal**: Disconnected sessions marked terminated, not left as active indefinitely

**Independent Test**: Connect agent, force-close connection, verify session status changes

### Tests for US5

- [ ] T044 [P] [US5] Integration test for disconnect detection in `tests/integration/disconnect_tests.rs` (scenarios S037-S039)

### Implementation for US5

- [ ] T045 [US5] Hook stream close event in `src/mcp/sse.rs` — trigger `session_repo.set_terminated()` on connection drop
- [ ] T046 [US5] Ensure session lookup by transport session ID is available for cleanup

**Checkpoint**: Stale sessions cleaned up promptly on disconnect

---

## Phase 8: User Story 6 + 13 — Policy Hot-Reload + Regex Pre-Compilation (Priority: P2/P4)

**Goal**: Policy changes take effect immediately; regex patterns pre-compiled for performance

**Independent Test**: Modify settings.json, verify next auto_check reflects new rules without restart

### Tests for US6/US13

- [ ] T047 [P] [US6] Unit test for `CompiledWorkspacePolicy` creation in `tests/unit/policy_tests.rs` (scenarios S074-S079)
- [ ] T048 [P] [US6] Unit test for `PolicyEvaluator::check()` with `CompiledWorkspacePolicy` in `tests/unit/policy_evaluator_tests.rs` (scenario S075)
- [ ] T049 [P] [US6] Contract test for `auto_check` reading from cache in `tests/contract/auto_check_contract_tests.rs` (scenarios S043-S044)
- [ ] T050 [P] [US6] Unit test for invalid regex handling in `tests/unit/policy_tests.rs` (scenario S076)

### Implementation for US6/US13

- [ ] T051 [US6] Update `src/policy/evaluator.rs` — replace `match_command_pattern` with `RegexSet::matches()` on `CompiledWorkspacePolicy`
- [ ] T052 [US6] Wire `PolicyCache` into `AppState` reads in `src/mcp/tools/check_auto_approve.rs`
- [ ] T053 [US6] Update `src/policy/watcher.rs` — ensure cache stores `CompiledWorkspacePolicy`

**Checkpoint**: Policy hot-reload end-to-end, regex pre-compiled

---

## Phase 9: User Story 7 + 8 — Audit Logging + Failure Reporting (Priority: P3)

**Goal**: All interactions audited in JSONL; agent failures reported to Slack

**Independent Test**: Run session with tool calls, inspect audit log; simulate stall, verify Slack notification

### Tests for US7/US8

- [ ] T054 [P] [US7] Unit test for `JsonlAuditWriter` in `tests/unit/audit_writer_tests.rs` (scenarios S049-S057)
- [ ] T055 [P] [US7] Unit test for daily rotation in `tests/unit/audit_writer_tests.rs` (scenario S054)
- [ ] T056 [P] [US8] Unit test for stall notification in `tests/unit/stall_detector_tests.rs` (scenarios S058-S061)

### Implementation for US7/US8

- [ ] T057 [US7] Wire `AuditLogger` into tool call handlers — log after each tool call in `src/mcp/tools/mod.rs` or individual handlers
- [ ] T058 [US7] Wire `AuditLogger` into approval/rejection flow in `src/slack/handlers/approval.rs`
- [ ] T059 [US7] Wire `AuditLogger` into session lifecycle in `src/orchestrator/session_manager.rs`
- [ ] T060 [US8] Update `src/orchestrator/stall_detector.rs` — send Slack notification with session details and recovery steps on stall
- [ ] T061 [US8] Update stall notification to include actionable recovery suggestions

**Checkpoint**: Full audit trail; failures proactively reported

---

## Phase 10: User Story 10 + 11 — Detail Levels + Auto-Approve Suggestion (Priority: P3)

**Goal**: Configurable Slack message verbosity; auto-approve suggestions after manual approval

**Independent Test**: Set detail level to minimal, verify terse messages; approve command, verify suggestion

### Tests for US10/US11

- [ ] T062 [P] [US10] Unit test for detail level message filtering in `tests/unit/blocks_tests.rs` (scenarios S062-S067)
- [ ] T063 [P] [US11] Unit test for auto-approve suggestion generation in `tests/unit/command_approve_tests.rs` (scenarios S068-S073)

### Implementation for US10/US11

- [ ] T064 [US10] Update `src/slack/blocks.rs` — message builders check detail level; approvals/errors always full
- [ ] T065 [US10] Pass `slack_detail_level` from config through `SlackService` to message builders
- [ ] T066 [US11] Create `src/slack/handlers/command_approve.rs` — auto-approve suggestion flow after manual approval
- [ ] T067 [US11] Add "Add to auto-approve?" button in `src/slack/blocks.rs`
- [ ] T068 [US11] Implement regex pattern generation and write to `.intercom/settings.json`

**Checkpoint**: Slack messages respect detail level; commands can self-learn auto-approve patterns

---

## Phase 11: User Story 14 + 15 — Ping Fallback + Queue Drain (Priority: P4)

**Goal**: Ping resilient to stale sessions; shutdown drains queue unconditionally

### Tests for US14/US15

- [ ] T069 [P] [US14] Unit test for ping fallback in `tests/unit/heartbeat_tests.rs` (scenarios S080-S082)
- [ ] T070 [P] [US15] Integration test for unconditional drain in `tests/integration/shutdown_tests.rs` (scenarios S083-S086)

### Implementation for US14/US15

- [ ] T071 [US14] Update `src/mcp/tools/heartbeat.rs` — sort active sessions by `updated_at DESC`, pick first
- [ ] T072 [US15] Update `src/main.rs` — move queue drain to `shutdown_with_timeout`, run unconditionally

**Checkpoint**: Ping handles stale sessions gracefully; shutdown drains all messages

---

## Phase 12: User Story 16 — Approval File Attachment (Priority: P2)

**Goal**: Approval messages include original file content as Slack attachment for informed operator review

**Independent Test**: Call `check_clearance` with a diff for an existing file, verify Slack shows both diff and original file

### Tests for US16

> **Write these tests FIRST, verify they FAIL before implementation**

- [ ] T081 [P] [US16] Unit test for original file attachment logic in `tests/unit/ask_approval_tests.rs` (scenarios S087-S090)
- [ ] T082 [P] [US16] Unit test for graceful handling of missing/unreadable file in `tests/unit/ask_approval_tests.rs` (scenarios S091, S093)
- [ ] T083 [P] [US16] Contract test for `check_clearance` response with file attachment in `tests/contract/ask_approval_contract_tests.rs` (scenarios S087-S088)

### Implementation for US16

- [ ] T084 [US16] Update `src/mcp/tools/ask_approval.rs` — after computing `original_hash`, read original file content and upload as Slack file attachment alongside the diff
- [ ] T085 [US16] Handle new file case: skip original file upload when file does not exist (no `original_hash`)
- [ ] T086 [US16] Handle file read errors gracefully — log warning, post approval message without original attachment

**Checkpoint**: Operators see full file context alongside diffs in approval requests

---

## Phase 13: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, prompts, and final integration

- [ ] T073 [P] [US9] Create `docs/configuration.md` with comprehensive config.toml breakdown
- [ ] T074 [P] [US9] Update `README.md` with config documentation reference and updated defaults
- [ ] T075 [P] [US9] Update `config.toml.example` with correct defaults (host_cli="copilot", host_cli_args=["--sse"])
- [ ] T076 [P] [US12] Create `.github/prompts/ping-loop.prompt.md` — heartbeat loop pattern template
- [ ] T077 Add retention purge for `steering_message` and `task_inbox` in `src/persistence/retention.rs`
- [ ] T078 Run full test suite (`cargo test`) — verify all scenarios pass
- [ ] T079 Run `cargo clippy -- -D warnings` — zero warnings
- [ ] T080 Run `cargo fmt --all -- --check` — formatting compliant

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user stories
- **Phase 3-12 (User Stories)**: All depend on Phase 2 completion
  - Phase 3 (Steering) can start immediately after Phase 2
  - Phase 4 (Startup) can start in parallel with Phase 3
  - Phase 5 (Inbox) can start in parallel with Phase 3
  - Phase 6 (Modal) independent of other stories
  - Phase 7 (SSE Disconnect) independent
  - Phase 8 (Policy) independent
  - Phase 9 (Audit + Failure) depends on Phase 1 T004-T005 (audit module)
  - Phase 10 (Detail + Auto-Approve) independent
  - Phase 11 (Ping Fallback + Drain) depends on Phase 3 (heartbeat changes)
  - Phase 12 (Approval File Attachment) independent — modifies ask_approval.rs only
- **Phase 13 (Polish)**: Depends on all desired user stories

### User Story Dependencies

- **US1 (Steering)**: After Phase 2 — no story dependencies
- **US2 (Startup)**: After Phase 2 — no story dependencies
- **US3 (Inbox)**: After Phase 2 — no story dependencies
- **US4 (Modal)**: After Phase 2 — no story dependencies
- **US5 (SSE Disconnect)**: After Phase 2 — no story dependencies
- **US6+13 (Policy)**: After Phase 2 — no story dependencies
- **US7+8 (Audit + Failure)**: After Phase 2 — no story dependencies
- **US10+11 (Detail + Approve)**: After Phase 2 — no story dependencies
- **US14 (Ping Fallback)**: After Phase 3 (shares heartbeat.rs changes)
- **US15 (Queue Drain)**: After Phase 2 — no story dependencies
- **US16 (Approval File Attachment)**: After Phase 2 — no story dependencies (modifies ask_approval.rs only)

### Parallel Opportunities

- Phase 1: T002-T005 all parallel (different files)
- Phase 2: T007-T009 all parallel (different files)
- Phase 3+: Most user stories can run in parallel after Phase 2
- Within each story: test tasks marked [P] can run in parallel

---

## Implementation Strategy

### MVP First (Phase 3: Steering Queue Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: US1 — Steering Queue
4. **STOP and VALIDATE**: Test steering queue independently
5. Deploy/demo — operators can steer running agents

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add Steering Queue (US1) → MVP!
3. Add Server Startup (US2) → Reliability
4. Add Task Inbox (US3) → Cold-start workflow
5. Add Slack Modal (US4) → Real instructions
6. Continue by priority...

---

## Summary

| Metric | Count |
|---|---|
| Total tasks | 86 |
| Phase 1 (Setup) | 6 |
| Phase 2 (Foundational) | 7 |
| Phase 3 (Steering - MVP) | 10 |
| Phase 4 (Startup) | 4 |
| Phase 5 (Inbox) | 8 |
| Phase 6 (Modal) | 8 |
| Phase 7 (SSE Disconnect) | 3 |
| Phase 8 (Policy) | 7 |
| Phase 9 (Audit + Failure) | 8 |
| Phase 10 (Detail + Approve) | 7 |
| Phase 11 (Ping + Drain) | 4 |
| Phase 12 (Approval File Attachment) | 6 |
| Phase 13 (Polish) | 8 |

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story
- Each user story independently completable and testable
- TDD: write tests first, verify they fail, then implement
- Commit after each task or logical group
