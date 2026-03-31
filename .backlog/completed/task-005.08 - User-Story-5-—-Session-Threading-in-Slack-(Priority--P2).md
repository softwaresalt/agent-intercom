---
id: TASK-005.08
title: "005 - User Story 5 — Session Threading in Slack (Priority: P2)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5080
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Each session owns a dedicated Slack thread; all messages posted as threaded replies

**Independent Test**: Start two sessions, verify each gets separate Slack thread with no cross-contamination

### Tests (S036–S042)

- [x] T052 [P] [US5] Write unit test for thread_ts recording on first Slack message in `tests/unit/session_routing_tests.rs` — covers S036
- [x] T053 [P] [US5] Write unit test for subsequent messages using thread_ts in `tests/unit/session_routing_tests.rs` — covers S037, S038
- [x] T054 [P] [US5] Write integration test for two concurrent sessions with separate threads in `tests/integration/thread_routing_tests.rs` — covers S041
- [x] T055 [P] [US5] Write boundary test for thread_ts immutability in `tests/unit/session_routing_tests.rs` — covers S042

### Implementation

- [x] T056 [US5] Add `thread_ts: Option<&str>` parameter to `SlackService::post_message` and thread-aware posting methods in `src/slack/client.rs`
- [x] T057 [US5] Create session thread root message builder in `src/slack/blocks.rs` — formats the initial "Session started" message
- [x] T058 [US5] Wire thread_ts recording — after first Slack message posted, call `session_repo.set_thread_ts(session_id, ts)` in Slack posting code
- [x] T059 [US5] Update all session-scoped Slack message sends (status, clearance, broadcast, stall) to include `thread_ts` in `src/slack/client.rs` and callers
- [x] T060 [US5] Post final "Session ended" summary as thread reply on session termination

**Checkpoint**: Each session has its own Slack thread; all messages are properly threaded

---

<!-- SECTION:DESCRIPTION:END -->
