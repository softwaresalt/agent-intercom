---
id: TASK-002.02
title: "002 - Setup"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-002
dependencies: []
ordinal: 2020
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Swap dependencies and update configuration

- [X] T001 Replace `surrealdb` with `sqlx` in Cargo.toml workspace dependencies: remove `surrealdb = "1.5"`, add `sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "json", "chrono", "macros"] }`
- [X] T002 Update `[database]` section in config.toml: replace SurrealDB engine/namespace/database fields with `path = "data/agent-rc.db"`
- [X] T003 Update `DatabaseConfig` struct and `db_path()` method in src/config.rs to parse the new `path` field instead of SurrealDB-specific fields

---

<!-- SECTION:DESCRIPTION:END -->
