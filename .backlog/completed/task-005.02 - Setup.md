---
id: TASK-005.02
title: "005 - Setup"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5020
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Project initialization and new module scaffolding

- [x] T001 Create `src/driver/mod.rs` with `AgentDriver` trait definition, `AgentEvent` enum, and module doc comments
- [x] T002 [P] Create `src/acp/mod.rs` with module structure and doc comments
- [x] T003 [P] Add `AppError::Acp(String)` variant in `src/errors.rs` with `Display` and `From` implementations
- [x] T004 [P] Add `ProtocolMode` enum (`Mcp`, `Acp`) to `src/models/session.rs` with serde serialization
- [x] T004b [P] Enable `codec` feature on `tokio-util` in `Cargo.toml` — change `features = ["rt"]` to `features = ["rt", "codec"]` for `LinesCodec`/`FramedRead`/`FramedWrite` support

**Checkpoint**: New module stubs exist, project compiles with `cargo check`

---

<!-- SECTION:DESCRIPTION:END -->
