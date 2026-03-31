---
id: TASK-005.18
title: "005 - Dependencies & Execution Order"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5180
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user stories
- **Phase 3 (US1 — Dual-Mode Startup)**: Depends on Phase 2
- **Phase 4 (US2 — Agent Driver)**: Depends on Phase 2
- **Phase 5 (US3 — ACP Lifecycle)**: Depends on Phase 3 (mode flag) and Phase 4 (driver trait)
- **Phase 6 (US4 — Workspace Mapping)**: Depends on Phase 2 only — can parallel with Phase 3/4
- **Phase 7 (US5 — Session Threading)**: Depends on Phase 2 only — can parallel with Phase 3/4
- **Phase 8 (US6 — Channel Routing)**: Depends on Phase 7 (thread_ts) and Phase 2 (channel_id column)
- **Phase 9 (US7 — ACP Stream)**: Depends on Phase 4 (driver trait) and Phase 5 (spawner)
- **Phase 10 (US8 — Offline Queue)**: Depends on Phase 9 (stream) and feature 004 (steering queue)
- **Phase 11 (US9 — Stall Detection)**: Depends on Phase 9 (stream activity) and Phase 4 (driver)
- **Phase 12 (Polish)**: Depends on all desired user stories being complete
- **Phase 13 (Critical Fixes)**: Depends on Phase 12 (all core features complete). HITL-003 depends on Phase 3 (MCP transport) + Phase 9 (ACP stream). HITL-005/006 depend on Phase 8 (channel routing).
- **Phase 14 (Security)**: Depends on Phase 5 (spawner) for ES-004, Phase 3 (config) for ES-010, Phase 9 (writer) for ES-008. Can parallel with Phase 13.
- **Phase 15 (Reliability)**: Depends on Phase 11 (stall detector) for ES-006, Phase 9 (reader) for ES-005/ES-007. Can parallel with Phase 13/14.
- **Phase 16 (Usability)**: Depends on Phase 8 (commands) for all items. Can parallel with Phase 14/15.

### User Story Dependencies

```
Phase 2 (Foundation)
  ├── Phase 3 (US1: Mode Flag) ─────────┐
  ├── Phase 4 (US2: Driver Trait) ──────┤
  │   ├── Phase 5 (US3: ACP Lifecycle) ──┤── Phase 9 (US7: Stream) ── Phase 10 (US8: Offline)
  │   └── Phase 11 (US9: Stall)  ────────┘                           Phase 11 (US9: Stall)
  ├── Phase 6 (US4: Workspace Mapping) [parallel with 3/4]
  └── Phase 7 (US5: Threading) ── Phase 8 (US6: Channel Routing)

Phase 12 (Polish) ← depends on all above
  ├── Phase 13 (Critical Fixes: HITL-003, HITL-005, HITL-006)
  ├── Phase 14 (Security: ES-004, ES-010, ES-008) [parallel with 13]
  ├── Phase 15 (Reliability: HITL-001, HITL-007, ES-005/006/007/009) [parallel with 13/14]
  └── Phase 16 (Usability: HITL-002, HITL-004, HITL-008) [parallel with 14/15]
```

### Parallel Opportunities

- **Phase 3 + Phase 6 + Phase 7**: Mode flag, workspace mapping, and threading can all run in parallel after Phase 2
- **Phase 13 + Phase 14**: Critical fixes and security hardening can run in parallel
- **Phase 14 + Phase 15 + Phase 16**: Security, reliability, and usability can all run in parallel
- **Within each phase**: All tasks marked [P] can run in parallel
- **All test tasks marked [P]** within a phase can run in parallel

---

<!-- SECTION:DESCRIPTION:END -->
