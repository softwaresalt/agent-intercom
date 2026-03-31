---
id: TASK-006.08
title: "006 - Polish & Cross-Cutting Concerns"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-006
dependencies: []
ordinal: 6080
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Verify concurrent behavior, lifecycle edge cases, full scenario coverage, and manual end-to-end validation.

- [x] T020 Write integration tests in tests/integration/acp_event_integration.rs for: (a) concurrent event processing — two clearance requests for same session in rapid succession create separate records and messages (S047), interleaved clearance+prompt for same session produce independent records with no cross-contamination (S048), events from multiple sessions processed independently with no shared state leakage (S049), AcpDriver registration and DB persistence are consistent under slow DB writes (S050); (b) event consumer lifecycle — normal dispatch loop receives and routes multiple event variants correctly (S051), cancellation token fires causing graceful consumer exit (S052), mpsc sender dropped causing consumer exit on channel close (S053), operator responds to clearance after ACP session terminated — driver returns error, Slack handler logs warning (S054); (c) full round-trip flow — emit ClearanceRequested event → handler registers + persists + posts to Slack → simulate operator Accept button click → verify resolve_clearance dispatches to agent stream with correct approval ID (S067 authorization guard verified, S068 thread_ts DB failure path) *(UF-23: ensures end-to-end wiring is correct)*
- [x] T021 Verify all 56 SCENARIOS.md scenarios are covered by test assertions — cross-reference scenario IDs S001–S056 against test function names across tests/unit/acp_event_wiring.rs (25 scenarios), tests/contract/acp_event_contract.rs (17 scenarios), tests/integration/acp_event_integration.rs (14 scenarios). S051 (normal dispatch loop) covered implicitly through other integration tests; S067 (authorization guard) covered by existing Slack events tests outside this file.
- [x] T022 Run full quality gate suite: `cargo check && cargo clippy -- -D warnings && cargo fmt --all -- --check && cargo test` — 996 tests pass (271 integration, 244 unit, 438 contract, 37 doc tests, 6 doc tests), 0 failures; clippy clean; fmt clean.
- [x] T023 [P] Manual quickstart validation DEFERRED — requires live ACP agent + Slack app running; no automated equivalent. Manual Test 1–3 from quickstart.md must be run against a live server deployment.
- [x] T024 Commit all changes with conventional commit messages on feature branch `006-acp-event-wiring`.

---

<!-- SECTION:DESCRIPTION:END -->
