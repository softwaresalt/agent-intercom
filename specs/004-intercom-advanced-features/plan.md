# Implementation Plan: Intercom Advanced Features

**Branch**: `004-intercom-advanced-features` | **Date**: 2026-02-26 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/004-intercom-advanced-features/spec.md`

## Summary

This feature set adds 16 capabilities to agent-intercom: an operator steering queue for proactive agent communication via `ping`, a task inbox for cold-start work queuing, server startup reliability (single-instance enforcement, bind-failure exit), Slack modal instruction capture (replacing placeholder strings), SSE disconnect session cleanup, policy hot-reload wiring, structured audit logging (JSONL with daily rotation), agent failure reporting to Slack, configuration documentation, configurable Slack message detail levels, auto-approve command suggestions, a heartbeat loop prompt template, policy regex pre-compilation, ping fallback to most-recent session, Slack queue drain race fix, and approval file attachment (uploading original file content alongside diffs for informed operator review).

## Technical Context

**Language/Version**: Rust stable, edition 2021
**Primary Dependencies**: rmcp 0.13, axum 0.8, slack-morphism 2.17, sqlx 0.8, tokio 1.37, notify 6.1, interprocess 2.0, clap 4.5, regex (existing), serde/serde_json 1.0, chrono 0.4, uuid 1.7
**Storage**: SQLite via sqlx 0.8 (file-based prod, in-memory tests). New tables: `steering_message`, `task_inbox`. Audit logs written to filesystem (JSONL).
**Testing**: cargo test (unit/, contract/, integration/ tiers). TDD required.
**Target Platform**: Windows (primary), Linux/macOS (secondary)
**Project Type**: Single workspace, two binaries (agent-intercom, agent-intercom-ctl)
**Performance Goals**: Steering message delivery < 5s from send to ping response. Auto-check response time O(1) with pre-compiled regex. Modal open < 3s (Slack trigger_id window).
**Constraints**: No new external dependencies unless justified. Single-binary simplicity. All paths validated within workspace root.
**Scale/Scope**: Single operator, 1-5 concurrent agent sessions typical.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Safety-First Rust | Pass | All new code in Rust stable. No unsafe. Result/AppError pattern for all fallible ops. Clippy pedantic. |
| II. MCP Protocol Fidelity | Pass | Steering queue delivered via existing `ping` tool response extension. No new tools hidden conditionally. Task inbox delivered via `reboot` or new visible tool. |
| III. Test-First Development | Pass | TDD required. New unit tests for queue repos, contract tests for extended ping/reboot responses, integration tests for end-to-end steering and inbox flows. |
| IV. Security Boundary Enforcement | Pass | Audit log paths validated within workspace. Steering messages authorized via existing Slack auth guard. IPC commands via existing named pipe security. |
| V. Structured Observability | Pass | All new operations emit tracing spans. Audit logging adds persistent structured records beyond tracing. |
| VI. Single-Binary Simplicity | Pass | Two new DB tables (steering_message, task_inbox) in existing SQLite. Audit logs to filesystem. No new binaries or databases. New IPC commands and slash commands extend existing infrastructure. |

No violations. All features align with constitution principles.

## Project Structure

### Documentation (this feature)

```text
specs/004-intercom-advanced-features/
├── plan.md              # This file
├── research.md          # Phase 0: research findings
├── data-model.md        # Phase 1: entity models
├── quickstart.md        # Phase 1: implementation quickstart
├── contracts/           # Phase 1: MCP tool schema extensions
│   ├── ping-response.md
│   ├── reboot-response.md
│   ├── steering-queue.md
│   └── task-inbox.md
└── tasks.md             # Phase 2: task breakdown
```

### Source Code (repository root)

```text
src/
├── config.rs               # + detail_level config field
├── main.rs                 # + single-instance check, bind-failure exit
├── mcp/
│   ├── handler.rs          # + PolicyCache in AppState
│   └── tools/
│       ├── heartbeat.rs    # + steering message delivery in ping, fallback logic
│       ├── check_auto_approve.rs  # + read from PolicyCache, compiled regex
│       ├── ask_approval.rs        # + original file attachment upload
│       └── recover_state.rs       # + task inbox delivery in reboot
├── models/
│   ├── steering.rs         # NEW: SteeringMessage model
│   ├── inbox.rs            # NEW: TaskInboxItem model
│   └── policy.rs           # + CompiledWorkspacePolicy
├── orchestrator/
│   └── stall_detector.rs   # + Slack failure notification
├── persistence/
│   ├── schema.rs           # + steering_message, task_inbox DDL
│   ├── steering_repo.rs    # NEW: steering queue CRUD
│   └── inbox_repo.rs       # NEW: task inbox CRUD
├── policy/
│   ├── evaluator.rs        # + RegexSet-based matching
│   ├── loader.rs           # + CompiledWorkspacePolicy return
│   └── watcher.rs          # + PolicyCache wired to AppState
├── slack/
│   ├── blocks.rs           # + modal view builder, auto-approve suggestion button
│   ├── client.rs           # + unconditional queue drain
│   ├── commands.rs         # + /intercom steer, /intercom task
│   ├── events.rs           # + ViewSubmission handler
│   └── handlers/
│       ├── prompt.rs       # + trigger_id → modal flow
│       ├── wait.rs         # + trigger_id → modal flow
│       ├── steer.rs        # NEW: steering message ingestion
│       └── command_approve.rs  # NEW: auto-approve suggestion flow
├── ipc/
│   └── server.rs           # + steer, task IPC commands
├── mcp/
│   └── sse.rs              # + stream close → session termination
└── audit/                  # NEW module
    ├── mod.rs              # AuditLogger trait + JSONL implementation
    └── writer.rs           # Daily-rotating JSONL file writer

ctl/
└── main.rs                 # + steer, task subcommands

tests/
├── unit/
│   ├── steering_repo_tests.rs      # NEW
│   ├── inbox_repo_tests.rs         # NEW
│   ├── audit_writer_tests.rs       # NEW
│   ├── policy_evaluator_tests.rs   # UPDATED (RegexSet)
│   └── policy_tests.rs             # UPDATED (CompiledWorkspacePolicy)
├── contract/
│   ├── ping_contract_tests.rs      # UPDATED (steering in response)
│   ├── reboot_contract_tests.rs    # UPDATED (inbox in response)
│   └── auto_check_contract_tests.rs  # UPDATED (cached policy)
└── integration/
    ├── steering_flow_tests.rs      # NEW: end-to-end steering
    ├── inbox_flow_tests.rs         # NEW: end-to-end inbox
    └── startup_tests.rs            # NEW: single-instance, bind failure

docs/
└── configuration.md        # NEW: comprehensive config documentation

.github/
└── prompts/
    └── ping-loop.prompt.md  # NEW: heartbeat loop pattern template
```

**Structure Decision**: Extends the existing single-workspace layout. New modules `src/audit/` and `src/models/steering.rs`, `src/models/inbox.rs` follow established patterns. No new binaries or workspaces.

## Complexity Tracking

No constitution violations to justify.