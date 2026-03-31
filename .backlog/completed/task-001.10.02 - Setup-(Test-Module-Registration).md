---
id: TASK-001.10.02
title: "001-002 - Setup (Test Module Registration)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001.10
dependencies: []
ordinal: 1120
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Register new integration test modules in the test harness entry point

- [X] T001 Register `policy_watcher_tests` module in `tests/integration.rs`
- [X] T002 Register `ipc_server_tests` module in `tests/integration.rs`
- [X] T003 Register `mcp_dispatch_tests` module in `tests/integration.rs`

**Checkpoint**: `cargo check --test integration` compiles with empty new modules

---

<!-- SECTION:DESCRIPTION:END -->
