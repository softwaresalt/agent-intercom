---
id: TASK-004.13
title: "004 - User Story 16 — Approval File Attachment (Priority: P2)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-004
dependencies: []
ordinal: 4130
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Approval messages include original file content as Slack attachment for informed operator review

**Independent Test**: Call `check_clearance` with a diff for an existing file, verify Slack shows both diff and original file

### Tests for US16

> **Write these tests FIRST, verify they FAIL before implementation**

- [x] T081 [P] [US16] Unit test for original file attachment logic in `tests/unit/ask_approval_tests.rs` (scenarios S087-S090)
- [x] T082 [P] [US16] Unit test for graceful handling of missing/unreadable file in `tests/unit/ask_approval_tests.rs` (scenarios S091, S093)
- [x] T083 [P] [US16] Contract test for `check_clearance` response with file attachment in `tests/contract/ask_approval_contract_tests.rs` (scenarios S087-S088)

### Implementation for US16

- [x] T084 [US16] Update `src/mcp/tools/ask_approval.rs` — after computing `original_hash`, read original file content and upload as Slack file attachment alongside the diff
- [x] T085 [US16] Handle new file case: skip original file upload when file does not exist (no `original_hash`)
- [x] T086 [US16] Handle file read errors gracefully — log warning, post approval message without original attachment

**Checkpoint**: Operators see full file context alongside diffs in approval requests

---

<!-- SECTION:DESCRIPTION:END -->
