---
id: TASK-004.02
title: "004 - Setup"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-004
dependencies: []
ordinal: 4020
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Project initialization — new modules and schema

- [X] T001 Add `steering_message` and `task_inbox` DDL to `src/persistence/schema.rs`
- [X] T002 [P] Create `src/models/steering.rs` with `SteeringMessage` struct
- [X] T003 [P] Create `src/models/inbox.rs` with `TaskInboxItem` struct
- [X] T004 [P] Create `src/audit/mod.rs` with `AuditLogger` trait and `AuditEntry` struct
- [X] T005 [P] Create `src/audit/writer.rs` with `JsonlAuditWriter` (daily rotation)
- [X] T006 Register new modules in `src/models/mod.rs` and create `src/audit/` module

---

<!-- SECTION:DESCRIPTION:END -->
