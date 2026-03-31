---
id: TASK-008.09
title: "008 - Visual Rendering Tests"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-008
dependencies: []
ordinal: 8090
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Screenshot-based verification of Block Kit rendering in real Slack (SC-009, SC-010).

**Depends on**: Phase 7 (Playwright scaffolding).

### Tasks

- [X] **8.1** Create `tests/visual/scenarios/message-rendering.spec.ts`
- Navigate to test channel
- Verify approval message rendering: emoji, diff block, buttons
- Verify prompt message rendering: text, buttons
- Verify stall alert rendering: warning emoji, duration, buttons
- Verify session started notification
- Verify code snippet blocks
- Capture screenshots for each
- Scenarios: S-T3-002, S-T3-003, S-T3-004, S-T3-009, S-T3-010
- FRs: FR-026

- [X] **8.2** Create `tests/visual/scenarios/approval-flow.spec.ts`
- Click Accept button on approval message
- Capture before/after screenshots showing button replacement
- Scenario: S-T3-008
- FRs: FR-027, FR-025

- [X] **8.3** Create `tests/visual/scenarios/button-replacement.spec.ts`
- Click various buttons (Continue, Nudge, Resume) and capture transitions
- Verify static status text replaces interactive buttons
- Scenarios: S-T3-008
- FRs: FR-027

### Constitution Gate

- [X] All visual rendering tests pass against test workspace (SC-010 verified in requirements.md)
- [X] Screenshots captured for every scenario (Phase 9 report §3.2 lists all screenshot filenames)
- [X] SC-010: visual confirmation of correct Block Kit rendering (verified in requirements.md)

---

<!-- SECTION:DESCRIPTION:END -->
