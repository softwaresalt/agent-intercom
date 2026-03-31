---
id: TASK-005.11
title: "005 - User Story 8 — Offline Agent Message Queuing (Priority: P3)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5110
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Queue operator messages for offline/disconnected agents, deliver on reconnect

**Independent Test**: Disconnect agent, send messages from Slack, reconnect, verify delivery

### Tests (S059–S062)

- [x] T085 [P] [US8] Write unit test for steering queue when agent offline in `tests/unit/offline_queue_tests.rs` — covers S059
- [x] T086 [P] [US8] Write integration test for queued message delivery on reconnect in `tests/integration/acp_lifecycle_tests.rs` — covers S060, S062

### Implementation

- [x] T087 [US8] Add agent connectivity status tracking to session model (online/offline/stalled) in `src/models/session.rs`
- [x] T088 [US8] Update steering handler in `src/slack/handlers/steer.rs` to check connectivity status and post "Agent offline — message queued" notification
- [x] T089 [US8] Implement message flush on ACP reconnect — on stream activity resume, read all unconsumed steering messages and deliver via `driver.send_prompt`
- [x] T090 [US8] Post "Agent back online — delivering N queued messages" notification to Slack thread

**Checkpoint**: Offline queuing works transparently; operator sees clear status indicators

---

<!-- SECTION:DESCRIPTION:END -->
