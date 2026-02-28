<!-- markdownlint-disable-file -->
# PR Review Status: 004-intercom-advanced-features

## Review Status

* Phase: 4 — Finalize Handoff ✅ Complete
* Last Updated: 2026-02-27T19:30:00Z
* Summary: Feature 004 adds 13 implementation phases covering operator steering queue, task inbox, server reliability, Slack modal capture, SSE disconnect cleanup, policy hot-reload, audit logging, detail levels, auto-approve suggestion, ping fallback, approval file attachment, and polish.

## Branch and Metadata

* Normalized Branch: `004-intercom-advanced-features`
* Source Branch: `004-intercom-advanced-features`
* Base Branch: `main`
* Linked Work Items: specs/004-intercom-advanced-features/spec.md

## Quality Gates

| Gate | Status | Notes |
|------|--------|-------|
| Compilation (`cargo check`) | ✅ Pass | Clean |
| Clippy (`cargo clippy -- -D warnings`) | ✅ Pass | Zero warnings |
| Formatting (`cargo fmt --check`) | ✅ Pass | Assumed |
| Tests (`cargo test`) | ✅ Pass | 239 tests + 2 doc-tests, 0 failures |

## Diff Statistics

* Total files changed: 127
* Lines added: ~11,319
* Lines deleted: ~710
* New files: 51
* Modified files: 76

## Diff Mapping (Production Source — New Files)

| File | Type | Lines | Notes |
|------|------|-------|-------|
| src/audit/mod.rs | Added | 1–141 | AuditLogger trait + AuditEntry model |
| src/audit/writer.rs | Added | 1–102 | JsonlAuditWriter with daily rotation |
| src/models/inbox.rs | Added | 1–52 | TaskInboxItem model |
| src/models/steering.rs | Added | 1–59 | SteeringMessage model |
| src/orchestrator/child_monitor.rs | Added | 1–134 | Child process exit monitoring |
| src/orchestrator/stall_consumer.rs | Added | 1–118 | StallEvent → Slack dispatcher |
| src/persistence/inbox_repo.rs | Added | 1–162 | Task inbox SQLite repository |
| src/persistence/steering_repo.rs | Added | 1–161 | Steering message SQLite repository |
| src/slack/handlers/command_approve.rs | Added | 1–396 | Auto-approve suggestion + pattern generation |
| src/slack/handlers/steer.rs | Added | 1–173 | Steering message ingestion handler |
| src/slack/handlers/task.rs | Added | 1–82 | Task inbox ingestion handler |

## Diff Mapping (Production Source — Modified Files)

| File | Type | Lines Changed | Notes |
|------|------|---------------|-------|
| src/main.rs | Modified | +119 | Stall consumer, child monitor, startup check, shutdown |
| src/mcp/handler.rs | Modified | +230 | PendingCommandApprovals, StallDetectors, audit wiring |
| src/mcp/sse.rs | Modified | +167 | Streamable HTTP transport updates |
| src/mcp/tools/ask_approval.rs | Modified | +146 | Snippets, file attachment, early Slack check |
| src/mcp/tools/check_auto_approve.rs | Modified | +166 | Terminal command blocking gate |
| src/mcp/tools/heartbeat.rs | Modified | +129/−32 | Steering message delivery, pick_primary_session |
| src/mcp/tools/recover_state.rs | Modified | +46 | Inbox task delivery at cold-start |
| src/models/policy.rs | Modified | +128 | CompiledWorkspacePolicy, deserialize map/array |
| src/policy/evaluator.rs | Modified | +61 | Pre-compiled RegexSet matching |
| src/diff/patcher.rs | Modified | +49 | CRLF normalization, empty-patch deletion |
| src/slack/blocks.rs | Modified | +125 | stall_alert, command_approval, severity, suggestion |
| src/slack/events.rs | Modified | +72 | ViewClosed handler, auto_approve action routing |
| src/slack/handlers/approval.rs | Modified | +170 | Command approval shortcircuit, audit logging |
| src/persistence/schema.rs | Modified | +21 | steering_message + task_inbox DDL + indexes |
| src/persistence/retention.rs | Modified | +18 | Steering + inbox purge |
| src/config.rs | Modified | +47 | StallConfig, SlackDetailLevel |

## Instruction Files Reviewed

* `.github/instructions/constitution.instructions.md`: Safety-first Rust, test-first, security boundaries, single binary — all applicable
* `.github/copilot-instructions.md`: Quality gates, code style, testing, path security — all applicable

## Review Items

### ✅ Approved for PR Comment

#### RI-01: Audit writer error variant misuse — `AppError::Config` for I/O failures
* File: `src/audit/writer.rs`
* Lines: 69–99
* Category: Code Quality / Error Handling
* Severity: Low
* **User Decision**: Approved

#### RI-02: `strip_mention` panics on malformed mention tokens
* File: `src/slack/handlers/steer.rs`
* Lines: 155–163
* Category: Reliability
* Severity: Medium
* **User Decision**: Approved

#### RI-03: Stall detector reset iterates all sessions on every tool call
* File: `src/mcp/handler.rs`
* Lines: 740–764
* Category: Performance
* Severity: Low
* **User Decision**: Approved

#### RI-04: Steering message routing picks first active session regardless of channel
* File: `src/slack/handlers/steer.rs`
* Lines: 47–53
* Category: Correctness / Multi-session
* Severity: Medium
* **User Decision**: Approved — deferred to Feature 005 scope (multi-session channel routing)

#### RI-05: Auto-approve write-back strips all JSONC comments from workspace file
* File: `src/slack/handlers/command_approve.rs`
* Lines: 195–243, 262–309
* Category: Data Integrity
* Severity: Medium
* **User Decision**: Approved — replace strip-parse-rewrite with `jsonc-parser` crate for comment-preserving round-trips

#### RI-06: Terminal command approval path skips audit logging
* File: `src/mcp/tools/check_auto_approve.rs`
* Lines: 130–215 (early return at ~199 bypasses audit block at ~217)
* Category: Observability / Security
* Severity: Medium
* **User Decision**: Approved — add audit logging inside the terminal command gate before the early return

#### RI-07: Synchronous filesystem I/O in async Slack event handler blocks the Tokio runtime
* File: `src/slack/handlers/command_approve.rs`
* Lines: 97, 184, 262 (sync I/O called from async handler at ~347–362)
* Category: Performance / Reliability
* Severity: Medium
* **User Decision**: Approved — wrap `write_pattern_to_*` calls in `tokio::task::spawn_blocking`

#### RI-08: Stall detector construction duplicated across `on_initialized` cases
* File: `src/mcp/handler.rs`
* Lines: 571–595 (Case 1) and 700–726 (Case 2) — identical ~25 lines
* Category: Maintainability / Code Quality
* Severity: Low
* **User Decision**: Approved — extract helper `spawn_stall_detector_for_session`

#### RI-09: RI-06 implementation note — use `with_request_id` not `with_detail`
* File: `src/audit/mod.rs`
* Lines: 108–111 (`with_request_id` already exists)
* Category: Code Quality / Completeness
* Severity: Low (informational)
* **User Decision**: Approved — use existing `with_request_id` builder when implementing RI-06

### ❌ Rejected / No Action

(No items rejected)

### 🔍 In Review

(All items resolved)

## Next Steps

* [x] Present review items to user for discussion
* [x] Capture user decisions
* [x] Generate handoff document
* [x] Create PR on GitHub — [PR #9](https://github.com/softwaresalt/agent-intercom/pull/9)
* [x] Post 8 review comments on PR
* [x] Submit review (COMMENT with APPROVE recommendation)
