---
id: TASK-006.12
title: "006 - Notes"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-006
dependencies: []
ordinal: 6120
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

- **[P]** tasks target different files with no dependencies on incomplete tasks
- **[Story]** label maps each task to its user story for traceability
- Each user story is independently completable and testable at its checkpoint
- **TDD required**: Verify tests FAIL before implementing (Constitution principle III)
- **Error handling**: Warn-and-continue for all handler errors (Design Decision D3)
- **No new dependencies**: Uses existing rmcp, sqlx, slack-morphism, sha2, tokio crates
- **Quality gates per phase**: `cargo check` + `cargo clippy -- -D warnings` + `cargo fmt --all -- --check` + `cargo test`
- **Design decisions**: D1 (shared blocks in slack/blocks.rs), D2 (direct post for clearance, enqueue for prompt), D3 (log warn + continue), D4 (AcpDriver-only registration, no oneshot channels)

<!-- SECTION:DESCRIPTION:END -->
