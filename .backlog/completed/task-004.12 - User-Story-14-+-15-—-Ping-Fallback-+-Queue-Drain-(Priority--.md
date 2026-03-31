---
id: TASK-004.12
title: "004 - User Story 14 + 15 — Ping Fallback + Queue Drain (Priority: P4)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-004
dependencies: []
ordinal: 4120
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Ping resilient to stale sessions; shutdown drains queue unconditionally

### Tests for US14/US15

- [x] T069 [P] [US14] Unit test for ping fallback in `tests/unit/heartbeat_tests.rs` (scenarios S080-S082)
- [x] T070 [P] [US15] Integration test for unconditional drain in `tests/integration/shutdown_tests.rs` (scenarios S083-S086)

### Implementation for US14/US15

- [x] T071 [US14] Update `src/mcp/tools/heartbeat.rs` — sort active sessions by `updated_at DESC`, pick first
- [x] T072 [US15] Update `src/main.rs` — move queue drain to `shutdown_with_timeout`, run unconditionally

**Checkpoint**: Ping handles stale sessions gracefully; shutdown drains all messages

---

<!-- SECTION:DESCRIPTION:END -->
