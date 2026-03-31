---
id: TASK-008.07
title: "008 - Modal Diagnostics (API Level)"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-008
dependencies: []
ordinal: 8070
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Diagnose modal-in-thread issue at the API level (FR-022, FR-023).

**Depends on**: Phase 5 (live interaction infrastructure).

### Tasks

- [X] **6.1** Create `tests/live/live_modal_tests.rs`
- Test modal open for top-level button → document API result
- Test modal open for threaded button → document API result
- Compare results: success/error, trigger_id scope, timing
- Scenarios: S-T2-006, S-T2-007
- FRs: FR-015, FR-016, FR-022

- [X] **6.2** Add thread-reply fallback end-to-end test
- Simulate modal failure → fallback activates → thread reply resolves prompt
- Scenario: S-T2-008
- FRs: FR-017, FR-023

- [X] **6.3** Test wait-resume-instruct modal in thread
- Same pattern as prompt refine: threaded vs top-level
- Scenario: S-T2-011
- FRs: FR-015

- [X] **6.4** Write diagnostic report section in `SCENARIOS.md` or standalone file
- Categorize failure mode based on API evidence
- Scenarios: S-X-001

### Constitution Gate

- [X] Modal diagnostic tests pass
- [X] API-level evidence documented for modal-in-thread behavior
- [X] Fallback coverage verified for all 3 modal paths (SC-003 API portion)

---

<!-- SECTION:DESCRIPTION:END -->
