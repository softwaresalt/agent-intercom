---
id: TASK-005.20
title: "005 - Notes"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5200
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- TDD required: write tests first, verify they fail, then implement
- Commit after each task or logical group
- Total: 171 tasks across 16 phases (T001–T106 original, T107–T165 remediation)
- **Deferred**: `ctl/main.rs` ACP subcommands and `src/ipc/server.rs` ACP extensions are deferred to a future feature. ACP sessions are managed exclusively via Slack in this feature.
- **Findings traceability**: Each remediation task traces to a finding ID (ES-* or HITL-*) → FR → scenario → task chain

<!-- SECTION:DESCRIPTION:END -->
