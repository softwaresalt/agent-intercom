---
id: TASK-007.10
title: "007 - Dependencies & Execution Order"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-007
dependencies: []
ordinal: 7100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

### Phase Dependencies

- **Phase 1 (Setup)**: Empty — no setup needed
- **Phase 2 (Foundational)**: Empty — no shared prerequisites
- **Phase 3 (US1 / F-06)**: Independent — can start immediately
- **Phase 4 (US2 / F-07)**: Independent — can start immediately
- **Phase 5 (US5 / F-10 + F-13)**: Independent — can start immediately
- **Phase 6 (US4 / F-15 + conditional)**: Independent — can start immediately; has internal gate at T038
- **Phase 7 (Polish)**: Depends on Phases 3–6 completion

### User Story Dependencies

- **US1 (P1)**: No dependencies — fully self-contained in `src/acp/reader.rs`
- **US2 (P1)**: No dependencies — self-contained in `session_repo.rs` + `commands.rs`
- **US5 (P2)**: No dependencies — F-10 and F-13 are in separate modules; can be done together or sequentially
- **US4 (P2)**: Internal dependency: T037→T038 gate determines if T039–T051 execute

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Implementation code makes tests pass
- Full cargo test verification after each story

### Parallel Opportunities

- **All four user stories can run in parallel** — they touch completely separate files
- Within US5: F-10 tests (T018–T022) and F-13 tests (T023–T026) are parallelizable
- Within US4: All conditional tests (T039–T044) are parallelizable

---

<!-- SECTION:DESCRIPTION:END -->
