---
id: TASK-005.12
title: "005 - User Story 9 — ACP Stall Detection and Recovery (Priority: P3)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5120
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Stall detection works for ACP via stream activity monitoring; nudges sent directly on stream

**Independent Test**: Silence agent stream, verify stall detector fires and nudge message appears on stream

### Tests (S063–S068)

- [x] T091 [P] [US9] Write unit test for ACP stream activity resetting stall timer in `tests/unit/stall_detector_tests.rs` — covers S063
- [x] T092 [P] [US9] Write unit test for ACP nudge delivery via stream in `tests/unit/stall_detector_tests.rs` — covers S064
- [x] T093 [P] [US9] Write unit test for nudge retry exhaustion and operator notification in `tests/unit/stall_detector_tests.rs` — covers S066
- [x] T094 [P] [US9] Write unit test for crash with pending clearance in `tests/unit/acp_session_tests.rs` — covers S068

### Implementation

- [x] T095 [US9] Add `StreamActivity` variant to stall detector activity source in `src/orchestrator/stall_detector.rs`
- [x] T096 [US9] Update ACP reader task to bump `last_stream_activity` timestamp on every successful parse in `src/acp/reader.rs`
- [x] T097 [US9] Wire stall detector to call `driver.send_prompt(session_id, nudge)` for ACP sessions instead of MCP notification
- [x] T098 [US9] Implement session restart from Slack — kill old process, spawn new with original prompt, same thread_ts (S067)
- [x] T099 [US9] Handle pending clearance resolution on crash — resolve as timeout, notify operator (S068)

**Checkpoint**: ACP stall detection and recovery fully functional

---

<!-- SECTION:DESCRIPTION:END -->
