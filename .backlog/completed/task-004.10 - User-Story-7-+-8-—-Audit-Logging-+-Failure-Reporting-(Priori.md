---
id: TASK-004.10
title: "004 - User Story 7 + 8 — Audit Logging + Failure Reporting (Priority: P3)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-004
dependencies: []
ordinal: 4100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: All interactions audited in JSONL; agent failures reported to Slack

**Independent Test**: Run session with tool calls, inspect audit log; simulate stall, verify Slack notification

### Tests for US7/US8

- [x] T054 [P] [US7] Unit test for `JsonlAuditWriter` in `tests/unit/audit_writer_tests.rs` (scenarios S049-S057)
- [x] T055 [P] [US7] Unit test for daily rotation in `tests/unit/audit_writer_tests.rs` (scenario S054)
- [x] T056 [P] [US8] Unit test for stall notification in `tests/unit/stall_detector_tests.rs` (scenarios S058-S061)

### Implementation for US7/US8

- [x] T057 [US7] Wire `AuditLogger` into tool call handlers — log after each tool call in `src/mcp/tools/mod.rs` or individual handlers
- [x] T058 [US7] Wire `AuditLogger` into approval/rejection flow in `src/slack/handlers/approval.rs`
- [x] T059 [US7] Wire `AuditLogger` into session lifecycle in `src/orchestrator/session_manager.rs`
- [x] T060 [US8] Update `src/orchestrator/stall_detector.rs` — send Slack notification with session details and recovery steps on stall
- [x] T061 [US8] Update stall notification to include actionable recovery suggestions

**Checkpoint**: Full audit trail; failures proactively reported

---

<!-- SECTION:DESCRIPTION:END -->
