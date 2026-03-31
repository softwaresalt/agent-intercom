---
id: TASK-001.04
title: "001 - Foundational (Blocking Prerequisites)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001
dependencies: []
ordinal: 1040
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Tests (Constitution Principle III)

- [X] T101 [P] Write unit tests for `GlobalConfig` TOML deserialization in `tests/unit/config_tests.rs`: valid complete config, missing required fields, invalid field types, default value population, credential env var fallback (FR-036)
- [X] T102 [P] Write unit tests for all domain model `Serialize`/`Deserialize` round-trips in `tests/unit/model_tests.rs`: Session (all status transitions), ApprovalRequest (all status values), Checkpoint, ContinuationPrompt (all prompt types and decisions), StallAlert (all status values), ProgressItem (all statuses); verify enum variant serialization matches data-model.md
- [X] T103 [P] Write unit tests for path validation in `tests/unit/path_validation_tests.rs`: valid resolved path, `..` traversal rejection, symlink escape rejection, workspace root boundary enforcement, relative path resolution (FR-006)
- [X] T104 Write contract tests for SurrealDB schema in `tests/contract/schema_tests.rs`: verify table creation, field constraints, and `ASSERT` rules match data-model.md definitions

### Configuration & Credentials

- [X] T005 Implement `GlobalConfig` struct and TOML deserialization in `src/config.rs` per data-model.md GlobalConfig entity; include `default_workspace_root`, `slack` (channel_id, authorized_user_ids), `timeouts`, `stall`, `commands`, `http_port`, `ipc_name`, `retention_days`, `max_concurrent_sessions`, `host_cli`, `host_cli_args` fields
- [X] T006 Implement credential loading in `src/config.rs`: load `slack_app_token` and `slack_bot_token` from OS keychain (service `monocoque-agent-rc`) using `keyring` crate, falling back to `SLACK_APP_TOKEN`/`SLACK_BOT_TOKEN` environment variables (FR-036); wrap keychain calls in `tokio::task::spawn_blocking`
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

- [X] T016 Implement database connection and initialization in `src/persistence/db.rs`: connect to SurrealDB embedded (RocksDB backend from config path, in-memory for tests via feature flag), select namespace `monocoque` and database `agent_rc`
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

- [X] T029 Implement MCP server handler struct `AgentRcServer` in `src/mcp/handler.rs`: implement `rmcp::ServerHandler` trait with `call_tool`, `list_tools`, `list_resources`, `read_resource`, `on_initialized` methods; store reference to shared application state (config, DB, Slack client, session registry); tracing span on every tool call (FR-037)
- [X] T030 Implement session context resolution in `src/mcp/context.rs`: given an MCP request context, resolve the active `Session` and its `workspace_root`; create `ToolContext` struct carrying session reference, workspace_root, peer handle, and shared state
- [X] T031 [P] Implement stdio transport setup in `src/mcp/transport.rs`: wire `AgentRcServer` to `rmcp::transport::stdio` for the primary agent connection; create and auto-activate a default session on first tool call using `default_workspace_root` from GlobalConfig or `--workspace` CLI arg
- [X] T032 [P] Implement HTTP/SSE transport setup in `src/mcp/sse.rs`: mount `rmcp::StreamableHttpService` onto an `axum::Router` via `nest_service("/mcp", service)`; bind to `config.http_port` on localhost; each SSE connection creates a new session with workspace_root from connection parameters
- [X] T033 Create `src/mcp/mod.rs` re-exporting handler, context, transport, sse

### Path Safety

- [X] T034 Implement path validation utility in `src/diff/path_safety.rs`: `validate_path(file_path, workspace_root) -> Result<PathBuf>` that canonicalizes both paths, verifies the resolved path `starts_with` the workspace root, rejects `..` segments, and returns the canonicalized absolute path (FR-006)

### Server Bootstrap

- [X] T035 Implement server bootstrap in `src/main.rs`: load config (T005-T007), initialize DB and run schema (T016-T017), start retention service (T023), connect Slack client (T025), create `AgentRcServer` with shared state, start stdio transport (T031), start SSE transport with axum (T032), register Slack interaction handler (T027), register SIGTERM/SIGINT signal handler that triggers graceful shutdown (shutdown persistence logic in T081)
- [X] T036 Update `src/lib.rs` to re-export all public modules: config, errors, models, persistence, slack, mcp, diff, policy, orchestrator, ipc
- [X] T037 Verify full compilation with `cargo build` and `cargo clippy` pass cleanly

**Checkpoint**: Foundation ready — all infrastructure compiled, SurrealDB schema deployed, Slack connected, MCP server accepting connections, session context resolution operational. User story implementation can now begin.

---

<!-- SECTION:DESCRIPTION:END -->
