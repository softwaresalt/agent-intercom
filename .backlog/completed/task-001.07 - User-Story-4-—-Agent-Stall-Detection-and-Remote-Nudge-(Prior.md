---
id: TASK-001.07
title: "001 - User Story 4 — Agent Stall Detection and Remote Nudge (Priority: P1)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001
dependencies: []
ordinal: 1070
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

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

<!-- SECTION:DESCRIPTION:END -->
