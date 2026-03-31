---
id: TASK-005.17
title: "005 - Usability Improvements (Findings Remediation) 🎨"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5170
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Session history, session titles, help text fixes, paused session visibility

**Findings**: HITL-002 (LOW), HITL-004 (LOW), HITL-008 (LOW)

### HITL-002 — Session History & Titles (FR-048, FR-049)

#### Tests (S113–S116)

- [X] T155 [P] Write unit test for `/arc sessions --all` query returning all statuses in `tests/unit/command_tests.rs` — covers S113, S114
- [X] T156 [P] Write unit test for session title truncation in `tests/unit/session_model_tests.rs` — covers S115, S116

#### Implementation

- [X] T157 Add `title` column to session table schema in `src/persistence/schema.rs` — `TEXT DEFAULT NULL`, idempotent migration
- [X] T158 Update `SessionRepo` in `src/persistence/session_repo.rs` — include `title` in INSERT/SELECT, add `list_all_by_channel` query returning all statuses
- [X] T159 Update ACP session-start handler to set `title` = truncated initial prompt (max 80 chars, append "..." if truncated) in `src/slack/commands.rs`
- [X] T160 Update `handle_sessions` in `src/slack/commands.rs` — parse `--all` flag, call `list_all_by_channel` or `list_active`, format output with status icons and titles

### HITL-004 — session-checkpoint Help Text (FR-050)

#### Tests (S117–S118)

- [X] T161 [P] Write unit test for session-checkpoint help text accuracy in `tests/unit/command_tests.rs` — covers S117, S118

#### Implementation

- [X] T162 Update session-checkpoint help text in `src/slack/commands.rs` — change `[session_id]` to show correct optionality, update error messages to clearly state "no active session in this channel" when resolution fails

### HITL-008 — Paused Sessions in Listing (FR-051)

#### Tests (S119–S120)

- [X] T163 [P] Write unit test for paused session visibility in `/arc sessions` in `tests/unit/command_tests.rs` — covers S119, S120

#### Implementation

- [X] T164 Update `list_active` query in `src/persistence/session_repo.rs` to include Paused sessions (or add `list_visible` query returning Active + Paused)
- [X] T165 Update session listing format in `src/slack/commands.rs` — add ⏸ icon for Paused, 🟢 for Active

**Checkpoint**: Session history queryable; titles visible; help text accurate; paused sessions visible

---

<!-- SECTION:DESCRIPTION:END -->
