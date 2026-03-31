---
id: TASK-004.11
title: "004 - User Story 10 + 11 — Detail Levels + Auto-Approve Suggestion (Priority: P3)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-004
dependencies: []
ordinal: 4110
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Configurable Slack message verbosity; auto-approve suggestions after manual approval

**Independent Test**: Set detail level to minimal, verify terse messages; approve command, verify suggestion

### Tests for US10/US11

- [x] T062 [P] [US10] Unit test for detail level message filtering in `tests/unit/blocks_tests.rs` (scenarios S062-S067)
- [x] T063 [P] [US11] Unit test for auto-approve suggestion generation in `tests/unit/command_approve_tests.rs` (scenarios S068-S073)

### Implementation for US10/US11

- [x] T064 [US10] Update `src/slack/blocks.rs` — message builders check detail level; approvals/errors always full
- [x] T065 [US10] Pass `slack_detail_level` from config through `SlackService` to message builders
- [x] T066 [US11] Create `src/slack/handlers/command_approve.rs` — auto-approve suggestion flow after manual approval
- [x] T067 [US11] Add "Add to auto-approve?" button in `src/slack/blocks.rs`
- [x] T068 [US11] Implement regex pattern generation and write to `.intercom/settings.json`

**Checkpoint**: Slack messages respect detail level; commands can self-learn auto-approve patterns

---

<!-- SECTION:DESCRIPTION:END -->
