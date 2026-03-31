---
id: TASK-008.03
title: "008 - Simulated Interaction Dispatch"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-008
dependencies: []
ordinal: 8030
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Verify handler pipeline processes synthetic operator actions correctly (SC-002 for Tier 1).

**Depends on**: Phase 1 (test infrastructure).

### Tasks

- [X] **2.1** Create `tests/integration/slack_interaction_tests.rs`
- [X] Build mock `AppState` with in-memory DB and registered oneshot channels
- [X] Test approval accept: dispatch synthetic action → verify oneshot resolved, DB updated
- [X] Test approval reject: same pattern
- Scenarios: S-T1-009, S-T1-010
- FRs: FR-002, FR-009

- [X] **2.2** Add prompt interaction tests to `slack_interaction_tests.rs`
- [X] Test prompt continue: dispatch → verify oneshot resolved
- [X] Test prompt stop: dispatch → verify oneshot resolved
- [X] Test nudge: dispatch → verify stall resolved
- [X] Test wait resume: dispatch → verify standby resolved
- Scenarios: S-T1-011, S-T1-025, S-T1-026
- FRs: FR-002, FR-009

- [X] **2.3** Create `tests/integration/slack_modal_flow_tests.rs`
- [X] Test prompt refine → modal open path (with `state.slack = None`, verify fallback activates)
- [X] Test modal submission → prompt resolution with instruction text
- Scenarios: S-T1-012, S-T1-013
- FRs: FR-002, FR-009, FR-011

- [X] **2.4** Create `tests/unit/command_routing_tests.rs`
- [X] Test `/acom` prefix routing for MCP mode
- [X] Test `/arc` prefix routing for ACP mode
- [X] Test mode gating: ACP-only command rejected in MCP mode
- [X] Test malformed arguments → usage message
- Scenarios: S-T1-021, S-T1-022, S-T1-023
- FRs: FR-005

- [X] **2.5** Register new test modules in `tests/integration/mod.rs`
- [X] Added `mod slack_interaction_tests;` and `mod slack_modal_flow_tests;`
- [X] Added `mod command_routing_tests;` in `tests/unit.rs`

### Constitution Gate

- [X] All interaction tests pass: `cargo test -- slack_interaction` (8 passed)
- [X] All modal flow tests pass: `cargo test -- slack_modal` (5 passed)
- [X] All command routing tests pass: `cargo test -- command_routing` (12 passed)
- [X] Clippy clean: `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`

---

<!-- SECTION:DESCRIPTION:END -->
