---
id: TASK-005.16
title: "005 - Reliability & Observability (Findings Remediation) 📊"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5160
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: WebSocket notifications, audit logging, rate limiting, stall timer persistence, startup ordering

**Findings**: HITL-001 (MEDIUM), HITL-007 (MEDIUM), ES-005 (MEDIUM), ES-006 (MEDIUM), ES-007 (LOW), ES-009 (LOW)

### HITL-001 — Socket Mode Disconnect Notifications (FR-042)

#### Tests (S098–S100)

- [ ] T135 [P] Write unit test for WebSocket drop notification posting in `tests/unit/slack_client_tests.rs` — covers S098, S099
- [ ] T136 [P] Write unit test for no notification when no active sessions in `tests/unit/slack_client_tests.rs` — covers S100

#### Implementation

- [ ] T137 Add `on_disconnect` and `on_reconnect` callback hooks in `src/slack/client.rs` — query active session channels, post notification via HTTP REST API (not Socket Mode)
- [ ] T138 Wire disconnect/reconnect hooks into Socket Mode event loop — detect connection state changes, invoke hooks

### HITL-007 — ACP Audit Logging (FR-043)

#### Tests (S101–S103)

- [ ] T139 [P] Write unit test for ACP session lifecycle audit entries in `tests/unit/audit_tests.rs` — covers S101, S102, S103

#### Implementation

- [ ] T140 Add ACP audit event types to `src/audit/writer.rs` — `acp_session_start`, `acp_session_stop`, `acp_session_pause`, `acp_session_resume`, `acp_steer_delivered`, `acp_task_queued`
- [ ] T141 Add audit log writes in ACP session handlers in `src/slack/commands.rs` — call `audit_logger.log()` in session-start, session-stop, session-pause, session-resume handlers
- [ ] T142 Add audit log writes in steering/task handlers — call `audit_logger.log()` in `src/slack/handlers/steer.rs` and task handler when mode is ACP

### ES-005 — ACP Stream Rate Limiting (FR-044)

#### Tests (S104–S106)

- [ ] T143 [P] Write unit test for token-bucket rate limiter in `tests/unit/acp_codec_tests.rs` — covers S104, S105, S106

#### Implementation

- [ ] T144 Create `TokenBucketRateLimiter` struct in `src/acp/reader.rs` — configurable rate (default 10/sec), burst allowance, sustained violation detection
- [ ] T145 Wire rate limiter into ACP reader loop in `src/acp/reader.rs` — check each message, log WARN on burst, terminate session on sustained flood
- [ ] T146 Add `max_msg_rate` config field to `[acp]` section in `src/config.rs` with default value 10

### ES-006 — Stall Timer Initialization on Restart (FR-045)

#### Tests (S107–S108)

- [ ] T147 [P] Write unit test for stall timer initialization from DB timestamps in `tests/unit/stall_detector_tests.rs` — covers S107, S108

#### Implementation

- [ ] T148 Add `load_active_session_timestamps` query to `src/persistence/session_repo.rs` — return `Vec<(session_id, last_activity_at)>` for active/interrupted sessions
- [ ] T149 Update stall detector initialization in `src/orchestrator/stall_detector.rs` — on startup, call `load_active_session_timestamps`, initialize each timer with `now - last_activity_at` elapsed

### ES-007 — Startup Race Condition (FR-046)

#### Tests (S109–S110)

- [ ] T150 [P] Write unit test verifying session DB commit happens before reader start in `tests/unit/acp_session_tests.rs` — covers S109, S110

#### Implementation

- [ ] T151 Reorder ACP session start sequence in `src/slack/commands.rs` and `src/acp/spawner.rs` — commit session to DB → register in driver map → THEN start reader task
- [ ] T152 Add grace period buffer in ACP reader task — if `AgentEvent` dispatched for unknown session, retry lookup once after 100ms delay before logging error

### ES-009 — Workspace Mapping Hot-Reload Race (FR-047)

#### Tests (S111–S112)

- [ ] T153 [P] Write concurrent test for config reload during session creation in `tests/integration/workspace_routing_tests.rs` — covers S111, S112

#### Implementation

- [ ] T154 Update ACP session creation in `src/slack/commands.rs` — acquire read lock on `workspace_mappings` before channel resolution and hold through session record creation

**Checkpoint**: WebSocket notifications working; audit logging complete; rate limiting enforced; stall timers persistent; startup race eliminated; config reload race-safe

---

<!-- SECTION:DESCRIPTION:END -->
