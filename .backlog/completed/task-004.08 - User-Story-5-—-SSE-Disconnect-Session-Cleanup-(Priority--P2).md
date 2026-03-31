---
id: TASK-004.08
title: "004 - User Story 5 — SSE Disconnect Session Cleanup (Priority: P2)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-004
dependencies: []
ordinal: 4080
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Disconnected sessions marked terminated, not left as active indefinitely

**Independent Test**: Connect agent, force-close connection, verify session status changes

### Tests for US5

- [x] T044 [P] [US5] Integration test for disconnect detection in `tests/integration/disconnect_tests.rs` (scenarios S037-S039)

### Implementation for US5

- [x] T045 [US5] Hook stream close event in `src/mcp/sse.rs` — trigger `session_repo.set_terminated()` on connection drop
- [x] T046 [US5] Ensure session lookup by transport session ID is available for cleanup

**Checkpoint**: Stale sessions cleaned up promptly on disconnect

---

<!-- SECTION:DESCRIPTION:END -->
