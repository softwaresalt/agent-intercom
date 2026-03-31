---
id: TASK-007.09
title: "007 - Technical Debt — Fallback Hardening (Post-Review Deferred Items)"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-007
dependencies: []
ordinal: 7090
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Address the five MEDIUM/LOW findings deferred during the PR adversarial review.
These are all contained within the thread-reply fallback mechanism (F-16/F-17) and the
`deliver_queued_messages` pipeline (F-06). No new features — correctness and maintainability only.

### TQ-008 — Extract shared fallback helper

- [x] T057 [US4] Extract duplicated fallback logic from `prompt.rs`, `wait.rs`, and `approval.rs` into a shared `spawn_thread_reply_fallback` function in `src/slack/handlers/thread_reply.rs`. The helper should accept the resolution callback as a boxed async closure or via an enum discriminant. Reduces triplication and ensures timeout/zombie-guard/cleanup logic stays in sync across handlers.

### TQ-009 — Push_event integration tests for negative paths

- [x] T058 [US4] Add `tests/integration/push_events_thread_reply_tests.rs` (or extend existing) covering:
  - Unauthorized user reply is silently ignored (entry stays registered)
  - Timeout expiry: spawned task exits after `FALLBACK_REPLY_TIMEOUT` without panicking
  - Duplicate registration for same composite key: second `register_thread_reply_fallback` call on same key logs a warning and returns without overwriting, OR documents overwrite behavior explicitly

### LC-05 — StreamActivity emitted for failed deliveries

- [x] T059 [US1] In `src/acp/reader.rs`, have `deliver_queued_messages` return a count of successfully delivered messages. Change the `StreamActivity` emission loop (line ~507) to emit only for the count of successfully delivered messages, not for all queued messages. Add/update unit test in `tests/unit/acp_reader_steering_delivery.rs` to verify `StreamActivity` is NOT emitted for failed deliveries.

### LC-04 — Silent overwrite on duplicate fallback registration

- [x] T060 [US4] In `src/slack/handlers/thread_reply.rs`, change `register_thread_reply_fallback` to check for an existing entry before inserting. If a key already exists, log a `warn!` with the channel and thread_ts, drop the new sender (sends `RecvError` to the new `rx`), and return without overwriting. Add a unit test in `tests/unit/thread_reply_fallback.rs` verifying the original entry survives a duplicate registration attempt.

### CS-06 — Hardcoded SQL status strings

- [x] T061 [P] In `src/persistence/session_repo.rs`, replace the inline string literals `'active'`, `'created'`, `'paused'` in `count_active_acp` with `SessionStatus` enum `.as_str()` calls (or equivalent constant references) to match the pattern used elsewhere in the repo. Verify no other queries in `session_repo.rs` use raw string literals for status values; update those too.

**Checkpoint**: All five deferred items resolved, 1,032+ tests passing, clippy clean.

---

<!-- SECTION:DESCRIPTION:END -->
