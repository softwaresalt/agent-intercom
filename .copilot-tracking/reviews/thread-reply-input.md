<!-- markdownlint-disable-file -->
# Implementation Review: @-Mention Thread-Reply Input (F-16/F-17)

**Review Date**: 2025-07-14  
**Related Plan**: `.copilot-tracking/plans/thread-reply-input.md`  
**Related Changes**: `.copilot-tracking/changes/thread-reply-input.md`  
**Related Research**: None (plan self-contained with inline research)

---

## Review Summary

Three-phase implementation that routes Slack `AppMention` events arriving inside
threads through `pending_thread_replies` before forwarding to normal steering.
All required phases are present: `strip_mention` visibility change (Phase 1),
AppMention arm update (Phase 2), and proactive thread detection in `prompt.rs`
(Phase 3, recommended). Four unit tests are registered and passing.
All quality gates pass: 0 errors, 0 clippy warnings, 612/612 unit tests green.

---

## Implementation Checklist

### From Implementation Plan

#### Phase 1 — Make `strip_mention` pub(crate)

- [x] `fn strip_mention` changed to `pub(crate) fn strip_mention` in `steer.rs` line 259
  - Source: plan Phase 1
  - Status: Verified
  - Evidence: `src/slack/handlers/steer.rs:259` — `pub(crate) fn strip_mention(text: &str) -> &str`

#### Phase 2 — Route @mentions through `pending_thread_replies` first

- [x] `thread_ts` extracted from `mention.origin.thread_ts`
  - Source: plan Phase 2
  - Status: Verified
  - Evidence: `push_events.rs:52` — `let thread_ts = mention.origin.thread_ts.clone();`

- [x] `strip_mention` called BEFORE `route_thread_reply`
  - Source: plan Phase 2
  - Status: Verified
  - Evidence: `push_events.rs:62` — `let stripped = handlers::steer::strip_mention(text).trim();` then `route_thread_reply(... stripped ...)` at line 63

- [x] `Ok(true)` branch exits early via `return Ok(())`; does NOT fall through to `ingest_app_mention`
  - Source: plan Phase 2
  - Status: Verified
  - Evidence: `push_events.rs:72-80` — `Ok(true) => { … return Ok(()); }`

- [x] `Ok(false)` branch falls through to `ingest_app_mention` (normal mentions still work)
  - Source: plan Phase 2
  - Status: Verified
  - Evidence: `push_events.rs:81-83` — `Ok(false) => { // No pending fallback — fall through to normal steering. }` then execution reaches `ingest_app_mention` at line 96

- [x] `Err(e)` branch logs warning and returns without crashing
  - Source: plan Phase 2
  - Status: Verified
  - Evidence: `push_events.rs:84-92` — `Err(err) => { warn!(%err, channel = channel_id, "…"); return Ok(()); }`

- [x] Top-level mention (no `thread_ts`) skips `route_thread_reply` and goes straight to `ingest_app_mention`
  - Source: plan Phase 2 design notes
  - Status: Verified
  - Evidence: Guard `if let Some(ref ts) = thread_ts { … }` at `push_events.rs:61`; `ingest_app_mention` at line 96 is outside the guard.

- [x] `ingest_app_mention` still receives the original `text` (not stripped) on fall-through
  - Source: plan design note ("no double-strip")
  - Status: Verified
  - Evidence: `push_events.rs:96` — `handlers::steer::ingest_app_mention(text, &channel_id, &app).await;` — uses original `text` variable, not `stripped`

#### Phase 3 — Proactive thread detection in prompt.rs (Recommended)

- [x] Proactive detection block inserted after `let callback_id = …`
  - Source: plan Phase 3a
  - Status: Verified
  - Evidence: `prompt.rs:106-164` — `let is_thread_context = message.is_some_and(|m| m.origin.thread_ts.is_some());`

- [x] Proactive branch only fires when `thread_ts.is_some()`
  - Source: plan Phase 3a
  - Status: Verified
  - Evidence: `prompt.rs:106` — `is_some_and(|m| m.origin.thread_ts.is_some())`; guard correctly uses `is_some_and` (not just `is_some()` on the outer `message`)

- [x] Proactive block calls `activate_thread_reply_fallback` with correct `@agent-intercom` wording
  - Source: plan Phase 3a
  - Status: Verified
  - Evidence: `prompt.rs:124` — `"Please tag \`@agent-intercom\` in this thread with your revised instructions."`

- [x] Proactive block returns `Ok(())` early, skipping `views.open`
  - Source: plan Phase 3a
  - Status: Verified
  - Evidence: `prompt.rs:158-159` — `.await?; return Ok(());`

- [x] Fallback error path returns descriptive `Err` string
  - Source: plan Phase 3a
  - Status: Verified
  - Evidence: `prompt.rs:161-163` — `return Err("thread context: missing channel or thread_ts for Refine fallback".to_owned());`

- [x] Modal-failure fallback text updated to include `@agent-intercom` instruction
  - Source: plan Phase 3b
  - Status: Verified
  - Evidence: `prompt.rs:206` — `"Modal unavailable \u{2014} please tag \`@agent-intercom\` in this thread with your revised instructions."`

### From Success Criteria

- [x] `cargo check` clean — **Verified** (exit 0, 0 errors)
- [x] `cargo clippy -- -D warnings` zero warnings — **Verified** (exit 0, 0 warnings)
- [x] `cargo test` all tests pass — **Verified** (612/612 passed, 0 failed)
- [x] Unit test exercises `route_thread_reply` with @-mention-stripped text and confirms `Ok(true)` — **Verified** (`test_mr001_stripped_mention_captured_by_route_thread_reply`)
- [x] @-mention without pending fallback still reaches normal steering — **Verified** (`test_mr002_no_pending_entry_returns_false_so_steering_unaffected` + code path analysis)

---

## Validation Results

### Correctness

| Check | Result |
|-------|--------|
| `thread_ts` extracted from `mention.origin.thread_ts` | ✅ |
| `strip_mention` called before `route_thread_reply` | ✅ |
| `Ok(true)` exits early, does not reach `ingest_app_mention` | ✅ |
| `Ok(false)` falls through to `ingest_app_mention` | ✅ |
| `Err(e)` logs and returns, does not crash or route to steering | ✅ |
| Proactive branch only fires when `thread_ts.is_some()` | ✅ |
| Fallback wording instructs `@agent-intercom` submission | ✅ |

### Safety

| Check | Result |
|-------|--------|
| No `unwrap()` or `expect()` in new production code paths | ✅ |
| All `Result` values handled | ✅ |
| `Ok(true)` path does not corrupt `PendingThreadReplies` map | ✅ — entry is atomically removed by `route_thread_reply` before returning `true` |
| No `unsafe` blocks | ✅ |

### Tests

| Check | Result |
|-------|--------|
| `test_mr001`: @-mention in thread with pending waiter routes correctly (Ok(true), oneshot delivers text) | ✅ |
| `test_mr002`: @-mention in thread WITHOUT pending waiter returns Ok(false), fall-through safe | ✅ |
| `test_mr003`: top-level mention (mismatched thread_ts) returns Ok(false), existing entry untouched | ✅ |
| `test_mr004`: unauthorized @-mention returns Ok(false), pending entry preserved, oneshot not triggered | ✅ |
| Tests registered in `tests/unit.rs` `mod` block | ✅ — `mod slack_thread_mention_routing;` at line 47 |

### Convention Compliance

| Check | Result |
|-------|--------|
| `pub(crate)` used for `strip_mention` (not `pub`) | ✅ |
| Error messages lowercase, no trailing period | ✅ — e.g. `"push event: app state not available"`, `"oneshot receiver dropped before reply could be delivered"` |
| `warn!` calls use structured fields (`%err`, not `{err}`) | ✅ — `push_events.rs:85` uses `%err` |
| No new `allow` attributes required beyond existing `too_many_lines` | ✅ |

### Validation Commands

| Command | Result |
|---------|--------|
| `cargo check` | ✅ Finished with 0 errors |
| `cargo clippy -- -D warnings` | ✅ Finished with 0 warnings |
| `cargo test --test unit` | ✅ 612 passed; 0 failed |

---

## Additional or Deviating Changes

None. All changes match the plan exactly. The implementation uses
`is_some_and` (plan showed `map_or(false, …)`) — both are equivalent and
`is_some_and` is the more idiomatic Rust 1.70+ spelling. No deviation in intent.

---

## Missing Work

None identified. All required phases (1, 2) and the recommended phase (3)
are implemented and validated.

---

## Follow-Up Work

### Deferred from Current Scope

* **HITL / smoke test for end-to-end routing** — the unit tests validate the
  wiring of `route_thread_reply` directly but cannot exercise the full
  `handle_push_event` dispatch path (requires a live Socket Mode connection).
  - Source: plan Success Criteria ("verifiable via HITL test or manual smoke test")
  - Recommendation: add to the HITL test suite (`tests/live/`) once a
    Socket Mode test harness is available.

### Identified During Review

* None.

---

## Review Completion

**Overall Status**: Complete  
**Reviewer Notes**: All correctness, safety, test, and quality gate checks pass.
The implementation faithfully mirrors the plan specification. The proactive
thread-detection branch (Phase 3) is fully implemented and correctly guards on
`thread_ts.is_some()`. No unsafe code, no `unwrap`/`expect`, no silent Result
discards. The 4 new unit tests are registered, named clearly (MR-001–004), and
cover the critical happy-path, no-entry, mismatched-key, and unauthorized-sender
scenarios. The only outstanding item is a live HITL smoke test, which requires
infrastructure beyond the unit test scope and is appropriately deferred.
