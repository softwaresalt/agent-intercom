---
id: TASK-005.03
title: "005 - Foundational (Blocking Prerequisites)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5030
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Tests

- [x] T005 [P] Write unit test for `ProtocolMode` serde round-trip in `tests/unit/session_model_tests.rs`
- [x] T006 [P] Write unit test for `AppError::Acp` display format in `tests/unit/error_tests.rs`
- [x] T007 [P] Write unit test for `AgentEvent` enum construction and field access in `tests/unit/driver_trait_tests.rs`

### Implementation

- [x] T008 Add `protocol_mode`, `channel_id`, `thread_ts`, `connectivity_status`, `last_activity_at`, `restart_of` fields to `Session` struct in `src/models/session.rs`
- [x] T009 Write idempotent schema migration in `src/persistence/schema.rs` — add `protocol_mode`, `channel_id`, `thread_ts`, `connectivity_status`, `last_activity_at`, `restart_of` columns to `session` table using PRAGMA table_info check
- [x] T010 Update `SessionRepo` in `src/persistence/session_repo.rs` — include new fields in INSERT/SELECT/UPDATE queries
- [x] T011 [P] Add `find_active_by_channel(channel_id)` query to `src/persistence/session_repo.rs`
- [x] T012 [P] Add `find_by_channel_and_thread(channel_id, thread_ts)` query to `src/persistence/session_repo.rs`
- [x] T013 [P] Add `set_thread_ts(session_id, thread_ts)` update method to `src/persistence/session_repo.rs`
- [x] T014 Create new index `idx_session_channel` and `idx_session_channel_thread` in `src/persistence/schema.rs`

**Checkpoint**: Foundation ready — Session model has new fields, schema migrates cleanly, repo queries work

---

<!-- SECTION:DESCRIPTION:END -->
