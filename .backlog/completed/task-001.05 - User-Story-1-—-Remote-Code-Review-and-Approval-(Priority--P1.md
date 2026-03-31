---
id: TASK-001.05
title: "001 - User Story 1 — Remote Code Review and Approval (Priority: P1) 🎯 MVP"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001
dependencies: []
ordinal: 1050
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

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

<!-- SECTION:DESCRIPTION:END -->
