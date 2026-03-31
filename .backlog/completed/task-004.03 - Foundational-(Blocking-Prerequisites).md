---
id: TASK-004.03
title: "004 - Foundational (Blocking Prerequisites)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-004
dependencies: []
ordinal: 4030
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Repository layer, compiled policy, AppState wiring — MUST complete before user stories

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T007 [P] Create `src/persistence/steering_repo.rs` — insert, fetch_unconsumed, mark_consumed, purge
- [X] T008 [P] Create `src/persistence/inbox_repo.rs` — insert, fetch_unconsumed_by_channel, mark_consumed, purge
- [X] T009 [P] Add `CompiledWorkspacePolicy` struct to `src/models/policy.rs` (wraps `WorkspacePolicy` + `RegexSet`)
- [X] T010 Update `src/policy/loader.rs` — `load()` returns `CompiledWorkspacePolicy` with pre-compiled `RegexSet`
- [X] T011 Add `slack_detail_level` field to `GlobalConfig` in `src/config.rs` (minimal/standard/verbose, default standard)
- [X] T012 Wire `PolicyCache` and `AuditLogger` into `AppState` in `src/mcp/handler.rs`
- [X] T013 Register new repos in `src/persistence/mod.rs`

**Checkpoint**: Foundation ready — user story implementation can begin

---

<!-- SECTION:DESCRIPTION:END -->
