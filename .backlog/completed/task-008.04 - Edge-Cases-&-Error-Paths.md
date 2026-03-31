---
id: TASK-008.04
title: "008 - Edge Cases & Error Paths"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-008
dependencies: []
ordinal: 8040
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Cover error paths, guard logic, and edge cases (FR-003, FR-004, FR-010, FR-012).

**Depends on**: Phase 2 (interaction dispatch infrastructure).

### Tasks

- [X] **3.1** Add authorization guard tests to `slack_interaction_tests.rs`
- [X] Test unauthorized user → action rejected, no state change
- [X] Test authorized user → action proceeds
- Scenarios: S-T1-015, S-T1-016
- FRs: FR-003

- [X] **3.2** Add double-submission prevention test
- [X] Dispatch same action twice → first resolves, second silently ignored
- Scenario: S-T1-014
- FRs: FR-004

- [X] **3.3** Create `tests/integration/slack_fallback_tests.rs`
- [X] Test thread-reply fallback: register pending, send reply → oneshot resolved
- [X] Test orphaned thread reply: no pending → ignored gracefully
- Scenarios: S-T1-017, S-T1-018
- FRs: FR-010, FR-011

- [X] **3.4** Add error path tests to `slack_interaction_tests.rs`
- [X] Unknown action_id → graceful handling
- [X] Stale session reference → graceful error
- [X] Consumed oneshot channel → graceful handling
- Scenarios: S-T1-019, S-T1-020, S-T1-027
- FRs: FR-010, FR-012

- [X] **3.5** Create `tests/integration/slack_threading_tests.rs`
- [X] Two sessions in same channel with different thread_ts
- [X] Button action in Session A → only Session A affected
- Scenario: S-T1-024
- FRs: FR-006

### Constitution Gate

- [X] All error path tests pass: `cargo test -- slack_fallback slack_threading`
- [X] Clippy clean
- [X] No panics in any error path test
- [X] SC-002 Tier 1 portion complete: all 6 interaction types have simulated tests

---

<!-- SECTION:DESCRIPTION:END -->
