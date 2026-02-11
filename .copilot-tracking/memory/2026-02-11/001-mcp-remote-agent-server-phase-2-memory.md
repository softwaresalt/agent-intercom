# Session Memory: Phase 2 — Foundational Infrastructure

**Feature**: 001-mcp-remote-agent-server
**Phase**: 2 (Foundational)
**Date**: 2026-02-11
**Status**: Complete

## Task Overview

Phase 2 implements all foundational infrastructure for the monocoque-agent-rem MCP server. 38 tasks covering domain models, configuration, persistence, Slack client, MCP server handler, path safety, tests, and server bootstrap.

## Current State

### Completed Tasks (38/38)

| ID | Description | Status |
|----|-------------|--------|
| T101 | Config unit tests | ✅ |
| T102 | Model serde round-trip tests | ✅ |
| T103 | Path validation tests | ✅ |
| T104 | Schema contract tests | ✅ |
| T005 | Refactor GlobalConfig (rename workspace_root, add retention_days, serde defaults) | ✅ |
| T006 | Credential loading (keyring + env fallback) | ✅ |
| T007 | CLI args (--config, --workspace, --log-format) | ✅ |
| T008 | Session model (add workspace_root, terminated_at, progress_snapshot) | ✅ |
| T009 | ApprovalRequest model (from placeholder) | ✅ |
| T010 | Checkpoint model (from placeholder) | ✅ |
| T011 | ContinuationPrompt model (from placeholder) | ✅ |
| T012 | StallAlert model (from placeholder) | ✅ |
| T013 | WorkspacePolicy model (from placeholder) | ✅ |
| T014 | ProgressItem/ProgressStatus model (new) | ✅ |
| T015 | Session state machine validation | ✅ |
| T016 | SurrealDB connect refactor (delegate schema) | ✅ |
| T017 | Schema DDL with IF NOT EXISTS and ASSERT constraints | ✅ |
| T018 | SessionRepo (add update_progress_snapshot, set_terminated) | ✅ |
| T019 | ApprovalRepo (full CRUD) | ✅ |
| T020 | CheckpointRepo (full CRUD) | ✅ |
| T021 | PromptRepo (full CRUD) | ✅ |
| T022 | StallAlertRepo (new, full CRUD) | ✅ |
| T023 | Retention service (hourly cascade purge) | ✅ |
| T024 | Schema idempotency | ✅ |
| T025 | SlackService (add update_message, upload_file, open_modal) | ✅ |
| T026 | Block Kit builders (severity, actions, diff, text sections) | ✅ |
| T027 | Interaction dispatch (route by action_id prefix) | ✅ |
| T028 | Slack handler sub-modules (placeholders) | ✅ |
| T128 | Slack handlers directory structure | ✅ |
| T029 | AgentRemServer with AppState | ✅ |
| T030 | ToolContext for per-request context | ✅ |
| T031 | Stdio transport | ✅ |
| T032 | SSE transport | ✅ |
| T033 | MCP mod.rs exports | ✅ |
| T034 | Path safety with symlink detection | ✅ |
| T035 | Server bootstrap (main.rs) | ✅ |
| T036 | lib.rs module exports | ✅ |
| T037 | Integration verification | ✅ |

### Files Modified (29 existing)

- `Cargo.toml` — added reqwest, keyring, tokio-util dependencies
- `src/config.rs` — refactored config structure, added credential loading
- `src/main.rs` — full bootstrap with CLI, DB, retention, transports, shutdown
- `src/diff/mod.rs` — delegated to path_safety module
- `src/mcp/mod.rs` — added context, handler, sse, transport exports
- `src/models/mod.rs` — added progress module
- `src/models/session.rs` — added workspace_root, terminated_at, progress_snapshot
- `src/models/approval.rs` — implemented from placeholder
- `src/models/checkpoint.rs` — implemented from placeholder
- `src/models/prompt.rs` — implemented from placeholder
- `src/models/stall.rs` — implemented from placeholder
- `src/models/policy.rs` — implemented from placeholder
- `src/persistence/db.rs` — delegated schema to schema.rs
- `src/persistence/mod.rs` — added retention, schema, stall_repo
- `src/persistence/session_repo.rs` — added update_progress_snapshot, set_terminated
- `src/persistence/approval_repo.rs` — implemented from placeholder
- `src/persistence/checkpoint_repo.rs` — implemented from placeholder
- `src/persistence/prompt_repo.rs` — implemented from placeholder
- `src/slack/blocks.rs` — implemented Block Kit builders
- `src/slack/client.rs` — added update_message, upload_file, open_modal
- `src/slack/events.rs` — implemented interaction dispatch
- `src/slack/mod.rs` — added handlers module
- `tests/unit/config_tests.rs` — extended with 8 new tests
- `tests/unit/path_validation_tests.rs` — extended with 7 new tests
- `tests/unit/session_repo_tests.rs` — updated for schema changes

### Files Created (13 new)

- `src/models/progress.rs` — ProgressItem, ProgressStatus, validate_snapshot
- `src/diff/path_safety.rs` — validate_path with symlink detection
- `src/mcp/handler.rs` — AgentRemServer, AppState, 9 tool definitions
- `src/mcp/context.rs` — ToolContext struct
- `src/mcp/transport.rs` — serve_stdio
- `src/mcp/sse.rs` — serve_sse with axum
- `src/persistence/schema.rs` — SurrealDB DDL
- `src/persistence/retention.rs` — spawn_retention_task
- `src/persistence/stall_repo.rs` — StallAlertRepo
- `src/slack/handlers/mod.rs` — handler sub-module declarations
- `src/slack/handlers/approval.rs` — placeholder
- `src/slack/handlers/nudge.rs` — placeholder
- `src/slack/handlers/prompt.rs` — placeholder
- `tests/unit/model_tests.rs` — 16 round-trip tests
- `tests/contract/schema_tests.rs` — 3 schema contract tests

### Files Deleted (1)

- `src/mcp/server.rs` — replaced by handler.rs

### Build Verification

- `cargo check` — PASS
- `cargo check --tests` — PASS
- `cargo clippy -- -D warnings -D clippy::pedantic` — PASS
- `cargo test` — NOT RUNNABLE (MSVC linker not in PATH on this machine)

## Important Discoveries

### Slack-Morphism Type Names (slack-morphism 2.17)

The slack-morphism crate uses naming conventions that differ from what one might assume:
- `SlackActionBlockElement` (not `SlackBlockActionsElement`)
- `SlackSectionBlock` (not `SlackBlockSectionBlock`)
- `SlackActionsBlock` (not `SlackBlockActionsBlock`)
- `SlackApiFilesComplete` (not `SlackApiFilesCompleteUploadExternalFileRef`)
- Method: `.get_upload_url_external()` (not `.files_get_upload_url_external()`)
- `SlackFileUploadUrl` doesn't implement Display; access inner Url via `.0.to_string()`

### Reqwest Feature Names

For reqwest 0.13.2, the TLS feature is `rustls` (not `rustls-tls`). Must also use `--no-default-features` to avoid pulling in native-tls.

### SseServerConfig Fields (rmcp 0.5)

The `SseServerConfig` in rmcp 0.5 has these fields: `bind`, `sse_path`, `post_path`, `ct`, `sse_keep_alive`. There is no `cancel_on_close` field.

### Clippy Pedantic Findings

- `SurrealDB` in doc comments triggers `doc_markdown` lint — use backtick-quoted form.
- `file_path` in doc comments triggers `doc_markdown` lint — use backtick-quoted form.
- `_ =` pattern in `tokio::select!` triggers `ignored_unit_patterns` — use `() =`.
- `all_tools()` at ~200 lines triggers `too_many_lines` — allowed with `#[allow(clippy::too_many_lines)]`.

## Next Steps

- **Phase 3** (User Story 1 — Remote Code Review and Approval): Wire `ask_approval` and `accept_diff` tool handlers, implement the blocking oneshot pattern, connect approval flow to Slack buttons.
- **MSVC linker**: `cargo test` requires `link.exe` in PATH. Fix by running from VS Developer Command Prompt or adding MSVC build tools to PATH.
- **Slack client initialization**: `AppState.slack` is currently `Option::None` — Phase 3 should wire the `SlackService::start()` call during bootstrap.
- **Integration tests**: Schema contract tests exist but full integration tests with in-memory SurrealDB are blocked by the linker issue.

## Context to Preserve

- All MCP tool handlers return "not implemented" errors — ready for Phase 3 wiring.
- Slack handler sub-modules (`approval.rs`, `nudge.rs`, `prompt.rs`) are doc-comment-only placeholders.
- Retention service runs hourly; uses `terminated_at` field on sessions for cutoff calculation.
- Path safety module includes symlink escape detection (unix-only test).
- `ToolContext` is constructed per-request but not yet used by any handler — Phase 3 will wire it.

## ADRs Created

- ADR-0001: Credential loading via keyring with environment variable fallback
- ADR-0002: SurrealDB schema with IF NOT EXISTS for idempotent bootstrap
- ADR-0003: MCP server handler refactored from McpServer to AgentRemServer with AppState
