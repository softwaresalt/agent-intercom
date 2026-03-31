---
id: TASK-002.05
title: "002 - User Story 4 — Data Retention Purge (Priority: P2)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-002
dependencies: []
ordinal: 2050
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Background retention task deletes expired sessions and all child records using SQLite SQL.

**Independent Test**: Create sessions with old termination timestamps, run purge, confirm cascading deletion in correct order.

### Tests (write first, confirm they fail)

- [x] T042 [US4] Write retention purge test in tests/integration/retention_tests.rs: create expired sessions with children, run `purge_expired()`, verify cascading deletion order (stall_alerts → checkpoints → prompts → approvals → sessions)

### Implementation

- [x] T043 [US4] Rewrite src/persistence/retention.rs: replace SurrealQL DELETE with SQLite `DELETE FROM ... WHERE session_id IN (SELECT id FROM session WHERE terminated_at < ? AND terminated_at IS NOT NULL)` — cascade order: stall_alert, checkpoint, continuation_prompt, approval_request, session

**Checkpoint**: Retention purge test passes. Expired sessions and all children deleted. Active/recent sessions untouched. US4 is complete.

---

<!-- SECTION:DESCRIPTION:END -->
