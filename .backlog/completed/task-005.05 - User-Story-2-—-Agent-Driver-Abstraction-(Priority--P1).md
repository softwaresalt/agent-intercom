---
id: TASK-005.05
title: "005 - User Story 2 — Agent Driver Abstraction (Priority: P1)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5050
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Protocol-agnostic `AgentDriver` trait with MCP implementation wrapping existing oneshot pattern

**Independent Test**: Mock driver resolves clearance/prompt/wait requests identically to current behavior

### Tests (S008–S017)

- [x] T022 [P] [US2] Write contract test for `McpDriver::resolve_clearance` approved/rejected in `tests/contract/driver_contract_tests.rs` — covers S008, S009
- [x] T023 [P] [US2] Write contract test for `resolve_clearance` with unknown request_id in `tests/contract/driver_contract_tests.rs` — covers S012
- [x] T024 [P] [US2] Write unit test for driver `interrupt` on terminated session (idempotent) in `tests/unit/driver_trait_tests.rs` — covers S016
- [x] T025 [P] [US2] Write concurrent resolution test (two requests resolved simultaneously) in `tests/unit/driver_trait_tests.rs` — covers S017

### Implementation

- [x] T026 [US2] Define `AgentDriver` trait in `src/driver/mod.rs` with 5 methods: `resolve_clearance`, `send_prompt`, `interrupt`, `resolve_prompt`, `resolve_wait`
- [x] T027 [US2] Implement `McpDriver` in `src/driver/mcp_driver.rs` — wraps existing `PendingApprovals`, `PendingPrompts`, `PendingWaits` oneshot maps
- [x] T028 [US2] Wire `McpDriver` as `Arc<dyn AgentDriver>` into `AppState` in `src/mcp/handler.rs`
- [x] T029 [US2] Refactor Slack approval handler in `src/slack/handlers/` to call `driver.resolve_clearance()` instead of directly accessing oneshot maps
- [x] T030 [US2] Refactor Slack prompt handler to call `driver.resolve_prompt()` instead of directly accessing oneshot maps
- [x] T031 [US2] Refactor Slack wait handler to call `driver.resolve_wait()` instead of directly accessing oneshot maps

**Checkpoint**: All Slack handlers route through AgentDriver trait; MCP behavior identical to before refactor

---

<!-- SECTION:DESCRIPTION:END -->
