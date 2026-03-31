---
id: TASK-005.06
title: "005 - User Story 3 — ACP Session Lifecycle via Slack (Priority: P1)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5060
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Start, monitor, and terminate ACP agent sessions from Slack commands

**Independent Test**: `/intercom session-start` in Slack spawns agent, status updates appear in Slack thread

### Tests (S018–S026)

- [x] T032 [P] [US3] Write integration test for ACP session start (spawn + initial prompt) in `tests/integration/acp_lifecycle_tests.rs` — covers S018
- [x] T033 [P] [US3] Write unit test for ACP session stop (process kill + status update) in `tests/unit/acp_session_tests.rs` — covers S021
- [x] T034 [P] [US3] Write unit test for agent process crash handling in `tests/unit/acp_session_tests.rs` — covers S023
- [x] T035 [P] [US3] Write unit test for startup timeout when agent never responds in `tests/unit/acp_session_tests.rs` — covers S025
- [x] T036 [P] [US3] Write boundary test for empty prompt rejection in `tests/unit/acp_session_tests.rs` — covers S026

### Implementation

- [x] T037 [US3] Create ACP spawner in `src/acp/spawner.rs` — spawn `host_cli` process with `kill_on_drop(true)`, `env_clear()` + safe variable allowlist (PATH, HOME, RUST_LOG), capture stdin/stdout handles, return `AcpConnection`
- [x] T037b [P] [US3] Write unit test verifying spawned process does NOT inherit SLACK_BOT_TOKEN or SLACK_APP_TOKEN in `tests/unit/acp_session_tests.rs` — covers S075
- [x] T038 [US3] Wire ACP session-start in `src/slack/commands.rs` — when mode is ACP, spawn agent via ACP spawner, pass `channel_id` from originating Slack channel context (FR-027), register session with AcpDriver
- [x] T039 [US3] Implement process exit monitoring in `src/acp/spawner.rs` — `tokio::spawn` task that awaits `child.wait()` and emits `AgentEvent::SessionTerminated`
- [x] T040 [US3] Implement startup timeout — if no message from agent within `startup_timeout_seconds`, kill process and emit failure event
- [x] T041 [US3] Handle max concurrent sessions check in ACP session-start path (S024)

**Checkpoint**: ACP sessions start/stop from Slack; process crashes are detected and reported

---

<!-- SECTION:DESCRIPTION:END -->
