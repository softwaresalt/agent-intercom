---
id: TASK-002.08
title: "002 - Dependencies & Execution Order"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-002
dependencies: []
ordinal: 2080
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user stories
- **Phase 3 (US1)**: Depends on Phase 2 — primary implementation phase
- **Phase 4 (US4)**: Depends on Phase 2 — can run in parallel with Phase 3
- **Phase 5 (US5)**: Depends on Phase 3 + Phase 4 — verification after all code changes
- **Phase 6 (Polish)**: Depends on Phase 5 — final validation

### User Story Dependencies

- **US2 + US3 (Foundation)**: No dependencies on other stories — delivers connect, schema, in-memory
- **US1 (P1)**: Depends on US2 + US3 (needs working connection + schema to rewrite repos)
- **US4 (P2)**: Depends on US2 + US3 (needs working connection + schema for retention queries). Independent of US1 — can be done in parallel if repo modules are available
- **US5 (P3)**: Depends on US1 + US4 (verification that all code changes are complete)

### Within Each Phase

- Tests MUST be written and observed to FAIL before implementation
- Foundation (db.rs, schema.rs) before repository modules
- Repository modules before test migration
- All quality gates (check, clippy, fmt, test) must pass at each checkpoint

### Parallel Opportunities

- T006, T007 (error handling + type alias) can run in parallel
- T010–T014 (model updates) can run in parallel with each other
- T016, T017 (MCP type updates) can run in parallel
- T018–T022 (repo unit tests) can run in parallel
- T031–T041 (test migration) can run in parallel with each other (T030 excluded — shares file with T004)
- T042 and T043 (retention) can run in parallel with Phase 3 test migration

---

<!-- SECTION:DESCRIPTION:END -->
