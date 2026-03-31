---
id: TASK-007.08
title: "007 - Polish & Cross-Cutting Concerns"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-007
dependencies: []
ordinal: 7080
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Final validation and documentation updates.

- [x] T052 [P] Run full quality gate: `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`
- [x] T053 [P] Run format check: `cargo fmt --all -- --check`
- [x] T054 Run full test suite and verify 959+ baseline tests still pass plus new tests: `cargo test --all-targets`
- [x] T055 Update `specs/007-acp-correctness-mobile/spec.md` — update FR-007 description to reflect `channel_id` removal instead of deprecation warning
- [x] T056 [P] Update `.context/backlog.md` — mark F-06, F-07, F-10, F-13 as complete

---

<!-- SECTION:DESCRIPTION:END -->
