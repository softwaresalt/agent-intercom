---
id: TASK-006.09
title: "006 - Dependencies & Execution Order"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-006
dependencies: []
ordinal: 6090
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — **BLOCKS all user stories**
- **US1 (Phase 3)**: Depends on Phase 2 completion
- **US2 (Phase 4)**: Depends on Phase 3 completion — MUST follow Phase 3 (both modify `run_acp_event_consumer` in src/main.rs, parallel execution would cause merge conflicts)
- **US3 (Phase 5)**: Depends on Phase 3 + Phase 4 (adds thread management to both handlers)
- **Polish (Phase 6)**: Depends on all user stories complete

### User Story Dependencies

- **US1 (P1)**: Can start after Phase 2 — no dependencies on other stories
- **US2 (P1)**: Can start after Phase 2 — logically independent of US1 but modifies same file (src/main.rs); sequential after US1 avoids merge conflicts
- **US3 (P2)**: Depends on US1 + US2 completion — modifies both handler implementations to add thread management

### Within Each User Story

1. Tests MUST be written and verified to FAIL before implementation (TDD red phase)
2. Implementation satisfies failing tests (TDD green phase)
3. Quality gates verify no regressions across all tiers
4. Story checkpoint — independently testable increment

### Parallel Opportunities

| Phase | Parallel Tasks | Reason |
|-------|---------------|--------|
| Phase 2 | T005 ‖ T006 | Different MCP tool files (ask_approval.rs vs forward_prompt.rs) |
| Phase 3 | T008 ‖ T009 | Different test tiers (unit/ vs contract/) |
| Phase 4 | T012 ‖ T013 | Different test tiers (unit/ vs contract/) |
| Phase 6 | T023 → T024 | T024 depends on T023 completion (no commit before validation) |

---

<!-- SECTION:DESCRIPTION:END -->
