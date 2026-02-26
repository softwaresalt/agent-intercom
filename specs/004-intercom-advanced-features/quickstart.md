# Quickstart: Intercom Advanced Features

**Feature**: 004-intercom-advanced-features
**Date**: 2026-02-26

## Prerequisites

- Rust stable toolchain (edition 2021)
- Existing agent-intercom codebase on `main` branch
- SQLite (bundled via sqlx)
- Slack app configured with Socket Mode

## Implementation Order

Work in this order to minimize blocked dependencies:

### Layer 1 — Foundation (no cross-dependencies)

1. **Schema additions** (`persistence/schema.rs`): Add `steering_message` and `task_inbox` CREATE TABLE statements to `bootstrap_schema()`.
2. **New models** (`models/steering.rs`, `models/inbox.rs`): Define `SteeringMessage` and `TaskInboxItem` structs.
3. **New repos** (`persistence/steering_repo.rs`, `persistence/inbox_repo.rs`): CRUD operations for both tables.
4. **Audit module** (`audit/mod.rs`, `audit/writer.rs`): `AuditLogger` trait + `JsonlAuditWriter` with daily rotation.
5. **CompiledWorkspacePolicy** (`models/policy.rs`): New struct wrapping `WorkspacePolicy` + `RegexSet`.
6. **PolicyLoader update** (`policy/loader.rs`): Return `CompiledWorkspacePolicy`.

### Layer 2 — Wiring (depends on Layer 1)

7. **AppState additions** (`mcp/handler.rs`): Add `PolicyCache` and `AuditLogger` to `AppState`.
8. **PolicyEvaluator update** (`policy/evaluator.rs`): Replace per-call `Regex::new()` with `RegexSet::matches()`.
9. **PolicyWatcher wiring** (`policy/watcher.rs`): Wire `PolicyCache` into `AppState`.
10. **auto_check update** (`mcp/tools/check_auto_approve.rs`): Read from `PolicyCache`.
11. **Config update** (`config.rs`): Add `slack_detail_level` field.

### Layer 3 — Features (depends on Layer 2)

12. **Steering via ping** (`mcp/tools/heartbeat.rs`): Fetch unconsumed steering messages, include in response, mark consumed.
13. **Inbox via reboot** (`mcp/tools/recover_state.rs`): Fetch unconsumed inbox items for channel, include in response, mark consumed.
14. **Ping fallback** (`mcp/tools/heartbeat.rs`): Sort multiple active sessions by `updated_at DESC`, pick first.
15. **Slack slash commands** (`slack/commands.rs`): Add `/intercom steer` and `/intercom task`.
16. **IPC commands** (`ipc/server.rs`): Add `steer` and `task` commands.
17. **CTL subcommands** (`ctl/main.rs`): Add `steer` and `task` clap subcommands.
18. **Server startup** (`main.rs`): Exit on bind failure, single-instance check.
19. **SSE disconnect** (`mcp/sse.rs`): Hook stream close to session termination.
20. **Queue drain** (`main.rs` or `slack/client.rs`): Unconditional drain on shutdown.
21. **Approval file attachment** (`mcp/tools/ask_approval.rs`): Upload original file content as Slack attachment alongside diff.

### Layer 4 — Slack Enhancements (depends on Layer 3)

21. **Modal builders** (`slack/blocks.rs`): Add instruction input modal view.
22. **ViewSubmission handler** (`slack/events.rs`): New match arm for modal submissions.
23. **Modal flow for wait/prompt** (`slack/handlers/wait.rs`, `slack/handlers/prompt.rs`): Thread `trigger_id`, open modal, resolve oneshot on submission.
24. **Detail levels** (`slack/blocks.rs`, `slack/client.rs`): Apply configured detail level to message builders.
25. **Failure reporting** (`orchestrator/stall_detector.rs`): Send Slack notification on stall/crash.
26. **Auto-approve suggestion** (`slack/handlers/command_approve.rs`, `slack/blocks.rs`): Suggest adding command to policy after manual approval.

### Layer 5 — Documentation and Prompts

27. **Config documentation** (`docs/configuration.md`, `README.md`): Comprehensive config.toml breakdown.
28. **Heartbeat loop prompt** (`.github/prompts/ping-loop.prompt.md`): Reusable agent prompt template.
29. **Audit logging integration**: Wire `AuditLogger` into tool handlers, approval flow, session lifecycle.

## Key Patterns

### MCP Tool Response Extension

Extend ping response with steering messages: fetch unconsumed from repo, include in JSON response, mark consumed in same transaction.

### Approval Workflow Auto-Check

Read from PolicyCache (in AppState) instead of PolicyLoader::load() for pre-compiled regex matching.

### Audit Logging

Log after every tool call and approval decision via AuditLogger trait on AppState.

## Testing Strategy

- **Unit tests first** for repos, audit writer, compiled policy
- **Contract tests** for extended ping/reboot responses
- **Integration tests** for end-to-end steering and inbox flows
- All tests use in-memory SQLite
