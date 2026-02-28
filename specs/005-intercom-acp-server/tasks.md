# Tasks: Intercom ACP Server

**Input**: Design documents from `/specs/005-intercom-acp-server/`
**Prerequisites**: plan.md, spec.md, SCENARIOS.md, data-model.md, contracts/

**Tests**: TDD is required per constitution Principle III. Test tasks precede implementation in each phase.

**Organization**: Tasks grouped by user story for independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Project initialization and new module scaffolding

- [x] T001 Create `src/driver/mod.rs` with `AgentDriver` trait definition, `AgentEvent` enum, and module doc comments
- [x] T002 [P] Create `src/acp/mod.rs` with module structure and doc comments
- [x] T003 [P] Add `AppError::Acp(String)` variant in `src/errors.rs` with `Display` and `From` implementations
- [x] T004 [P] Add `ProtocolMode` enum (`Mcp`, `Acp`) to `src/models/session.rs` with serde serialization
- [x] T004b [P] Enable `codec` feature on `tokio-util` in `Cargo.toml` — change `features = ["rt"]` to `features = ["rt", "codec"]` for `LinesCodec`/`FramedRead`/`FramedWrite` support

**Checkpoint**: New module stubs exist, project compiles with `cargo check`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Tests

- [x] T005 [P] Write unit test for `ProtocolMode` serde round-trip in `tests/unit/session_model_tests.rs`
- [x] T006 [P] Write unit test for `AppError::Acp` display format in `tests/unit/error_tests.rs`
- [x] T007 [P] Write unit test for `AgentEvent` enum construction and field access in `tests/unit/driver_trait_tests.rs`

### Implementation

- [x] T008 Add `protocol_mode`, `channel_id`, `thread_ts`, `connectivity_status`, `last_activity_at`, `restart_of` fields to `Session` struct in `src/models/session.rs`
- [x] T009 Write idempotent schema migration in `src/persistence/schema.rs` — add `protocol_mode`, `channel_id`, `thread_ts`, `connectivity_status`, `last_activity_at`, `restart_of` columns to `session` table using PRAGMA table_info check
- [x] T010 Update `SessionRepo` in `src/persistence/session_repo.rs` — include new fields in INSERT/SELECT/UPDATE queries
- [x] T011 [P] Add `find_active_by_channel(channel_id)` query to `src/persistence/session_repo.rs`
- [x] T012 [P] Add `find_by_channel_and_thread(channel_id, thread_ts)` query to `src/persistence/session_repo.rs`
- [x] T013 [P] Add `set_thread_ts(session_id, thread_ts)` update method to `src/persistence/session_repo.rs`
- [x] T014 Create new index `idx_session_channel` and `idx_session_channel_thread` in `src/persistence/schema.rs`

**Checkpoint**: Foundation ready — Session model has new fields, schema migrates cleanly, repo queries work

---

## Phase 3: User Story 1 — Dual-Mode Startup (Priority: P1) 🎯 MVP

**Goal**: Add `--mode` CLI flag to select MCP or ACP mode at startup

**Independent Test**: Start server with `--mode mcp` and `--mode acp`, verify each mode initializes correctly

### Tests (S001–S007)

- [x] T015 [P] [US1] Write unit test for `Mode` enum CLI parsing (mcp default, acp explicit, invalid) in `tests/unit/cli_tests.rs` — covers S001, S002, S003, S006
- [x] T016 [P] [US1] Write unit test for ACP config validation (missing host_cli) in `tests/unit/config_tests.rs` — covers S004, S005

### Implementation

- [x] T017 [US1] Add `Mode` enum (`Mcp`, `Acp`) with `ValueEnum` derive to `src/main.rs` CLI struct
- [x] T018 [US1] Add `--mode` flag to `Cli` struct in `src/main.rs` with `default_value_t = Mode::Mcp`
- [x] T019 [US1] Add ACP config validation in `src/config.rs` — validate `host_cli` is non-empty and exists when ACP mode selected
- [x] T020 [US1] Branch `run()` in `src/main.rs` on mode: MCP path unchanged, ACP path skips MCP transport startup
- [x] T021 [US1] Verify MCP mode regression — all 9 tools visible and functional with no changes (S007)

**Checkpoint**: Server starts in MCP or ACP mode; MCP behavior unchanged; ACP validates config

---

## Phase 4: User Story 2 — Agent Driver Abstraction (Priority: P1)

**Goal**: Protocol-agnostic `AgentDriver` trait with MCP implementation wrapping existing oneshot pattern

**Independent Test**: Mock driver resolves clearance/prompt/wait requests identically to current behavior

### Tests (S008–S017)

- [x] T022 [P] [US2] Write contract test for `McpDriver::resolve_clearance` approved/rejected in `tests/contract/driver_contract_tests.rs` — covers S008, S009
- [x] T023 [P] [US2] Write contract test for `resolve_clearance` with unknown request_id in `tests/contract/driver_contract_tests.rs` — covers S012
- [x] T024 [P] [US2] Write unit test for driver `interrupt` on terminated session (idempotent) in `tests/unit/driver_trait_tests.rs` — covers S016
- [x] T025 [P] [US2] Write concurrent resolution test (two requests resolved simultaneously) in `tests/unit/driver_trait_tests.rs` — covers S017

### Implementation

- [x] T026 [US2] Define `AgentDriver` trait in `src/driver/mod.rs` with 5 methods: `resolve_clearance`, `send_prompt`, `interrupt`, `resolve_prompt`, `resolve_wait`
- [x] T027 [US2] Implement `McpDriver` in `src/driver/mcp_driver.rs` — wraps existing `PendingApprovals`, `PendingPrompts`, `PendingWaits` oneshot maps
- [x] T028 [US2] Wire `McpDriver` as `Arc<dyn AgentDriver>` into `AppState` in `src/mcp/handler.rs`
- [x] T029 [US2] Refactor Slack approval handler in `src/slack/handlers/` to call `driver.resolve_clearance()` instead of directly accessing oneshot maps
- [x] T030 [US2] Refactor Slack prompt handler to call `driver.resolve_prompt()` instead of directly accessing oneshot maps
- [x] T031 [US2] Refactor Slack wait handler to call `driver.resolve_wait()` instead of directly accessing oneshot maps

**Checkpoint**: All Slack handlers route through AgentDriver trait; MCP behavior identical to before refactor

---

## Phase 5: User Story 3 — ACP Session Lifecycle via Slack (Priority: P1)

**Goal**: Start, monitor, and terminate ACP agent sessions from Slack commands

**Independent Test**: `/intercom session-start` in Slack spawns agent, status updates appear in Slack thread

### Tests (S018–S026)

- [ ] T032 [P] [US3] Write integration test for ACP session start (spawn + initial prompt) in `tests/integration/acp_lifecycle_tests.rs` — covers S018
- [ ] T033 [P] [US3] Write unit test for ACP session stop (process kill + status update) in `tests/unit/acp_session_tests.rs` — covers S021
- [ ] T034 [P] [US3] Write unit test for agent process crash handling in `tests/unit/acp_session_tests.rs` — covers S023
- [ ] T035 [P] [US3] Write unit test for startup timeout when agent never responds in `tests/unit/acp_session_tests.rs` — covers S025
- [ ] T036 [P] [US3] Write boundary test for empty prompt rejection in `tests/unit/acp_session_tests.rs` — covers S026

### Implementation

- [ ] T037 [US3] Create ACP spawner in `src/acp/spawner.rs` — spawn `host_cli` process with `kill_on_drop(true)`, `env_clear()` + safe variable allowlist (PATH, HOME, RUST_LOG), capture stdin/stdout handles, return `AcpConnection`
- [ ] T037b [P] [US3] Write unit test verifying spawned process does NOT inherit SLACK_BOT_TOKEN or SLACK_APP_TOKEN in `tests/unit/acp_session_tests.rs` — covers S075
- [ ] T038 [US3] Wire ACP session-start in `src/slack/commands.rs` — when mode is ACP, spawn agent via ACP spawner, pass `channel_id` from originating Slack channel context (FR-027), register session with AcpDriver
- [ ] T039 [US3] Implement process exit monitoring in `src/acp/spawner.rs` — `tokio::spawn` task that awaits `child.wait()` and emits `AgentEvent::SessionTerminated`
- [ ] T040 [US3] Implement startup timeout — if no message from agent within `startup_timeout_seconds`, kill process and emit failure event
- [ ] T041 [US3] Handle max concurrent sessions check in ACP session-start path (S024)

**Checkpoint**: ACP sessions start/stop from Slack; process crashes are detected and reported

---

## Phase 6: User Story 4 — Workspace-to-Channel Mapping (Priority: P2)

**Goal**: Centralized workspace-to-channel mapping in `config.toml` replaces per-workspace `channel_id` query params

**Independent Test**: Configure workspace mapping, connect with `workspace_id`, verify messages route to correct channel

### Tests (S027–S035)

- [ ] T042 [P] [US4] Write unit test for workspace mapping config parsing in `tests/unit/workspace_mapping_tests.rs` — covers S027, S032, S033
- [ ] T043 [P] [US4] Write unit test for workspace_id resolution (known, unknown, both params) in `tests/unit/workspace_mapping_tests.rs` — covers S027, S028, S029, S030, S031
- [ ] T044 [P] [US4] Write integration test for hot-reload of workspace mappings in `tests/integration/workspace_routing_tests.rs` — covers S034, S035

### Implementation

- [ ] T045 [US4] Add `WorkspaceMapping` struct and `[[workspace]]` TOML parsing to `src/config.rs`
- [ ] T046 [US4] Add `workspace_mappings: HashMap<String, String>` field to `GlobalConfig` in `src/config.rs`
- [ ] T047 [US4] Add workspace_id validation rules (non-empty, valid characters, no duplicates) to `src/config.rs`
- [ ] T048 [US4] Parse `workspace_id` query parameter in SSE middleware in `src/mcp/sse.rs`
- [ ] T049 [US4] Implement resolution logic in `src/mcp/sse.rs` — workspace_id lookup → channel_id, with channel_id fallback and deprecation warning
- [ ] T050 [US4] Extend `PolicyWatcher` (or create new watcher) to hot-reload workspace mappings from `config.toml` via `notify` in `src/policy/watcher.rs`
- [ ] T051 [US4] Update `config.toml.example` with `[[workspace]]` example entries

**Checkpoint**: Workspace mappings resolve correctly; legacy channel_id still works; hot-reload functional

---

## Phase 7: User Story 5 — Session Threading in Slack (Priority: P2)

**Goal**: Each session owns a dedicated Slack thread; all messages posted as threaded replies

**Independent Test**: Start two sessions, verify each gets separate Slack thread with no cross-contamination

### Tests (S036–S042)

- [ ] T052 [P] [US5] Write unit test for thread_ts recording on first Slack message in `tests/unit/session_routing_tests.rs` — covers S036
- [ ] T053 [P] [US5] Write unit test for subsequent messages using thread_ts in `tests/unit/session_routing_tests.rs` — covers S037, S038
- [ ] T054 [P] [US5] Write integration test for two concurrent sessions with separate threads in `tests/integration/thread_routing_tests.rs` — covers S041
- [ ] T055 [P] [US5] Write boundary test for thread_ts immutability in `tests/unit/session_routing_tests.rs` — covers S042

### Implementation

- [ ] T056 [US5] Add `thread_ts: Option<&str>` parameter to `SlackService::post_message` and thread-aware posting methods in `src/slack/client.rs`
- [ ] T057 [US5] Create session thread root message builder in `src/slack/blocks.rs` — formats the initial "Session started" message
- [ ] T058 [US5] Wire thread_ts recording — after first Slack message posted, call `session_repo.set_thread_ts(session_id, ts)` in Slack posting code
- [ ] T059 [US5] Update all session-scoped Slack message sends (status, clearance, broadcast, stall) to include `thread_ts` in `src/slack/client.rs` and callers
- [ ] T060 [US5] Post final "Session ended" summary as thread reply on session termination

**Checkpoint**: Each session has its own Slack thread; all messages are properly threaded

---

## Phase 8: User Story 6 — Multi-Session Channel Routing (Priority: P2)

**Goal**: Operator actions route to correct session by channel and thread context

**Independent Test**: Two sessions in different channels; operator actions in each channel route to correct session

### Tests (S043–S048)

- [ ] T061 [P] [US6] Write unit test for session lookup by channel_id in `tests/unit/session_routing_tests.rs` — covers S043, S044
- [ ] T062 [P] [US6] Write unit test for "no active session" response in `tests/unit/session_routing_tests.rs` — covers S045
- [ ] T063 [P] [US6] Write unit test for thread_ts disambiguation in `tests/unit/session_routing_tests.rs` — covers S046, S047
- [ ] T064 [P] [US6] Write concurrent test for three sessions in three channels in `tests/integration/workspace_routing_tests.rs` — covers S048

### Implementation

- [ ] T065 [US6] Refactor `store_from_slack` in `src/slack/handlers/steer.rs` to filter by `channel_id` before selecting session (RI-04 fix)
- [ ] T066 [US6] Update Slack approval handler in `src/slack/events.rs` to extract `channel_id` and `thread_ts` for routing
- [ ] T067 [US6] Update slash command handler in `src/slack/commands.rs` to scope commands to originating channel
- [ ] T068 [US6] Add "no active session" response when slash commands target a channel without sessions
- [ ] T068b [P] [US6] Write unit test for non-owner action rejection in `tests/unit/session_routing_tests.rs` — covers S076 (FR-031)
- [ ] T068c [US6] Add owner_user_id verification to Slack approval/steering handlers in `src/slack/events.rs` — reject actions from non-owners with ephemeral error message (FR-031)

**Checkpoint**: All operator actions correctly scoped by channel and thread

---

## Phase 9: User Story 7 — ACP Stream Processing (Priority: P2)

**Goal**: Reliable bidirectional NDJSON stream communication with agent processes

**Independent Test**: Send various message patterns through mock stream, verify parsing and dispatch

### Tests (S049–S058)

- [ ] T069 [P] [US7] Write unit test for single NDJSON message parsing in `tests/unit/acp_codec_tests.rs` — covers S049
- [ ] T070 [P] [US7] Write unit test for batched message parsing in `tests/unit/acp_codec_tests.rs` — covers S050
- [ ] T071 [P] [US7] Write unit test for partial delivery reassembly in `tests/unit/acp_codec_tests.rs` — covers S051
- [ ] T072 [P] [US7] Write unit test for malformed JSON handling in `tests/unit/acp_codec_tests.rs` — covers S052
- [ ] T073 [P] [US7] Write unit test for unknown method skip in `tests/unit/acp_codec_tests.rs` — covers S053
- [ ] T074 [P] [US7] Write unit test for missing required field handling in `tests/unit/acp_codec_tests.rs` — covers S054
- [ ] T075 [P] [US7] Write unit test for stream EOF → SessionTerminated in `tests/unit/acp_codec_tests.rs` — covers S055
- [ ] T076 [P] [US7] Write unit test for outbound clearance response serialization in `tests/unit/acp_codec_tests.rs` — covers S056
- [ ] T077 [P] [US7] Write boundary test for max line length exceeded in `tests/unit/acp_codec_tests.rs` — covers S057
- [ ] T078 [P] [US7] Write boundary test for empty line handling in `tests/unit/acp_codec_tests.rs` — covers S058

### Implementation

- [ ] T079 [US7] Implement NDJSON codec wrapper in `src/acp/codec.rs` using `tokio_util::codec::LinesCodec`
- [ ] T080 [US7] Implement ACP reader task in `src/acp/reader.rs` — `FramedRead` on `ChildStdout`, parse JSON, emit `AgentEvent` via mpsc channel
- [ ] T081 [US7] Implement ACP writer task in `src/acp/writer.rs` — receive outbound messages via mpsc, serialize JSON, write to `ChildStdin` via `FramedWrite`
- [ ] T082 [US7] Implement `AcpDriver` struct in `src/driver/acp_driver.rs` — holds `stream_writers: Arc<Mutex<HashMap<String, mpsc::Sender<Value>>>>` for per-session stream routing, with `register_session`/`deregister_session` lifecycle methods
- [ ] T083 [US7] Implement `AgentDriver` trait for `AcpDriver` in `src/driver/acp_driver.rs` — serialize JSON responses and write to stream
- [ ] T084 [US7] Wire ACP reader → core event loop in `src/main.rs` ACP startup path — spawn reader task, consume events

**Checkpoint**: ACP stream reliably reads/writes NDJSON messages; all parsing edge cases handled

---

## Phase 10: User Story 8 — Offline Agent Message Queuing (Priority: P3)

**Goal**: Queue operator messages for offline/disconnected agents, deliver on reconnect

**Independent Test**: Disconnect agent, send messages from Slack, reconnect, verify delivery

### Tests (S059–S062)

- [ ] T085 [P] [US8] Write unit test for steering queue when agent offline in `tests/unit/offline_queue_tests.rs` — covers S059
- [ ] T086 [P] [US8] Write integration test for queued message delivery on reconnect in `tests/integration/acp_lifecycle_tests.rs` — covers S060, S062

### Implementation

- [ ] T087 [US8] Add agent connectivity status tracking to session model (online/offline/stalled) in `src/models/session.rs`
- [ ] T088 [US8] Update steering handler in `src/slack/handlers/steer.rs` to check connectivity status and post "Agent offline — message queued" notification
- [ ] T089 [US8] Implement message flush on ACP reconnect — on stream activity resume, read all unconsumed steering messages and deliver via `driver.send_prompt`
- [ ] T090 [US8] Post "Agent back online — delivering N queued messages" notification to Slack thread

**Checkpoint**: Offline queuing works transparently; operator sees clear status indicators

---

## Phase 11: User Story 9 — ACP Stall Detection and Recovery (Priority: P3)

**Goal**: Stall detection works for ACP via stream activity monitoring; nudges sent directly on stream

**Independent Test**: Silence agent stream, verify stall detector fires and nudge message appears on stream

### Tests (S063–S068)

- [ ] T091 [P] [US9] Write unit test for ACP stream activity resetting stall timer in `tests/unit/stall_detector_tests.rs` — covers S063
- [ ] T092 [P] [US9] Write unit test for ACP nudge delivery via stream in `tests/unit/stall_detector_tests.rs` — covers S064
- [ ] T093 [P] [US9] Write unit test for nudge retry exhaustion and operator notification in `tests/unit/stall_detector_tests.rs` — covers S066
- [ ] T094 [P] [US9] Write unit test for crash with pending clearance in `tests/unit/acp_session_tests.rs` — covers S068

### Implementation

- [ ] T095 [US9] Add `StreamActivity` variant to stall detector activity source in `src/orchestrator/stall_detector.rs`
- [ ] T096 [US9] Update ACP reader task to bump `last_stream_activity` timestamp on every successful parse in `src/acp/reader.rs`
- [ ] T097 [US9] Wire stall detector to call `driver.send_prompt(session_id, nudge)` for ACP sessions instead of MCP notification
- [ ] T098 [US9] Implement session restart from Slack — kill old process, spawn new with original prompt, same thread_ts (S067)
- [ ] T099 [US9] Handle pending clearance resolution on crash — resolve as timeout, notify operator (S068)

**Checkpoint**: ACP stall detection and recovery fully functional

---

## Phase 12: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] T100 [P] Update `config.toml.example` with all new configuration sections (`[[workspace]]`, `[acp]`)
- [ ] T101 [P] Update `docs/configuration.md` (if exists) with workspace mapping and ACP mode documentation
- [ ] T102 [P] Add migration guide for `channel_id` → `workspace_id` query parameter transition
- [ ] T103 Run full regression: `cargo test` — all existing + new tests pass
- [ ] T104 Run `cargo clippy -- -D warnings` — zero warnings
- [ ] T105 Run `cargo fmt --all -- --check` — formatting clean
- [ ] T106 Validate quickstart.md against actual implementation

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user stories
- **Phase 3 (US1 — Dual-Mode Startup)**: Depends on Phase 2
- **Phase 4 (US2 — Agent Driver)**: Depends on Phase 2
- **Phase 5 (US3 — ACP Lifecycle)**: Depends on Phase 3 (mode flag) and Phase 4 (driver trait)
- **Phase 6 (US4 — Workspace Mapping)**: Depends on Phase 2 only — can parallel with Phase 3/4
- **Phase 7 (US5 — Session Threading)**: Depends on Phase 2 only — can parallel with Phase 3/4
- **Phase 8 (US6 — Channel Routing)**: Depends on Phase 7 (thread_ts) and Phase 2 (channel_id column)
- **Phase 9 (US7 — ACP Stream)**: Depends on Phase 4 (driver trait) and Phase 5 (spawner)
- **Phase 10 (US8 — Offline Queue)**: Depends on Phase 9 (stream) and feature 004 (steering queue)
- **Phase 11 (US9 — Stall Detection)**: Depends on Phase 9 (stream activity) and Phase 4 (driver)
- **Phase 12 (Polish)**: Depends on all desired user stories being complete

### User Story Dependencies

```
Phase 2 (Foundation)
  ├── Phase 3 (US1: Mode Flag) ─────────┐
  ├── Phase 4 (US2: Driver Trait) ──────┤
  │   ├── Phase 5 (US3: ACP Lifecycle) ──┤── Phase 9 (US7: Stream) ── Phase 10 (US8: Offline)
  │   └── Phase 11 (US9: Stall)  ────────┘                           Phase 11 (US9: Stall)
  ├── Phase 6 (US4: Workspace Mapping) [parallel with 3/4]
  └── Phase 7 (US5: Threading) ── Phase 8 (US6: Channel Routing)
```

### Parallel Opportunities

- **Phase 3 + Phase 6 + Phase 7**: Mode flag, workspace mapping, and threading can all run in parallel after Phase 2
- **Within each phase**: All tasks marked [P] can run in parallel
- **All test tasks marked [P]** within a phase can run in parallel

---

## Implementation Strategy

### MVP First (User Stories 1 + 2)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: Dual-Mode Startup (US1)
4. Complete Phase 4: Agent Driver Abstraction (US2)
5. **STOP and VALIDATE**: MCP mode works identically; ACP mode validates config

### Core ACP (Add US3 + US7)

6. Complete Phase 5: ACP Session Lifecycle (US3)
7. Complete Phase 9: ACP Stream Processing (US7)
8. **STOP and VALIDATE**: Full ACP session works end-to-end

### Multi-Workspace (Add US4 + US5 + US6)

9. Complete Phase 6 + 7 + 8: Workspace Mapping, Threading, Channel Routing
10. **STOP and VALIDATE**: Multiple workspaces route correctly, sessions threaded

### Reliability (Add US8 + US9)

11. Complete Phase 10 + 11: Offline Queuing, Stall Detection
12. Complete Phase 12: Polish
13. **FINAL VALIDATION**: Full regression, clippy, fmt

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- TDD required: write tests first, verify they fail, then implement
- Commit after each task or logical group
- Total: 112 tasks across 12 phases
- **Deferred**: `ctl/main.rs` ACP subcommands and `src/ipc/server.rs` ACP extensions are deferred to a future feature. ACP sessions are managed exclusively via Slack in this feature.
