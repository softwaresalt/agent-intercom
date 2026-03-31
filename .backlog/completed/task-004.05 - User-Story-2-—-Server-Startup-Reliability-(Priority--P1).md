---
id: TASK-004.05
title: "004 - User Story 2 — Server Startup Reliability (Priority: P1)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-004
dependencies: []
ordinal: 4050
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Server exits cleanly on port conflict; single-instance enforcement

**Independent Test**: Start two instances, verify second exits with clear error

### Tests for US2

- [X] T024 [P] [US2] Integration test for bind failure in `tests/integration/startup_tests.rs` (scenarios S024-S026)
- [X] T025 [P] [US2] Integration test for normal startup in `tests/integration/startup_tests.rs` (scenario S023)

### Implementation for US2

- [X] T026 [US2] Update `src/main.rs` — if HTTP transport bind fails, log error and `std::process::exit(1)`
- [X] T027 [US2] Update `src/main.rs` — shut down already-started services (Slack) before exit on bind failure

**Checkpoint**: No more zombie processes on port conflict

---

<!-- SECTION:DESCRIPTION:END -->
