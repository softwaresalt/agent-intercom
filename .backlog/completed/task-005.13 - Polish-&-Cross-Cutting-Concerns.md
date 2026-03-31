---
id: TASK-005.13
title: "005 - Polish & Cross-Cutting Concerns"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5130
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Improvements that affect multiple user stories

- [x] T100 [P] Update `config.toml.example` with all new configuration sections (`[[workspace]]`, `[acp]`)
- [x] T101 [P] Update `docs/configuration.md` (if exists) with workspace mapping and ACP mode documentation
- [x] T102 [P] Add migration guide for `channel_id` → `workspace_id` query parameter transition
- [x] T103 Run full regression: `cargo test` — all existing + new tests pass
- [x] T104 Run `cargo clippy -- -D warnings` — zero warnings
- [x] T105 Run `cargo fmt --all -- --check` — formatting clean
- [x] T106 Validate quickstart.md against actual implementation

---

<!-- SECTION:DESCRIPTION:END -->
