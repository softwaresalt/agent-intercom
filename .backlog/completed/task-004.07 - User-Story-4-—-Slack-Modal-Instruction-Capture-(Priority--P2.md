---
id: TASK-004.07
title: "004 - User Story 4 — Slack Modal Instruction Capture (Priority: P2)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-004
dependencies: []
ordinal: 4070
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: `standby` and `transmit` deliver real operator-typed instructions instead of placeholder strings

**Independent Test**: Press "Resume with Instructions", type text, submit; agent receives exact text

### Tests for US4

- [x] T036 [P] [US4] Unit test for modal view builder in `tests/unit/blocks_tests.rs` (scenario S029)
- [x] T037 [P] [US4] Contract test for `standby` with real instruction text in `tests/contract/wait_contract_tests.rs` (scenario S030)
- [x] T038 [P] [US4] Contract test for `transmit` refine with real text in `tests/contract/prompt_contract_tests.rs` (scenario S032)

### Implementation for US4

- [x] T039 [US4] Add modal view builder (text input block) in `src/slack/blocks.rs`
- [x] T040 [US4] Update `src/slack/handlers/wait.rs` — extract `trigger_id`, call `views.open`, store session in `private_metadata`
- [x] T041 [US4] Update `src/slack/handlers/prompt.rs` — same `trigger_id` → modal flow for "Refine"
- [x] T042 [US4] Add `ViewSubmission` match arm in `src/slack/events.rs` — extract text, resolve oneshot
- [x] T043 [US4] Thread `trigger_id` from `BlockActions` payload into handler functions

**Checkpoint**: No more placeholder strings — real operator instructions flow through

---

<!-- SECTION:DESCRIPTION:END -->
