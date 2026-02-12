# Tasks: MCP Remote Agent Server

**Input**: Design documents from `specs/001-mcp-remote-agent-server/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Test tasks included per Constitution Principle III (Test-First Development). Run `cargo test` / `cargo clippy` after each phase.

**Organization**: Tasks grouped by user story (10 stories from spec.md, priority P1→P3).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Exact file paths included in all descriptions

## Path Conventions

Single Rust project at repository root per plan.md:

```text
src/           # Main binary source
ctl/           # monocoque-ctl companion binary
tests/         # contract/, integration/, unit/
```

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization, dependency wiring, and basic compile-check structure

- [X] T001 Add `keyring = "3"` dependency to `Cargo.toml` workspace dependencies and package dependencies for OS keychain credential loading (FR-036)
- [X] T002 [P] Create shared error type enum `AppError` with variants for config, persistence, slack, mcp, diff, policy, ipc, and path violation errors in `src/errors.rs`; implement `std::fmt::Display` and `std::error::Error`
- [X] T003 [P] Initialize tracing subscriber with `env-filter` and `fmt` features in `src/main.rs`; configure JSON output via `--log-format json` CLI flag using `clap` (FR-037)
- [X] T100 [P] Add `#![forbid(unsafe_code)]` attribute to `src/lib.rs` to enforce memory safety at the workspace level per Constitution Principle I (Safety-First Rust)
- [X] T004 Verify project compiles with `cargo build` and passes `cargo clippy`

**Checkpoint**: Project compiles, tracing initialized, error types defined

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Tests (Constitution Principle III)

- [X] T101 [P] Write unit tests for `GlobalConfig` TOML deserialization in `tests/unit/config_tests.rs`: valid complete config, missing required fields, invalid field types, default value population, credential env var fallback (FR-036)
- [X] T102 [P] Write unit tests for all domain model `Serialize`/`Deserialize` round-trips in `tests/unit/model_tests.rs`: Session (all status transitions), ApprovalRequest (all status values), Checkpoint, ContinuationPrompt (all prompt types and decisions), StallAlert (all status values), ProgressItem (all statuses); verify enum variant serialization matches data-model.md
- [X] T103 [P] Write unit tests for path validation in `tests/unit/path_validation_tests.rs`: valid resolved path, `..` traversal rejection, symlink escape rejection, workspace root boundary enforcement, relative path resolution (FR-006)
- [X] T104 Write contract tests for SurrealDB schema in `tests/contract/schema_tests.rs`: verify table creation, field constraints, and `ASSERT` rules match data-model.md definitions

### Configuration & Credentials

- [X] T005 Implement `GlobalConfig` struct and TOML deserialization in `src/config.rs` per data-model.md GlobalConfig entity; include `default_workspace_root`, `slack` (channel_id, authorized_user_ids), `timeouts`, `stall`, `commands`, `http_port`, `ipc_name`, `retention_days`, `max_concurrent_sessions`, `host_cli`, `host_cli_args` fields
- [X] T006 Implement credential loading in `src/config.rs`: load `slack_app_token` and `slack_bot_token` from OS keychain (service `monocoque-agent-rem`) using `keyring` crate, falling back to `SLACK_APP_TOKEN`/`SLACK_BOT_TOKEN` environment variables (FR-036); wrap keychain calls in `tokio::task::spawn_blocking`
- [X] T007 Add CLI argument parsing in `src/main.rs` using `clap`: `--config <path>` (required), `--log-format <text|json>` (default text), `--workspace <path>` (optional override for primary agent workspace root)

### Domain Models

- [X] T008 Implement `Session` struct in `src/models/session.rs` per data-model.md: `session_id`, `owner_user_id`, `workspace_root`, `status` (enum: Created, Active, Paused, Terminated, Interrupted), `prompt`, `mode` (enum: Remote, Local, Hybrid), `created_at`, `updated_at`, `terminated_at`, `last_tool`, `nudge_count`, `stall_paused`, `progress_snapshot` (Option<Vec<ProgressItem>>); derive `Serialize`/`Deserialize`. **Depends on T014** (ProgressItem type)
- [X] T009 [P] Implement `ApprovalRequest` struct in `src/models/approval.rs` per data-model.md: `request_id`, `session_id`, `title`, `description`, `diff_content`, `file_path`, `risk_level` (enum: Low, High, Critical), `status` (enum: Pending, Approved, Rejected, Expired, Consumed, Interrupted), `original_hash`, `slack_ts`, `created_at`, `consumed_at`; derive `Serialize`/`Deserialize`
- [X] T010 [P] Implement `Checkpoint` struct in `src/models/checkpoint.rs` per data-model.md: `checkpoint_id`, `session_id`, `label`, `session_state` (serde_json::Value), `file_hashes` (HashMap<String, String>), `workspace_root`, `progress_snapshot`, `created_at`; derive `Serialize`/`Deserialize`
- [X] T011 [P] Implement `ContinuationPrompt` struct in `src/models/prompt.rs` per data-model.md: `prompt_id`, `session_id`, `prompt_text`, `prompt_type` (enum: Continuation, Clarification, ErrorRecovery, ResourceWarning), `elapsed_seconds`, `actions_taken`, `decision` (enum: Continue, Refine, Stop), `instruction`, `slack_ts`, `created_at`; derive `Serialize`/`Deserialize`
- [X] T012 [P] Implement `StallAlert` struct in `src/models/stall.rs` per data-model.md: `alert_id`, `session_id`, `last_tool`, `last_activity_at`, `idle_seconds`, `nudge_count`, `status` (enum: Pending, Nudged, SelfRecovered, Escalated, Dismissed), `nudge_message`, `progress_snapshot`, `slack_ts`, `created_at`; derive `Serialize`/`Deserialize`
- [X] T013 [P] Implement `WorkspacePolicy` struct in `src/models/policy.rs` per data-model.md: `enabled`, `commands`, `tools`, `file_patterns` (write/read glob lists), `risk_level_threshold`, `log_auto_approved`, `summary_interval_seconds`; derive `Deserialize` from JSON
- [X] T014 [P] Implement `ProgressItem` struct in `src/models/progress.rs`: `label: String`, `status: ProgressStatus` (enum: Done, InProgress, Pending); derive `Serialize`/`Deserialize`; add validation function to reject malformed snapshots
- [X] T015 Create `src/models/mod.rs` re-exporting all model types from session, approval, checkpoint, prompt, stall, policy, progress submodules

### SurrealDB Persistence Layer

- [X] T016 Implement database connection and initialization in `src/persistence/db.rs`: connect to SurrealDB embedded (RocksDB backend from config path, in-memory for tests via feature flag), select namespace `monocoque` and database `agent_rem`
- [X] T017 Implement schema DDL in `src/persistence/schema.rs`: define SCHEMAFULL tables `session`, `approval_request`, `checkpoint`, `continuation_prompt`, `stall_alert` with `DEFINE FIELD` and `ASSERT` constraints per data-model.md; execute on startup with `IF NOT EXISTS` for idempotent migrations
- [X] T018 [P] Implement `SessionRepo` in `src/persistence/session_repo.rs`: CRUD operations for Session — `create`, `get_by_id`, `list_active`, `update_status`, `update_activity` (sets updated_at + last_tool), `update_progress_snapshot`, `set_terminated`, `count_active_sessions`
- [X] T019 [P] Implement `ApprovalRepo` in `src/persistence/approval_repo.rs`: CRUD operations for ApprovalRequest — `create`, `get_by_id`, `get_pending_for_session`, `update_status`, `mark_consumed`, `list_pending`
- [X] T020 [P] Implement `CheckpointRepo` in `src/persistence/checkpoint_repo.rs`: CRUD for Checkpoint — `create`, `get_by_id`, `list_for_session`, `delete_for_session`
- [X] T021 [P] Implement `PromptRepo` in `src/persistence/prompt_repo.rs`: CRUD for ContinuationPrompt — `create`, `get_by_id`, `get_pending_for_session`, `update_decision`
- [X] T022 [P] Implement `StallAlertRepo` in `src/persistence/stall_repo.rs`: CRUD for StallAlert — `create`, `get_active_for_session`, `update_status`, `increment_nudge_count`, `dismiss`
- [X] T023 Implement `RetentionService` in `src/persistence/retention.rs`: background task running hourly via `tokio::time::interval`; delete children first (approval_request, checkpoint, continuation_prompt, stall_alert), then session where `status = terminated AND terminated_at < now - retention_days` (FR-035)
- [X] T024 Create `src/persistence/mod.rs` re-exporting db, schema, all repos, and retention service

### Slack Client Foundation

- [X] T025 Implement Slack Socket Mode client wrapper in `src/slack/client.rs`: connect using `slack-morphism` `SlackSocketModeClientsManager` with app_token; handle reconnection; expose methods `post_message`, `update_message`, `upload_file`, `open_modal`; implement rate-limit queue with exponential backoff (FR-020)
- [X] T026 [P] Implement Slack Block Kit message builders in `src/slack/blocks.rs`: helper functions for building `rich_text_preformatted` blocks (small diffs), file upload payloads (large diffs), `actions` blocks with buttons (Accept/Reject, Continue/Refine/Stop, Nudge/Stop), severity-formatted log messages (info ℹ️, success ✅, warning ⚠️, error ❌)
- [X] T027 [P] Implement interaction handler dispatch in `src/slack/interactions.rs`: receive button press and modal submission payloads from Socket Mode events; dispatch to appropriate handler by `action_id`; verify `user.id` matches session owner (FR-013); replace buttons with static status text after first action (FR-022)
- [X] T028 Create `src/slack/mod.rs` re-exporting client, blocks, interactions, handlers
- [X] T128 Create `src/slack/handlers/mod.rs` re-exporting approval, nudge, prompt handler submodules

### MCP Server Foundation

- [X] T029 Implement MCP server handler struct `AgentRemServer` in `src/mcp/handler.rs`: implement `rmcp::ServerHandler` trait with `call_tool`, `list_tools`, `list_resources`, `read_resource`, `on_initialized` methods; store reference to shared application state (config, DB, Slack client, session registry); tracing span on every tool call (FR-037)
- [X] T030 Implement session context resolution in `src/mcp/context.rs`: given an MCP request context, resolve the active `Session` and its `workspace_root`; create `ToolContext` struct carrying session reference, workspace_root, peer handle, and shared state
- [X] T031 [P] Implement stdio transport setup in `src/mcp/transport.rs`: wire `AgentRemServer` to `rmcp::transport::stdio` for the primary agent connection; create and auto-activate a default session on first tool call using `default_workspace_root` from GlobalConfig or `--workspace` CLI arg
- [X] T032 [P] Implement HTTP/SSE transport setup in `src/mcp/sse.rs`: mount `rmcp::StreamableHttpService` onto an `axum::Router` via `nest_service("/mcp", service)`; bind to `config.http_port` on localhost; each SSE connection creates a new session with workspace_root from connection parameters
- [X] T033 Create `src/mcp/mod.rs` re-exporting handler, context, transport, sse

### Path Safety

- [X] T034 Implement path validation utility in `src/diff/path_safety.rs`: `validate_path(file_path, workspace_root) -> Result<PathBuf>` that canonicalizes both paths, verifies the resolved path `starts_with` the workspace root, rejects `..` segments, and returns the canonicalized absolute path (FR-006)

### Server Bootstrap

- [X] T035 Implement server bootstrap in `src/main.rs`: load config (T005-T007), initialize DB and run schema (T016-T017), start retention service (T023), connect Slack client (T025), create `AgentRemServer` with shared state, start stdio transport (T031), start SSE transport with axum (T032), register Slack interaction handler (T027), register SIGTERM/SIGINT signal handler that triggers graceful shutdown (shutdown persistence logic in T081)
- [X] T036 Update `src/lib.rs` to re-export all public modules: config, errors, models, persistence, slack, mcp, diff, policy, orchestrator, ipc
- [X] T037 Verify full compilation with `cargo build` and `cargo clippy` pass cleanly

**Checkpoint**: Foundation ready — all infrastructure compiled, SurrealDB schema deployed, Slack connected, MCP server accepting connections, session context resolution operational. User story implementation can now begin.

---

## Phase 3: User Story 1 — Remote Code Review and Approval (Priority: P1) 🎯 MVP

**Goal**: Agent submits code proposals for remote approval via Slack; operator reviews diffs and taps Accept/Reject from mobile

**Independent Test**: Start server, connect agent, invoke `ask_approval` with a sample diff, verify diff appears in Slack with actionable buttons, tap Accept, verify agent receives approved response

### Tests (Constitution Principle III)

- [X] T105 Write contract tests for `ask_approval` tool in `tests/contract/ask_approval_tests.rs`: validate input schema (required fields, enum values, optional fields) and output schema (`status` enum, `request_id` presence, optional `reason`) per mcp-tools.json contract
- [X] T106 Write integration test for approval flow in `tests/integration/approval_flow_tests.rs`: submit approval request → verify DB record created → simulate Accept → verify oneshot resolves with `approved` status → verify DB updated; repeat for Reject and timeout paths

### Implementation for User Story 1

- [X] T038 [US1] Implement `ask_approval` MCP tool handler in `src/mcp/tools/ask_approval.rs`: accept `title`, `description`, `diff`, `file_path`, `risk_level` per mcp-tools.json contract; validate `file_path` via `validate_path` against session's `workspace_root`; compute SHA-256 hash of current file content; create `ApprovalRequest` record (status=Pending) in SurrealDB; render diff in Slack (inline for <20 lines, snippet upload for ≥20 lines) with Accept/Reject buttons carrying `request_id` in action value; block on `tokio::sync::oneshot` channel until operator responds or timeout elapses; return `{status, request_id, reason}` per contract
- [X] T039 [US1] Implement approval interaction callback in `src/slack/handlers/approval.rs`: handle Accept and Reject button presses from `src/slack/interactions.rs` dispatch; verify session owner (FR-013); update `ApprovalRequest` status in DB; resolve the `oneshot::Sender` to unblock the waiting tool call; replace buttons with status text (FR-022); for Reject, capture optional reason from operator
- [X] T040 [US1] Implement approval timeout logic in `src/mcp/tools/ask_approval.rs`: if `timeouts.approval_seconds` elapses with no response, resolve oneshot with `timeout` status, update DB record to Expired, post timeout notification to Slack channel
- [X] T041 [US1] Wire pending approval request map in `src/mcp/handler.rs`: maintain `HashMap<String, oneshot::Sender<ApprovalResponse>>` keyed by `request_id` in shared state; `ask_approval` inserts sender, interaction callback extracts and resolves it
- [X] T042 [US1] Add tracing spans to `ask_approval` tool: span covering full tool execution with `request_id`, `file_path`, `risk_level` attributes; child span for Slack API call; log final outcome (approved/rejected/timeout) at info level

**Checkpoint**: User Story 1 functional — agent can submit diffs, operator can review and approve/reject from Slack

---

## Phase 4: User Story 2 — Programmatic Diff Application (Priority: P1)

**Goal**: After approval, server applies code changes to the local file system programmatically

**Independent Test**: Submit diff via `ask_approval`, approve it, invoke `accept_diff` with the `request_id`, verify file written to disk with correct content

### Tests (Constitution Principle III)

- [X] T107 Write unit tests for diff application in `tests/unit/diff_tests.rs`: full-file write (new file, overwrite), unified diff patch (clean apply, failed apply), atomic write via tempfile, parent directory creation
- [X] T108 Write contract tests for `accept_diff` tool in `tests/contract/accept_diff_tests.rs`: validate input/output schemas per mcp-tools.json; test `not_approved`, `already_consumed`, `path_violation`, `patch_conflict` error codes
- [X] T109 Write integration test for approve→apply pipeline in `tests/integration/diff_apply_tests.rs`: submit diff → approve → apply → verify file on disk; test hash mismatch conflict detection with file mutation between proposal and application

### Implementation for User Story 2

- [X] T043 [US2] Implement file writing utility in `src/diff/writer.rs`: `write_full_file(path, content, workspace_root) -> Result<WriteSummary>` that validates path, creates parent directories if needed, writes to `tempfile::NamedTempFile` in the same directory, then `persist()` for atomic rename; return `{path, bytes_written}`
- [X] T044 [US2] Implement diff/patch application utility in `src/diff/patcher.rs`: `apply_patch(path, unified_diff, workspace_root) -> Result<WriteSummary>` using `diffy::Patch::from_str` and `diffy::apply`; read existing file, apply patch, write result via `write_full_file`; handle patch failure with descriptive error
- [X] T045 [US2] Implement `accept_diff` MCP tool handler in `src/mcp/tools/accept_diff.rs`: accept `request_id` and `force` per mcp-tools.json contract; look up `ApprovalRequest` by ID; validate status is `Approved` (return `not_approved` error otherwise); verify `consumed_at` is None (return `already_consumed` if set); recompute SHA-256 of current file and compare to `original_hash` — if mismatch and `force=false` return `patch_conflict`, if `force=true` log warning to Slack; determine full-file vs patch mode from `diff_content` format; apply via writer or patcher; mark request as `Consumed` with `consumed_at` timestamp; post confirmation to Slack; return `{status: applied, files_written}` per contract
- [X] T046 [US2] Add tracing spans to `accept_diff` tool: span with `request_id`, `file_path`, `force` attributes; log hash comparison result; log write outcome

**Checkpoint**: End-to-end remote workflow complete — agent proposes, operator approves, server writes files

---

## Phase 5: User Story 4 — Agent Stall Detection and Remote Nudge (Priority: P1)

**Goal**: Server detects when agent goes silent, alerts operator via Slack, and nudges agent to resume

**Independent Test**: Connect agent, make several tool calls, simulate silence (no calls for threshold period), verify stall alert in Slack, tap Nudge, verify agent receives notification

### Tests (Constitution Principle III)

- [X] T110 Write unit tests for stall detection in `tests/unit/stall_detector_tests.rs`: timer fires after threshold, `reset()` prevents firing, `pause()`/`resume()` toggle, consecutive nudge counting, self-recovery detection clears alert
- [X] T111 Write contract tests for `heartbeat` tool in `tests/contract/heartbeat_tests.rs`: validate input/output schemas per mcp-tools.json; test with status_message only, with valid progress_snapshot, with malformed snapshot (must reject), with omitted snapshot (must preserve existing)
- [X] T112 Write integration test for nudge flow in `tests/integration/nudge_flow_tests.rs`: agent makes tool calls → goes silent → verify stall alert created → simulate nudge → verify `monocoque/nudge` notification delivered with progress snapshot summary

### Implementation for User Story 4

- [X] T047 [US4] Implement per-session stall detection timer in `src/orchestrator/stall_detector.rs`: for each active session, maintain a `tokio::time::Interval` that fires after `stall.inactivity_threshold_seconds` of no MCP activity; expose `reset()` method called on every tool call request, tool call response, and heartbeat; expose `pause()` and `resume()` for long-running server operations; use `CancellationToken` for cleanup on session termination
- [X] T048 [US4] Implement stall alert posting in `src/orchestrator/stall_detector.rs`: when timer fires, create `StallAlert` record in DB, post alert to Slack with last tool name, idle seconds, and session prompt context; if session has a `progress_snapshot`, render checklist with ✅/🔄/⬜ emoji per item (FR-026); include Nudge, Nudge with Instructions, and Stop buttons
- [X] T049 [US4] Implement `heartbeat` MCP tool handler in `src/mcp/tools/heartbeat.rs`: accept `status_message` and optional `progress_snapshot` per mcp-tools.json contract; validate snapshot structure if provided (reject malformed, preserve existing); update session's `progress_snapshot` in DB if provided; reset stall timer; optionally log `status_message` to Slack via `remote_log`; return `{acknowledged, session_id, stall_detection_enabled}` per contract
- [X] T050 [US4] Implement nudge interaction callback in `src/slack/handlers/nudge.rs`: handle Nudge, Nudge with Instructions, and Stop button presses; for Nudge: send `monocoque/nudge` CustomNotification via `context.peer.send_notification()` with default message, progress_snapshot summary, and nudge_count (FR-027); for Nudge with Instructions: open Slack modal for custom message, then send notification with that text; for Stop: terminate session; update StallAlert status in DB; replace buttons with status text
- [X] T051 [US4] Implement auto-nudge escalation in `src/orchestrator/stall_detector.rs`: after `stall.escalation_threshold_seconds` with no operator response, auto-nudge the agent with default continuation message including progress snapshot summary (FR-028, FR-034); increment nudge counter; if counter exceeds `stall.max_retries`, post escalated alert with `@channel` mention (FR-029)
- [X] T052 [US4] Implement self-recovery detection in `src/orchestrator/stall_detector.rs`: when agent resumes activity (any tool call resets timer) while a stall alert is pending/nudged, update alert status to SelfRecovered, update Slack message to show auto-recovery, disable action buttons (FR-030)
- [X] T053 [US4] Wire stall timer reset into MCP handler in `src/mcp/handler.rs`: on every `call_tool` invocation and every tool response, call `stall_detector.reset(session_id)`; auto-pause timer when executing long-running server operations (command execution) and resume on completion (FR-025)
- [X] T054 [US4] Add tracing spans to stall detection: spans for timer fire, alert posting, nudge sending, auto-nudge escalation, self-recovery events

**Checkpoint**: Stall detection operational — silent agents detected, operator alerted, nudge restores agent, auto-escalation works

---

## Phase 6: User Story 3 — Remote Status Logging (Priority: P2)

**Goal**: Agent sends non-blocking progress messages to Slack with severity-based formatting

**Independent Test**: Invoke `remote_log` with messages at info/success/warning/error levels, verify each appears in Slack with correct formatting

### Tests (Constitution Principle III)

- [ ] T113 Write contract tests for `remote_log` tool in `tests/contract/remote_log_tests.rs`: validate input/output schemas per mcp-tools.json; verify all severity levels (info, success, warning, error) produce correct Block Kit formatting

### Implementation for User Story 3

- [ ] T055 [US3] Implement `remote_log` MCP tool handler in `src/mcp/tools/remote_log.rs`: accept `message`, `level`, `thread_ts` per mcp-tools.json contract; format message using Block Kit severity builders from `src/slack/blocks.rs` (info ℹ️, success ✅, warning ⚠️, error ❌); post to Slack channel (or thread if `thread_ts` provided); do NOT block agent — queue message via Slack client's rate-limit queue; return `{posted, ts}` per contract
- [ ] T056 [US3] Add tracing span to `remote_log` tool: span with `level`, `thread_ts` attributes; log post result

**Checkpoint**: Agent can send visible progress updates to Slack

---

## Phase 7: User Story 5 — Continuation Prompt Forwarding (Priority: P2)

**Goal**: Agent-generated continuation prompts forwarded to Slack with Continue/Refine/Stop buttons

**Independent Test**: Invoke `forward_prompt` with a continuation prompt, verify it appears in Slack with three buttons, tap Continue, verify agent receives decision

### Tests (Constitution Principle III)

- [ ] T114 Write contract tests for `forward_prompt` tool in `tests/contract/forward_prompt_tests.rs`: validate input/output schemas per mcp-tools.json; test all `prompt_type` values and `decision` enum values
- [ ] T115 Write integration test for prompt→decision flow in `tests/integration/prompt_flow_tests.rs`: forward prompt → verify DB record → simulate Continue → verify oneshot resolves; repeat for Refine (with instruction) and Stop; test auto-timeout returns `continue`

### Implementation for User Story 5

- [ ] T057 [US5] Implement `forward_prompt` MCP tool handler in `src/mcp/tools/forward_prompt.rs`: accept `prompt_text`, `prompt_type`, `elapsed_seconds`, `actions_taken` per mcp-tools.json contract; create `ContinuationPrompt` record in DB; post prompt to Slack with Continue/Refine/Stop buttons and elapsed time context; block on `oneshot` channel until response or `timeouts.prompt_seconds` elapses; on timeout, auto-respond with `continue` decision and post timeout notification (FR-008); return `{decision, instruction}` per contract
- [ ] T058 [US5] Implement prompt interaction callback in `src/slack/handlers/prompt.rs`: handle Continue, Refine, and Stop button presses; for Continue: resolve oneshot with `continue`; for Refine: open modal dialog for revised instruction text, on submission resolve with `refine` + instruction; for Stop: resolve with `stop`; update DB record with decision; replace buttons with status text (FR-022)
- [ ] T059 [US5] Wire pending prompt map in `src/mcp/handler.rs`: maintain `HashMap<String, oneshot::Sender<PromptResponse>>` keyed by `prompt_id` in shared state, similar to approval pattern
- [ ] T060 [US5] Add tracing spans to `forward_prompt`: span with `prompt_type`, `prompt_id` attributes; log decision outcome

**Checkpoint**: Continuation prompts forwarded and resolved from Slack

---

## Phase 8: User Story 6 — Workspace Auto-Approve Policy (Priority: P2)

**Goal**: Workspace policy file auto-approves pre-trusted operations, reducing Slack notification noise

**Independent Test**: Create `.monocoque/settings.json` with "cargo test" auto-approved, invoke `check_auto_approve`, verify returns `auto_approved: true`

### Tests (Constitution Principle III)

- [ ] T116 Write unit tests for policy loader in `tests/unit/policy_tests.rs`: valid policy file parsing, malformed file fallback to deny-all, commands not in global allowlist rejected (FR-011), missing policy file returns deny-all
- [ ] T117 Write unit tests for policy evaluator in `tests/unit/policy_evaluator_tests.rs`: command matching, tool matching, file pattern glob matching, risk_level_threshold enforcement, global config supersedes workspace config
- [ ] T118 Write contract tests for `check_auto_approve` tool in `tests/contract/check_auto_approve_tests.rs`: validate input/output schemas per mcp-tools.json

### Implementation for User Story 6

- [ ] T061 [US6] Implement policy file loader in `src/policy/loader.rs`: parse `.monocoque/settings.json` from a given `workspace_root` into `WorkspacePolicy` struct; on parse error, fall back to "require approval for everything" and emit tracing warning (edge case from spec); validate `commands` entries exist in global `config.commands` allowlist (FR-011)
- [ ] T062 [US6] Implement policy evaluator in `src/policy/evaluator.rs`: `check_auto_approve(tool_name, context, workspace_policy, global_config) -> AutoApproveResult` matching against commands, tools, file_patterns, and risk_level_threshold; return matched rule name or `auto_approved: false`
- [ ] T063 [US6] Implement policy hot-reload via `notify` file watcher in `src/policy/watcher.rs`: watch `.monocoque/settings.json` for each active workspace_root using `notify::RecommendedWatcher`; on change event, reload policy via loader; register/unregister watchers as sessions start/terminate (FR-010)
- [ ] T064 [US6] Implement `check_auto_approve` MCP tool handler in `src/mcp/tools/check_auto_approve.rs`: accept `tool_name` and `context` per mcp-tools.json contract; load policy for session's workspace_root; evaluate via `PolicyEvaluator`; return `{auto_approved, matched_rule}` per contract
- [ ] T065 [US6] Create `src/policy/mod.rs` re-exporting loader, evaluator, watcher
- [ ] T066 [US6] Add tracing spans to policy evaluation: span with tool_name, matched_rule, auto_approved attributes

**Checkpoint**: Auto-approve policy functional — trusted operations bypass Slack round-trip

---

## Phase 9: User Story 7 — Remote Session Orchestration (Priority: P3)

**Goal**: Operator starts, pauses, resumes, terminates, checkpoints, and restores sessions from Slack

**Independent Test**: Issue `session-start` via Slack, verify agent spawns; pause and resume; create checkpoint; restore checkpoint

### Tests (Constitution Principle III)

- [ ] T119 Write integration test for session lifecycle in `tests/integration/session_lifecycle_tests.rs`: start → active → pause → resume → checkpoint (verify file hashes stored) → terminate; verify `max_concurrent_sessions` enforcement (FR-023); verify owner-only access (FR-013)
- [ ] T120 Write unit tests for checkpoint hash comparison in `tests/unit/checkpoint_tests.rs`: create checkpoint with file hashes → mutate files → restore → verify divergence warning includes correct file list

### Implementation for User Story 7

- [ ] T067 [US7] Implement slash command dispatcher in `src/slack/commands.rs`: parse `/monocoque <command> [args]` from Slack slash command events; dispatch to handler by command name per mcp-resources.json slashCommands contract; verify user is in `authorized_user_ids` (FR-013); for session-scoped commands, verify user is session owner
- [ ] T068 [US7] Implement agent process spawner in `src/orchestrator/spawner.rs`: `spawn_session(prompt, workspace_root, config) -> Result<Session>` using `tokio::process::Command` with `kill_on_drop(true)`; set `MONOCOQUE_WORKSPACE_ROOT` env var; pass `--config` and SSE endpoint URL to host CLI; create Session record in DB with status=Created; enforce `max_concurrent_sessions` limit (FR-023)
- [ ] T069 [US7] Implement session lifecycle commands in `src/orchestrator/session_manager.rs`: `pause_session` (set status=Paused, stop processing tool calls), `resume_session` (set status=Active), `terminate_session` (set status=Terminated, set terminated_at, send SIGTERM to child process with 5s grace, force kill if needed, post notification to Slack) (FR-012, FR-021)
- [ ] T070 [US7] Implement checkpoint creation in `src/orchestrator/checkpoint_manager.rs`: `create_checkpoint(session_id, label)` — snapshot session state, compute SHA-256 hashes of workspace files, capture progress_snapshot, store Checkpoint record in DB, post confirmation to Slack (FR-024)
- [ ] T071 [US7] Implement checkpoint restore in `src/orchestrator/checkpoint_manager.rs`: `restore_checkpoint(checkpoint_id)` — load checkpoint, compare `file_hashes` against current files, warn operator of divergences via Slack with file list, wait for confirmation, restore session state, include progress_snapshot in restore response (FR-024)
- [ ] T072 [US7] Implement `sessions` and `session-checkpoints` list commands in `src/slack/commands.rs`: query DB for active sessions or checkpoints, format and post to Slack
- [ ] T073 [US7] Implement `help` command in `src/slack/commands.rs`: list all available slash commands grouped by category per mcp-resources.json (FR-019)
- [ ] T074 [US7] Create `src/orchestrator/mod.rs` re-exporting spawner, session_manager, checkpoint_manager, stall_detector
- [ ] T075 [US7] Add tracing spans to session lifecycle: spans for spawn, pause, resume, terminate, checkpoint-create, checkpoint-restore

**Checkpoint**: Full session orchestration operational from Slack

---

## Phase 10: User Story 8 — Remote File Browsing and Command Execution (Priority: P3)

**Goal**: Operator browses workspace files and runs pre-approved commands from Slack

**Independent Test**: Issue `list-files` via Slack, verify directory tree; issue `show-file`, verify file contents; run a registered command

### Tests (Constitution Principle III)

- [ ] T121 Write unit tests for command execution safety in `tests/unit/command_exec_tests.rs`: allowed command passes, disallowed command rejected (FR-014), path validation for list-files/show-file stays within workspace root

### Implementation for User Story 8

- [ ] T076 [US8] Implement `list-files` command handler in `src/slack/commands.rs`: accept optional path and `--depth N` flag; list directory contents from session's workspace_root; validate path stays within workspace root (FR-006); format as tree and post to Slack
- [ ] T077 [US8] Implement `show-file` command handler in `src/slack/commands.rs`: accept path and optional `--lines START:END` range; validate path within workspace root; read file contents; upload to Slack as snippet with syntax highlighting based on file extension
- [ ] T078 [US8] Implement custom command execution handler in `src/slack/commands.rs`: accept command alias from Slack; look up in `config.commands` registry (FR-014); if not found return "command not found" error; if found, execute via `tokio::process::Command` with working directory set to session's workspace_root; capture stdout/stderr; post output to Slack; auto-pause stall timer during execution (FR-025)
- [ ] T079 [US8] Add tracing spans to file browsing and command execution

**Checkpoint**: Remote file browsing and command execution functional

---

## Phase 11: User Story 9 — State Recovery After Crash (Priority: P3)

**Goal**: After server crash/restart, agent recovers last known state including pending requests

**Independent Test**: Submit approval, kill server, restart, invoke `recover_state`, verify pending request returned

### Tests (Constitution Principle III)

- [ ] T122 Write contract tests for `recover_state` tool in `tests/contract/recover_state_tests.rs`: validate input/output schemas per mcp-tools.json; test `recovered` and `clean` status paths; verify progress_snapshot included in response
- [ ] T123 Write integration test for crash recovery in `tests/integration/crash_recovery_tests.rs`: create session with pending approval → simulate shutdown (mark Interrupted) → restart → invoke `recover_state` → verify pending request returned with original data and progress snapshot (SC-004)

### Implementation for User Story 9

- [ ] T080 [US9] Implement `recover_state` MCP tool handler in `src/mcp/tools/recover_state.rs`: accept optional `session_id` per mcp-tools.json contract; if provided, load specific session; otherwise find most recently active session; collect pending approval requests and prompts; include last checkpoint info; include `progress_snapshot` from session record; return `{status: recovered|clean, session_id, pending_requests, last_checkpoint, progress_snapshot}` per contract
- [ ] T081 [US9] Implement shutdown state persistence logic called by the graceful shutdown handler (T035) in `src/main.rs`: mark all pending approval requests and prompts as Interrupted in DB; mark all active/paused sessions as Interrupted with `terminated_at` set; post final notification to Slack; terminate spawned agent processes with 5s grace period; flush and close DB connection (FR-021)
- [ ] T082 [US9] Add Slack reconnection on startup: on server start, check for sessions with status=Interrupted, re-post any pending approval requests that were in-flight to Slack (edge case: Slack WebSocket drop mid-approval)
- [ ] T083 [US9] Add tracing spans to recovery: span covering recovery query with session_id, pending_count attributes

**Checkpoint**: Crash recovery operational — no data loss on server restart

---

## Phase 12: User Story 10 — Operational Mode Switching (Priority: P3)

**Goal**: Developer switches between remote/local/hybrid modes at runtime

**Independent Test**: Set mode to local, verify Slack suppressed; set to remote, verify Slack active; set to hybrid, verify both channels active

### Tests (Constitution Principle III)

- [ ] T124 Write contract tests for `set_operational_mode` and `wait_for_instruction` tools in `tests/contract/mode_tests.rs`: validate input/output schemas per mcp-tools.json; verify all mode enum values
- [ ] T125 Write unit tests for mode-aware routing in `tests/unit/mode_routing_tests.rs`: remote mode posts to Slack only, local mode routes to IPC only, hybrid mode posts to both

### Implementation for User Story 10

- [ ] T084 [US10] Implement `set_operational_mode` MCP tool handler in `src/mcp/tools/set_operational_mode.rs`: accept `mode` per mcp-tools.json contract; update session's mode in DB; persist across restarts; return `{previous_mode, current_mode}` per contract
- [ ] T085 [US10] Implement mode-aware message routing in `src/slack/client.rs`: before posting any Slack message, check session's current mode; if `local` mode, suppress Slack post and route to IPC channel; if `hybrid`, post to both; if `remote` (default), Slack only
- [ ] T086 [US10] Implement `wait_for_instruction` MCP tool handler in `src/mcp/tools/wait_for_instruction.rs`: accept `message` and `timeout_seconds` per mcp-tools.json contract; post waiting status to Slack; block on resume signal from Slack or IPC; return `{status: resumed|timeout, instruction}` per contract
- [ ] T087 [US10] Implement IPC server in `src/ipc/server.rs`: listen on named pipe (Windows) or Unix domain socket (Linux/macOS) using `interprocess::local_socket`; accept local approve/reject/resume commands from `monocoque-ctl`; route to appropriate handler (FR-016)
- [ ] T088 [US10] Implement `monocoque-ctl` CLI in `ctl/main.rs`: connect to IPC socket; implement `list`, `approve <id>`, `reject <id> [--reason]`, `resume [instruction]`, `mode <remote|local|hybrid>`, `credential set <key>` subcommands using `clap`
- [ ] T089 [US10] Create `src/ipc/mod.rs` re-exporting server
- [ ] T090 [US10] Add tracing spans to mode switching and IPC operations

**Checkpoint**: Full mode switching and local override operational

---

## Phase 13: User Story MCP Resource — Slack Channel History

**Goal**: Expose Slack channel history as an MCP resource for agent context

### Tests (Constitution Principle III)

- [ ] T126 Write contract tests for `slack://channel/{id}/recent` resource in `tests/contract/resource_tests.rs`: validate output schema per mcp-resources.json; test channel ID validation against config

### Implementation

- [ ] T091 Implement `slack://channel/{id}/recent` MCP resource handler in `src/mcp/resources/slack_channel.rs`: read recent messages from configured Slack channel using `conversations.history` API; return `{messages, has_more}` per mcp-resources.json contract; validate `id` matches `config.slack.channel_id` (FR-018)
- [ ] T092 Wire resource handler into `AgentRemServer::read_resource` in `src/mcp/handler.rs`

**Checkpoint**: Agent can read operator instructions from Slack channel

---

## Phase 14: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] T093 [P] Add authorization guard for all Slack interactions in `src/slack/interactions.rs`: verify user is in `authorized_user_ids` for all button presses and slash commands; silently ignore unauthorized users and log security event (FR-013, SC-009)
- [ ] T094 [P] Add double-submission prevention across all interactive messages in `src/slack/interactions.rs`: after first button action on any message (approval, prompt, stall alert), immediately call `chat.update` to replace buttons with static status text (FR-022)
- [ ] T095 [P] Add Slack reconnection handling in `src/slack/client.rs`: use `SlackSocketModeClientsManager` reconnection hooks; on reconnect, re-post any pending interactive messages that may have been lost (SC-003)
- [ ] T096 Verify end-to-end workflow: run server, connect agent, submit approval → approve → apply diff → send log → trigger stall → nudge → recover; validate all acceptance scenarios
- [ ] T097 Run `cargo clippy -- -D warnings` and `cargo test` to verify no regressions
- [ ] T098 Validate quickstart.md: follow setup steps from `specs/001-mcp-remote-agent-server/quickstart.md` end-to-end and verify accuracy

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user stories
- **Phases 3-4 (US1, US2)**: Depend on Phase 2 — US2 depends on US1 (needs approval before diff application)
- **Phase 5 (US4)**: Depends on Phase 2 — stall detection is independent of US1/US2
- **Phase 6 (US3)**: Depends on Phase 2 — logging is independent
- **Phase 7 (US5)**: Depends on Phase 2 — prompt forwarding is independent
- **Phase 8 (US6)**: Depends on Phase 2 — auto-approve is independent
- **Phase 9 (US7)**: Depends on Phase 2 — session orchestration is independent but benefits from US4 (stall detection)
- **Phase 10 (US8)**: Depends on Phase 2 — file browsing is independent
- **Phase 11 (US9)**: Depends on Phase 2 — recovery benefits from US1/US4 entities being in place
- **Phase 12 (US10)**: Depends on Phase 2 — mode switching is independent
- **Phase 13 (Resource)**: Depends on Phase 2
- **Phase 14 (Polish)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: After Phase 2 — no dependencies on other stories
- **US2 (P1)**: After US1 — requires ApprovalRequest to exist (approval→apply flow)
- **US4 (P1)**: After Phase 2 — independent, can run parallel with US1
- **US3 (P2)**: After Phase 2 — independent, can run parallel
- **US5 (P2)**: After Phase 2 — independent, can run parallel
- **US6 (P2)**: After Phase 2 — independent, can run parallel
- **US7 (P3)**: After Phase 2 — independent
- **US8 (P3)**: After Phase 2 — independent, benefits from US7 session model
- **US9 (P3)**: After Phase 2 — benefits from US1, US4 entities being defined
- **US10 (P3)**: After Phase 2 — independent

### Parallel Opportunities

After Phase 2 completion, the following can run in parallel:

```text
Stream A (P1 critical path): US1 → US2
Stream B (P1 parallel):      US4 (stall detection)
Stream C (P2 batch):         US3, US5, US6 (all independent)
Stream D (P3 batch):         US7, US8, US9, US10 (all independent)
```

### Within Each User Story

- Models → Persistence → Service logic → MCP tool handler → Slack handler → Tracing
- Core implementation before integration with other stories
- Story complete before polish phase

---

## Parallel Example: Phase 2 Foundational

```text
# Launch all model definitions in parallel (different files):
T008: Session model in src/models/session.rs
T009: ApprovalRequest model in src/models/approval.rs
T010: Checkpoint model in src/models/checkpoint.rs
T011: ContinuationPrompt model in src/models/prompt.rs
T012: StallAlert model in src/models/stall.rs
T013: WorkspacePolicy model in src/models/policy.rs
T014: ProgressItem model in src/models/progress.rs

# Then launch all repo implementations in parallel (different files):
T018: SessionRepo in src/persistence/session_repo.rs
T019: ApprovalRepo in src/persistence/approval_repo.rs
T020: CheckpointRepo in src/persistence/checkpoint_repo.rs
T021: PromptRepo in src/persistence/prompt_repo.rs
T022: StallAlertRepo in src/persistence/stall_repo.rs

# Launch Slack and MCP foundations in parallel:
T025: Slack client in src/slack/client.rs
T026: Block Kit builders in src/slack/blocks.rs
T029: MCP handler in src/mcp/handler.rs
T031: Stdio transport in src/mcp/transport.rs
T032: SSE transport in src/mcp/sse.rs
```

---

## Implementation Strategy

### MVP First (User Stories 1 + 2 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: User Story 1 (remote approval)
4. Complete Phase 4: User Story 2 (diff application)
5. **STOP and VALIDATE**: Test end-to-end approve→apply flow independently
6. Deploy/demo if ready — this is the core value proposition

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. US1 + US2 → MVP: remote approve + apply ✅
3. US4 → Stall detection + nudge ✅ (completes P1)
4. US3 + US5 + US6 → Logging, prompts, auto-approve ✅ (P2)
5. US7 + US8 + US9 + US10 → Session mgmt, file browsing, recovery, modes ✅ (P3)
6. Polish → Cross-cutting hardening

### Parallel Team Strategy

With multiple developers after Phase 2:
- **Developer A**: US1 → US2 (critical path)
- **Developer B**: US4 (stall detection, P1)
- **Developer C**: US3 + US5 + US6 (P2 batch)
- Developer D: US7 + US8 + US9 + US10 (P3 batch, after C finishes)

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Run `cargo clippy` after each phase to catch issues early
- All model structs use `#[derive(Serialize, Deserialize, Debug, Clone)]`
- All MCP tool handlers emit tracing spans per FR-037
- All Slack interactions verify session owner per FR-013
