---
id: TASK-003.13
title: "003 - Parallel Example: Phase 2 (Foundational)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-003
dependencies: []
ordinal: 3130
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

```text
# Sequential: Cargo.toml must be updated first
T003: Update Cargo.toml package name and binary names

# Then crate roots (can be parallel):
T004: Update src/lib.rs            ──┐
T005: Update src/main.rs             ├── Parallel
T006: Update ctl/main.rs           ──┘

# Then constants (all parallel):
T007: KEYCHAIN_SERVICE             ──┐
T008: IPC pipe name                  │
T009: Env var prefix                 ├── Parallel
T010: Policy directory               │
T011: Slack command root           ──┘

# Then global imports (parallel per directory):
T012: src/ imports                 ──┐
T013: tests/ imports                 ├── Parallel
T014: ctl/ imports                 ──┘

# Then sweep + gate:
T015: Doc comments sweep
T016: config.toml update
T017: cargo check gate
```

---

<!-- SECTION:DESCRIPTION:END -->
