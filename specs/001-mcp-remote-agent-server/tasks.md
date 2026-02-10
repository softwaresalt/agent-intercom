---
title: Tasks: MCP Remote Agent Server
description: Task list for the MCP remote agent server implementation
ms.date: 2026-02-09
ms.topic: reference
---

## Tasks: MCP Remote Agent Server

**Input**: Design documents from `/specs/001-mcp-remote-agent-server/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Not included — tests were not explicitly requested in the feature specification. Add test phases per user story if TDD is desired.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `ctl/`, `tests/` at repository root
- Two binary targets: `monocoque-agent-rem` (server) and `monocoque-ctl` (CLI)
- Config: `config.toml` at project root or `~/.config/monocoque/config.toml`
- Workspace policy: `.monocoque/settings.json` inside workspace root

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization, Cargo workspace, directory skeleton

- [x] T001 Create Cargo workspace with two binary targets (`monocoque-agent-rem` in src/main.rs, `monocoque-ctl` in ctl/main.rs) in Cargo.toml
- [x] T002 Add all dependencies to Cargo.toml: rmcp (features: server, transport-sse-server, transport-io), slack-morphism (features: hyper, socket-mode), axum 0.8, tokio (features: full), serde/serde_json, diffy 0.4, notify, tracing/tracing-subscriber, surrealdb (features: kv-rocksdb, kv-mem), sha2, tempfile, interprocess (features: tokio), uuid (features: v4, serde), chrono (features: serde), clap (features: derive), toml
- [x] T003 Create full directory structure with stub mod.rs module declarations per plan.md: src/models/, src/mcp/tools/, src/mcp/resources/, src/slack/, src/persistence/, src/orchestrator/, src/policy/, src/diff/, src/ipc/, ctl/, tests/contract/, tests/integration/, tests/unit/
- [x] T004 [P] Add rustfmt.toml (max_width=100, edition=2021) and configure clippy lints in Cargo.toml

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**CRITICAL**: No user story work can begin until this phase is complete

- [ ] T005 Implement GlobalConfig TOML parsing with all fields (workspace_root, slack tokens, channel_id, authorized_user_ids, max_concurrent_sessions, timeouts, stall config, host CLI, commands map, http_port, ipc_name) in src/config.rs
- [ ] T006 [P] Define shared error types (AppError enum with variants for config, db, slack, path_violation, patch_conflict, not_found, unauthorized, already_consumed), Result type alias, and module re-exports in src/lib.rs
- [ ] T007 [P] Configure tracing-subscriber with structured JSON logging and env-filter in src/main.rs
- [ ] T008 Implement SurrealDB embedded connection (kv-rocksdb for production, kv-mem for tests), namespace/database setup, and schema DDL with DEFINE TABLE/FIELD for Session, ApprovalRequest, Checkpoint, ContinuationPrompt, StallAlert in src/persistence/db.rs
- [ ] T009 [P] Implement Session model with status enum (Created, Active, Paused, Terminated, Interrupted), mode enum (Remote, Local, Hybrid), validation rules, and serde derives in src/models/session.rs
- [ ] T010 Implement Session SurrealDB repository: create, get_by_id, update_status, update_last_activity, list_active, count_active in src/persistence/session_repo.rs
- [ ] T011 [P] Implement path validation utility: canonicalize path, verify starts_with(workspace_root), reject traversal attempts, return AppError::PathViolation on failure in src/diff/mod.rs
- [ ] T012 Implement MCP ServerHandler trait with tool_list (all 9 tools) and call_tool router dispatching to individual tool handler modules in src/mcp/server.rs
- [ ] T013 [P] Implement Slack Socket Mode client: WebSocket connection lifecycle, auto-reconnect via SlackSocketModeClientsManager, on_message/on_error/on_disconnect hooks in src/slack/client.rs
- [ ] T014 [P] Implement base Slack Block Kit builders: text_section, button_action, actions_block, code_block, rich_text_preformatted, divider, context_block in src/slack/blocks.rs
- [ ] T015 Implement Slack interaction event handler scaffold: route block_actions and view_submission payloads by action_id, extract session_id/request_id from button values in src/slack/events.rs
- [ ] T016 [P] Implement Axum HTTP/SSE transport: mount rmcp StreamableHttpService on /mcp route, bind to configurable http_port on 127.0.0.1 in src/main.rs
- [ ] T017 Wire server entry point: load config, init DB, connect Slack, start stall detector, spawn Axum server, set up stdio MCP transport, register signal handlers in src/main.rs
- [ ] T089 Implement Slack message queue with rate limit handling: in-memory queue, exponential backoff retry, respect Retry-After headers, drain queue on reconnect. All Slack-posting modules use this queue instead of direct API calls in src/slack/client.rs
- [ ] T090 [P] Implement Slack channel recent history MCP resource: fetch via conversations.history API, return messages with ts/user/text/thread_ts, register as slack://channel/{id}/recent resource in src/mcp/resources/slack_channel.rs and src/mcp/resources/mod.rs

**Checkpoint**: Foundation ready — user story implementation can now begin

---

## Phase 3: User Story 1 — Remote Code Review and Approval (Priority: P1) MVP

**Goal**: Agent submits code proposals for remote operator approval via Slack with Accept/Reject buttons

**Independent Test**: Start server, connect agent, invoke ask_approval with a sample diff, verify diff appears in Slack with actionable buttons. Tap Accept and confirm agent receives approval response.

### Implementation for User Story 1

- [ ] T018 [P] [US1] Implement ApprovalRequest model with status enum (Pending, Approved, Rejected, Expired, Consumed, Interrupted), risk_level enum (Low, High, Critical), serde derives, and validation rules in src/models/approval.rs
- [ ] T019 [US1] Implement ApprovalRequest SurrealDB repository: create, get_by_id, update_status, set_consumed, query_pending_by_session, query_by_session in src/persistence/approval_repo.rs
- [ ] T020 [US1] Implement ask_approval tool handler: validate file_path within workspace_root, compute SHA-256 of target file, create ApprovalRequest record, post diff to Slack, block via tokio::sync::oneshot until operator response or timeout in src/mcp/tools/ask_approval.rs
- [ ] T021 [US1] Implement size-adaptive diff rendering: inline rich_text_preformatted for diffs under 20 lines, files.upload as .diff snippet with syntax highlighting for diffs of 20+ lines in src/slack/blocks.rs
- [ ] T022 [US1] Implement Accept/Reject interactive button payloads (action_id: approval_accept/approval_reject, value: JSON with request_id and session_id) and response handler that resolves the blocked tool call in src/slack/events.rs
- [ ] T023 [US1] Implement approval timeout logic: tokio::time::timeout wrapping the oneshot receiver, transition status to Expired, post timeout notification to Slack in src/mcp/tools/ask_approval.rs
- [ ] T024 [US1] Implement double-submission prevention: on first button action, call chat.update to replace action buttons with static status text (approved/rejected with timestamp) in src/slack/events.rs
- [ ] T025 [US1] Register ask_approval tool in MCP ServerHandler call_tool router in src/mcp/server.rs

**Checkpoint**: Remote code review and approval workflow is functional end-to-end via Slack

---

## Phase 4: User Story 2 — Programmatic Diff Application (Priority: P1)

**Goal**: Approved code changes are applied directly to the local file system with integrity checks and atomic writes

**Independent Test**: Submit a diff for approval, approve via Slack, invoke accept_diff with the request_id. Verify file is written to disk with correct content and Slack receives confirmation.

### Implementation for User Story 2

- [ ] T026 [US2] Implement diff applicator: parse unified diffs via diffy::Patch::from_str, apply patches via diffy::apply, support full-file write mode for raw content payloads in src/diff/applicator.rs
- [ ] T027 [US2] Implement file integrity checking: compute SHA-256 via sha2 crate, compare against stored original_hash, return AppError::PatchConflict when hashes diverge (unless force=true) in src/diff/applicator.rs
- [ ] T028 [US2] Implement atomic file writes: write to tempfile::NamedTempFile in same directory, create parent directories with std::fs::create_dir_all, persist via rename in src/diff/applicator.rs
- [ ] T029 [US2] Implement accept_diff tool handler: validate request_id exists and status is Approved, invoke diff applicator, transition status to Consumed with consumed_at timestamp, handle already_consumed and not_approved errors in src/mcp/tools/accept_diff.rs
- [ ] T030 [US2] Post diff application confirmation to Slack channel (file path, bytes written, checkmark indicator) in src/mcp/tools/accept_diff.rs
- [ ] T031 [US2] Register accept_diff tool in MCP ServerHandler call_tool router in src/mcp/server.rs

**Checkpoint**: End-to-end remote approval and file application workflow complete

---

## Phase 5: User Story 4 — Agent Stall Detection and Remote Nudge (Priority: P1)

**Goal**: Server detects silent agent stalls, alerts operator via Slack, and injects continuation prompts to resume work

**Independent Test**: Connect agent, simulate tool calls, then go silent past threshold. Verify stall alert posts to Slack. Tap Nudge and verify agent receives continuation notification.

### Implementation for User Story 4

- [ ] T032 [P] [US4] Implement StallAlert model with status enum (Pending, Nudged, SelfRecovered, Escalated, Dismissed), serde derives, and validation rules in src/models/stall.rs
- [ ] T033 [US4] Implement per-session stall detector: tokio::time::interval timer, reset on any MCP activity (tool call request, tool call response, heartbeat), auto-pause during long-running server operations, configurable thresholds from GlobalConfig in src/orchestrator/stall_detector.rs
- [ ] T034 [US4] Implement heartbeat tool handler: reset stall timer for session, optionally log status_message to Slack, return acknowledged/session_id/stall_detection_enabled in src/mcp/tools/heartbeat.rs
- [ ] T035 [US4] Implement stall alert Slack messages: include last tool called, elapsed idle time, session prompt context, with Nudge/Nudge with Instructions/Stop action buttons in src/slack/blocks.rs
- [ ] T036 [US4] Implement stall interaction handler: Nudge triggers default message delivery, Nudge with Instructions opens modal for custom text, Stop terminates session in src/slack/events.rs
- [ ] T037 [US4] Implement monocoque/nudge CustomNotification delivery via context.peer.send_notification(ServerNotification::CustomNotification) with session_id, message, nudge_count, idle_seconds, source fields in src/orchestrator/stall_detector.rs
- [ ] T038 [US4] Implement auto-nudge escalation policy: after escalation_threshold_seconds without operator response, auto-nudge agent; after max_retries consecutive auto-nudges, post escalated alert with @channel mention in src/orchestrator/stall_detector.rs
- [ ] T039 [US4] Implement self-recovery detection: when agent resumes MCP activity after stall alert, call chat.update to mark alert as self-recovered and disable action buttons in src/orchestrator/stall_detector.rs
- [ ] T040 [US4] Wire stall detector into session lifecycle: start timer on session activation, stop on pause/terminate, integrate with Session.last_tool and Session.nudge_count tracking in src/orchestrator/stall_detector.rs
- [ ] T041 [US4] Register heartbeat tool in MCP ServerHandler call_tool router in src/mcp/server.rs

**Checkpoint**: All P1 stories complete — agent stalls are detected, alerted, and recoverable

---

## Phase 6: User Story 3 — Remote Status Logging (Priority: P2)

**Goal**: Agent sends non-blocking progress updates to Slack with severity-based formatting

**Independent Test**: Invoke remote_log with messages at each severity level (info, success, warning, error) and verify each appears in Slack with correct visual indicator.

### Implementation for User Story 3

- [ ] T042 [P] [US3] Implement remote_log tool handler: post message to Slack via chat.postMessage (or chat.postMessage with thread_ts for replies), return posted=true with message timestamp in src/mcp/tools/remote_log.rs
- [ ] T043 [US3] Implement severity-based visual formatting: info (plain text), success (checkmark emoji prefix), warning (warning emoji prefix), error (x emoji prefix with red sidebar via attachment color) in src/slack/blocks.rs
- [ ] T044 [US3] Register remote_log tool in MCP ServerHandler call_tool router in src/mcp/server.rs

**Checkpoint**: Agent progress is visible to operator in real time via Slack

---

## Phase 7: User Story 5 — Continuation Prompt Forwarding (Priority: P2)

**Goal**: Agent continuation prompts are forwarded to Slack with actionable response buttons, eliminating blocking meta-prompts

**Independent Test**: Invoke forward_prompt with a continuation prompt. Verify it appears in Slack with Continue/Refine/Stop buttons. Tap Continue and verify agent receives the decision.

### Implementation for User Story 5

- [ ] T045 [US5] Implement ContinuationPrompt model with prompt_type enum (Continuation, Clarification, ErrorRecovery, ResourceWarning), decision enum (Continue, Refine, Stop), serde derives in src/models/prompt.rs
- [ ] T091 [US5] Implement ContinuationPrompt SurrealDB repository: create, get_by_id, update_decision, query_pending_by_session in src/persistence/prompt_repo.rs (required for FR-007 crash recovery of pending prompts)
- [ ] T046 [US5] Implement forward_prompt tool handler: create ContinuationPrompt record, post prompt to Slack with context (elapsed_seconds, actions_taken), block via oneshot until operator response or timeout in src/mcp/tools/forward_prompt.rs
- [ ] T047 [US5] Implement Slack prompt message with Continue/Refine/Stop action buttons (action_id: prompt_continue/prompt_refine/prompt_stop, value: JSON with prompt_id and session_id) in src/slack/blocks.rs
- [ ] T048 [US5] Implement Refine modal dialog: views.open triggered from prompt_refine button, text input for revised instructions, on submission deliver refine decision with instruction text in src/slack/events.rs
- [ ] T092 [US5] Implement double-submission prevention for prompt buttons: on first button action (Continue/Refine/Stop), call chat.update to replace action buttons with static status text (decision with timestamp) in src/slack/events.rs
- [ ] T049 [US5] Implement prompt timeout with auto-continue: tokio::time::timeout wrapping oneshot, default decision=continue, post timeout notification to Slack in src/mcp/tools/forward_prompt.rs
- [ ] T050 [US5] Register forward_prompt tool in MCP ServerHandler call_tool router in src/mcp/server.rs

**Checkpoint**: Continuation prompts no longer block unattended agent sessions

---

## Phase 8: User Story 6 — Workspace Auto-Approve Policy (Priority: P2)

**Goal**: Pre-authorized safe operations bypass the remote approval gate, reducing notification noise

**Independent Test**: Create .monocoque/settings.json with auto-approve for "cargo test". Invoke check_auto_approve and verify it returns auto_approved=true. Verify operations exceeding risk threshold still require approval.

### Implementation for User Story 6

- [ ] T051 [P] [US6] Implement WorkspacePolicy model (in-memory, not persisted): parse .monocoque/settings.json, fields for enabled, commands[], tools[], file_patterns (write/read), risk_level_threshold, log_auto_approved, summary_interval_seconds in src/models/policy.rs
- [ ] T052 [US6] Implement policy evaluator: match tool_name against commands and tools lists (glob wildcard support), match file_path against file_patterns, compare risk_level against threshold in src/policy/evaluator.rs
- [ ] T053 [US6] Implement global config allowlist enforcement: workspace policy commands must exist in GlobalConfig.commands map, reject commands not in global allowlist in src/policy/evaluator.rs
- [ ] T054 [US6] Implement policy file watcher: notify crate watching .monocoque/settings.json, on modify event re-parse and hot-swap the in-memory WorkspacePolicy, fall back to require-everything on parse error with warning logged to console and Slack in src/policy/watcher.rs
- [ ] T055 [US6] Implement check_auto_approve tool handler: invoke policy evaluator with tool_name and context, return auto_approved boolean and matched_rule in src/mcp/tools/check_auto_approve.rs
- [ ] T056 [US6] Register check_auto_approve tool in MCP ServerHandler call_tool router in src/mcp/server.rs

**Checkpoint**: All P2 stories complete — logging, prompts, and auto-approve operational

---

## Phase 9: User Story 7 — Remote Session Orchestration (Priority: P3)

**Goal**: Operator starts, pauses, resumes, terminates, checkpoints, and restores agent sessions entirely from Slack

**Independent Test**: Use /monocoque session-start to spawn an agent. Pause, resume, and terminate it. Create a checkpoint, make changes, restore, and verify prior state is recovered.

### Implementation for User Story 7

- [ ] T057 [P] [US7] Implement Checkpoint model with file_hashes map (path → SHA-256) for divergence detection, serialized session_state, label, serde derives in src/models/checkpoint.rs
- [ ] T058 [US7] Implement Checkpoint SurrealDB repository: create, get_by_id, list_by_session, delete in src/persistence/checkpoint_repo.rs
- [ ] T059 [US7] Implement session spawner: tokio::process::Command with kill_on_drop(true), configure host CLI binary and args from GlobalConfig, capture stdin/stdout for MCP transport in src/orchestrator/spawner.rs
- [ ] T060 [US7] Implement session manager: start (validate authorized user, enforce concurrent limit, spawn process, create Session record), pause (send pause signal, update status), resume (send resume signal, update status), terminate (kill process, update status) in src/orchestrator/session_manager.rs
- [ ] T061 [US7] Implement slash command router: parse /monocoque commands, dispatch to handler functions, validate session owner on all session-scoped commands in src/slack/commands.rs
- [ ] T062 [US7] Implement session management commands: session-start (spawn + confirm), session-pause, session-resume, session-clear (terminate + cleanup), sessions (list all with state/timestamps) in src/slack/commands.rs
- [ ] T063 [US7] Implement checkpoint commands: session-checkpoint (snapshot session state + compute file hashes), session-restore (compare file hashes, warn on divergence, post Slack message with diverged file list and Confirm Restore/Cancel buttons, restore state only after operator confirms), session-checkpoints (list with labels/timestamps) in src/slack/commands.rs and src/slack/events.rs
- [ ] T064 [US7] Implement wait_for_instruction tool handler: post standby message to Slack, block via oneshot until operator sends resume signal or new instruction via slash command, support configurable timeout in src/mcp/tools/wait_for_instruction.rs
- [ ] T065 [US7] Implement concurrent session limit enforcement: check count_active against max_concurrent_sessions before session-start, return descriptive error when limit exceeded in src/orchestrator/session_manager.rs
- [ ] T066 [US7] Implement session owner validation: on all session-scoped interactions (button clicks, slash commands), verify Slack user_id matches Session.owner_user_id, reject with informative message if mismatch in src/orchestrator/session_manager.rs
- [ ] T067 [US7] Register wait_for_instruction tool in MCP ServerHandler call_tool router and wire slash command handler into Slack event dispatcher in src/mcp/server.rs

**Checkpoint**: Full session orchestration available from Slack — start, manage, checkpoint, restore

---

## Phase 10: User Story 8 — Remote File Browsing and Command Execution (Priority: P3)

**Goal**: Operator browses workspace files and executes pre-approved commands from Slack

**Independent Test**: Issue list-files via Slack and verify directory tree appears. Issue show-file and verify file contents with syntax highlighting. Execute a registered command and verify output posts to Slack.

### Implementation for User Story 8

- [ ] T068 [P] [US8] Implement list-files slash command: recursive directory listing with configurable depth (default 3), format as indented tree, respect workspace_root boundary in src/slack/commands.rs
- [ ] T069 [P] [US8] Implement show-file slash command: read file content, validate path within workspace_root, render with syntax highlighting via code block with language hint, support --lines START:END range in src/slack/commands.rs
- [ ] T070 [US8] Implement custom command execution: look up alias in GlobalConfig.commands registry, reject unknown commands with explicit error, execute via tokio::process::Command, capture stdout/stderr in src/slack/commands.rs
- [ ] T071 [US8] Implement command output Slack formatting: wrap output in code blocks, truncate to Slack message limit (3000 chars) with "truncated" indicator, upload as snippet for large output in src/slack/blocks.rs

**Checkpoint**: Remote workspace visibility and command execution available

---

## Phase 11: User Story 9 — State Recovery After Crash (Priority: P3)

**Goal**: Pending requests and session state survive server restarts; agent recovers last known state on reconnection

**Independent Test**: Submit an approval request, kill the server, restart it, invoke recover_state and verify the pending request is returned with original data.

### Implementation for User Story 9

- [ ] T072 [US9] Implement recover_state tool handler: query SurrealDB for interrupted/active sessions, pending ApprovalRequests (status=Pending), and pending ContinuationPrompts (decision=null) via prompt_repo, return recovered status with session_id, pending_requests list (type: approval|prompt), and last_checkpoint in src/mcp/tools/recover_state.rs
- [ ] T073 [US9] Implement graceful shutdown handler: register SIGTERM/SIGINT/ctrl_c via tokio::signal, trigger CancellationToken on receipt in src/main.rs
- [ ] T074 [US9] Implement state persistence on shutdown: mark all active sessions as Interrupted, mark pending approvals as Interrupted, flush SurrealDB writes in src/main.rs
- [ ] T075 [US9] Implement Slack shutdown notification (post "Server shutting down" with pending request count) and child process cleanup (SIGTERM with 5-second grace period, then force kill) in src/main.rs
- [ ] T076 [US9] Register recover_state tool in MCP ServerHandler call_tool router in src/mcp/server.rs

**Checkpoint**: Server crash recovery is reliable — no data loss on restart

---

## Phase 12: User Story 10 — Operational Mode Switching (Priority: P3)

**Goal**: Switch between remote (Slack), local (IPC), and hybrid modes at runtime

**Independent Test**: Set mode to local and verify Slack notifications stop. Set to remote and verify Slack resumes. Set to hybrid and verify both channels active.

### Implementation for User Story 10

- [ ] T077 [P] [US10] Implement IPC local socket listener: create named pipe (Windows) / Unix domain socket (Linux/macOS) via interprocess crate, accept connections, parse JSON-RPC commands for approve/reject/list in src/ipc/socket.rs
- [ ] T078 [US10] Implement set_operational_mode tool handler: validate mode value, update Session.mode, persist mode to DB, return previous_mode and current_mode in src/mcp/tools/set_operational_mode.rs
- [ ] T079 [US10] Implement mode-aware routing: when mode=remote route to Slack, when mode=local route to IPC, when mode=hybrid route to both (first response wins via tokio::select) in src/mcp/server.rs
- [ ] T080 [US10] Implement monocoque-ctl CLI binary: clap-based CLI with subcommands list (show pending requests), approve <request_id>, reject <request_id> --reason, connecting to server via IPC socket in ctl/main.rs
- [ ] T081 [US10] Register set_operational_mode tool in MCP ServerHandler call_tool router in src/mcp/server.rs

**Checkpoint**: All P3 stories complete — full feature set operational

---

## Phase 13: Polish and Cross-Cutting Concerns

**Purpose**: Documentation, security hardening, and remaining Slack enhancements

> **Note**: Slack rate limit handling (T089) and MCP resource implementation (T090) were promoted to Phase 2 as foundational infrastructure.

- [ ] T083 [P] Implement help slash command: list all available slash commands grouped by category (Session Management, File Operations, System) with descriptions and argument syntax in src/slack/commands.rs
- [ ] T086 [P] Update README.md with project overview, architecture diagram, setup instructions, configuration reference, and usage examples
- [ ] T087 Run quickstart.md validation: build project, create config.toml, connect agent, execute basic approval workflow, verify end-to-end
- [ ] T088 Security hardening audit: verify all file paths use canonicalize + starts_with(workspace_root), verify unauthorized user interactions are logged with user ID and action, verify command allowlist is enforced, verify session owner binding is immutable

---

## Dependencies and Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 completion — **BLOCKS all user stories**
- **US1 (Phase 3)**: Depends on Phase 2 — first MVP deliverable
- **US2 (Phase 4)**: Depends on Phase 3 (needs ApprovalRequest with Approved status)
- **US4 (Phase 5)**: Depends on Phase 2 — can run in parallel with Phase 3/4 if staffed
- **US3 (Phase 6)**: Depends on Phase 2 — can run in parallel with Phase 3/4/5 if staffed
- **US5 (Phase 7)**: Depends on Phase 2 — can run in parallel with Phase 3-6 if staffed
- **US6 (Phase 8)**: Depends on Phase 2 — can run in parallel with Phase 3-7 if staffed
- **US7 (Phase 9)**: Depends on Phase 2 — can run in parallel with other stories if staffed
- **US8 (Phase 10)**: Depends on Phase 9 (needs slash command router from T061)
- **US9 (Phase 11)**: Depends on Phase 2 — can run in parallel with other stories if staffed
- **US10 (Phase 12)**: Depends on Phase 2 — can run in parallel with other stories if staffed
- **Polish (Phase 13)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: No story dependencies — first to implement
- **US2 (P1)**: Depends on US1 (needs approved ApprovalRequest to apply)
- **US4 (P1)**: No story dependencies — independent stall detection
- **US3 (P2)**: No story dependencies — independent logging
- **US5 (P2)**: No story dependencies — independent prompt forwarding (T091 adds persistence repo for crash recovery)
- **US6 (P2)**: No story dependencies — independent policy evaluation
- **US7 (P3)**: No story dependencies — independent session orchestration
- **US8 (P3)**: Depends on US7 (shares slash command infrastructure)
- **US9 (P3)**: No story dependencies — independent recovery
- **US10 (P3)**: No story dependencies — independent IPC/mode routing

### Within Each User Story

- Models before repositories
- Repositories before tool handlers
- Slack rendering before interaction handlers
- Core implementation before tool router registration
- Story complete before moving to next priority

### Parallel Opportunities

- All tasks marked [P] within a phase can run concurrently
- After Phase 2 completes, multiple user stories can be worked in parallel:
  - **Stream A**: US1 → US2 (sequential — US2 depends on US1)
  - **Stream B**: US4 (independent stall detection)
  - **Stream C**: US3, US5, US6 (independent P2 stories)
  - **Stream D**: US7 → US8 (sequential — US8 needs slash commands)
  - **Stream E**: US9, US10 (independent P3 stories)

---

## Parallel Example: User Story 1

```text
# These US1 tasks can run in parallel (different files):
T018: ApprovalRequest model          → src/models/approval.rs
(all other US1 tasks depend on T018/T019)

# After T019 (repo), these can proceed in parallel:
T020: ask_approval handler           → src/mcp/tools/ask_approval.rs
T021: diff rendering                 → src/slack/blocks.rs
T022: button interaction handler     → src/slack/events.rs
```

## Parallel Example: User Story 4

```text
# These US4 tasks can run in parallel (different files):
T032: StallAlert model               → src/models/stall.rs
T034: heartbeat handler              → src/mcp/tools/heartbeat.rs

# After T032, these can proceed:
T033: stall detector                 → src/orchestrator/stall_detector.rs
T035-T039: follow sequentially within stall_detector.rs and slack/
```

---

## Implementation Strategy

### MVP First (User Stories 1 + 2 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (**CRITICAL** — blocks everything)
3. Complete Phase 3: US1 — Remote Code Review
4. Complete Phase 4: US2 — Diff Application
5. **STOP AND VALIDATE**: Agent can submit diff → operator reviews on mobile → file written to disk
6. Deploy/demo if ready — this is the core value proposition

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. US1 + US2 → Remote approval + file writes (**MVP**)
3. US4 → Stall detection safety net (completes P1)
4. US3 → Operator visibility via logging
5. US5 → Eliminate prompt-based stalls
6. US6 → Reduce notification noise (completes P2)
7. US7 + US8 → Full session orchestration
8. US9 → Crash recovery
9. US10 → Mode switching (completes P3)
10. Polish → Resources, docs, hardening

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: US1 → US2 (approval pipeline — sequential)
   - Developer B: US4 (stall detection — independent)
   - Developer C: US3 + US5 + US6 (P2 features — independent)
3. After P1/P2 complete:
   - Developer A: US7 → US8 (session orchestration)
   - Developer B: US9 + US10 (recovery + mode switching)
   - Developer C: Polish

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks in same phase
- [Story] label maps each task to a specific user story for traceability
- Each user story should be independently completable and testable after Phase 2
- Commit after each task or logical group
- Stop at any checkpoint to validate the story independently
- All 9 MCP tools are always visible to agents (FR-032) — return errors for inapplicable calls, do not hide tools
- Session owner binding is immutable after creation (FR-013) — enforce in all interaction handlers
- Workspace policy can only reduce friction, never expand beyond GlobalConfig allowlist (FR-011)
- Total tasks: 92 (T001–T092, including T089–T092 added during analysis remediation)
