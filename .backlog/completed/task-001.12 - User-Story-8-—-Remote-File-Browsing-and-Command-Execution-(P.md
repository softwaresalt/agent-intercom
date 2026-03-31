---
id: TASK-001.12
title: "001 - User Story 8 — Remote File Browsing and Command Execution (Priority: P3)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001
dependencies: []
ordinal: 1120
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Operator browses workspace files and runs pre-approved commands from Slack

**Independent Test**: Issue `list-files` via Slack, verify directory tree; issue `show-file`, verify file contents; run a registered command

### Tests (Constitution Principle III)

- [x] T121 Write unit tests for command execution safety in `tests/unit/command_exec_tests.rs`: allowed command passes, disallowed command rejected (FR-014), path validation for list-files/show-file stays within workspace root

### Implementation for User Story 8

- [x] T076 [US8] Implement `list-files` command handler in `src/slack/commands.rs`: accept optional path and `--depth N` flag; list directory contents from session's workspace_root; validate path stays within workspace root (FR-006); format as tree and post to Slack
- [x] T077 [US8] Implement `show-file` command handler in `src/slack/commands.rs`: accept path and optional `--lines START:END` range; validate path within workspace root; read file contents; upload to Slack as snippet with syntax highlighting based on file extension
- [x] T078 [US8] Implement custom command execution handler in `src/slack/commands.rs`: accept command alias from Slack; look up in `config.commands` registry (FR-014); if not found return "command not found" error; if found, execute via `tokio::process::Command` with working directory set to session's workspace_root; capture stdout/stderr; post output to Slack; auto-pause stall timer during execution (FR-025)
- [x] T079 [US8] Add tracing spans to file browsing and command execution

**Checkpoint**: Remote file browsing and command execution functional

---

<!-- SECTION:DESCRIPTION:END -->
