---
id: TASK-008.06
title: "008 - Live Message & Interaction Tests"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-008
dependencies: []
ordinal: 8060
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Tier 2 message posting, threading, and interaction round-trips (SC-006, SC-008).

**Depends on**: Phase 4 (live harness).

### Tasks

- [X] **5.1** Complete `tests/live/live_message_tests.rs`
- Post approval message → verify structure via API
- Post to thread → verify via `conversations.replies`
- Post to multiple sessions → verify correct threading
- Scenarios: S-T2-001, S-T2-002, S-T2-003
- FRs: FR-013, FR-018

- [X] **5.2** Create `tests/live/live_interaction_tests.rs`
- Approval accept round-trip: post approval → dispatch synthetic accept → verify DB + follow-up message
- Prompt continue round-trip
- Stall nudge round-trip
- Button replacement: verify message updated after action
- Scenarios: S-T2-004, S-T2-005, S-T2-010, S-T2-013
- FRs: FR-014

- [X] **5.3** Create `tests/live/live_threading_tests.rs`
- Multi-session thread isolation in real Slack
- Scenarios: S-T2-003
- FRs: FR-018

- [X] **5.4** Create `tests/live/live_command_tests.rs`
- Slash command dispatch and response verification
- Scenario: S-T2-012

- [X] **5.5** Add rate limit handling test
- Post messages in rapid succession → verify server handles backoff
- Scenario: S-T2-009

### Constitution Gate

- [X] All live tests pass with credentials: `cargo test --features live-slack-tests`
- [X] Clippy clean
- [X] SC-006: all Tier 2 scenarios produce structured results
- [X] SC-008: threaded messages verified in correct threads

---

<!-- SECTION:DESCRIPTION:END -->
