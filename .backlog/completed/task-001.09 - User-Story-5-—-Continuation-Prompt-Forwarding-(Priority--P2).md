---
id: TASK-001.09
title: "001 - User Story 5 — Continuation Prompt Forwarding (Priority: P2)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001
dependencies: []
ordinal: 1090
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Agent-generated continuation prompts forwarded to Slack with Continue/Refine/Stop buttons

**Independent Test**: Invoke `forward_prompt` with a continuation prompt, verify it appears in Slack with three buttons, tap Continue, verify agent receives decision

### Tests (Constitution Principle III)

- [X] T114 Write contract tests for `forward_prompt` tool in `tests/contract/forward_prompt_tests.rs`: validate input/output schemas per mcp-tools.json; test all `prompt_type` values and `decision` enum values
- [X] T115 Write integration test for prompt→decision flow in `tests/integration/prompt_flow_tests.rs`: forward prompt → verify DB record → simulate Continue → verify oneshot resolves; repeat for Refine (with instruction) and Stop; test auto-timeout returns `continue`

### Implementation for User Story 5

- [X] T057 [US5] Implement `forward_prompt` MCP tool handler in `src/mcp/tools/forward_prompt.rs`: accept `prompt_text`, `prompt_type`, `elapsed_seconds`, `actions_taken` per mcp-tools.json contract; create `ContinuationPrompt` record in DB; post prompt to Slack with Continue/Refine/Stop buttons and elapsed time context; block on `oneshot` channel until response or `timeouts.prompt_seconds` elapses; on timeout, auto-respond with `continue` decision and post timeout notification (FR-008); return `{decision, instruction}` per contract
- [X] T058 [US5] Implement prompt interaction callback in `src/slack/handlers/prompt.rs`: handle Continue, Refine, and Stop button presses; for Continue: resolve oneshot with `continue`; for Refine: open modal dialog for revised instruction text, on submission resolve with `refine` + instruction; for Stop: resolve with `stop`; update DB record with decision; replace buttons with status text (FR-022)
- [X] T059 [US5] Wire pending prompt map in `src/mcp/handler.rs`: maintain `HashMap<String, oneshot::Sender<PromptResponse>>` keyed by `prompt_id` in shared state, similar to approval pattern
- [X] T060 [US5] Add tracing spans to `forward_prompt`: span with `prompt_type`, `prompt_id` attributes; log decision outcome

**Checkpoint**: Continuation prompts forwarded and resolved from Slack

---

<!-- SECTION:DESCRIPTION:END -->
