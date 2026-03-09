<!-- markdownlint-disable-file -->
# PR Review Handoff: 007-acp-correctness-mobile

## PR Overview

Implements five ACP correctness fixes identified during HITL testing, plus a
thread-reply fallback mechanism for Slack modal inaccessibility on mobile and
Socket Mode environments. A post-review Phase 8 also resolved all five previously
deferred MEDIUM/LOW findings before opening the PR.

* Branch: `007-acp-correctness-mobile`
* Base Branch: `main`
* Final Commit: `fa9c225`
* Total Files Changed: 84 (src + tests)
* Total Review Comments: 14 fixed, 0 deferred (all resolved)

## PR Title

```
fix(acp): correctness fixes + thread-reply fallback for modal-inaccessible environments
```

## PR Description

```markdown
## Summary

Addresses five correctness issues found during ACP HITL testing and adds a
thread-reply fallback path for Slack environments where `views.open` is
unavailable (mobile, Socket Mode trigger_id expiry). A post-review pass
also resolved all five MEDIUM/LOW deferred items from the adversarial review.

## Fixes Included

### F-06 — Steering delivery reliability
`deliver_queued_messages` in `acp/reader.rs` now marks a message consumed
only after successful delivery. Previously a transient send error could silently
drop the message while still marking it consumed. `StreamActivity` is also now
emitted only for messages that were actually delivered (not all queued messages).

### F-07 — ACP session capacity enforcement
`count_active_acp()` now correctly counts `paused` sessions as occupying a
capacity slot (they hold a live child process). Previously an operator could
pause N sessions and start N more, bypassing the configured limit.

### F-10 — Workspace-only channel routing
SSE connections no longer accept an operator-supplied `channel_id` parameter.
Channel is resolved exclusively from the workspace-to-channel mapping, preventing
agents from routing messages to arbitrary channels.

### F-13 — Unique ACP correlation IDs
`generate_correlation_id()` now generates a fresh `Uuid::new_v4()` per call.
Previously a static string was reused, making log correlation across concurrent
sessions impossible.

### F-16 / F-17 — Thread-reply modal fallback
When `views.open` fails (trigger_id expired after Socket Mode relay delay),
handlers register a `oneshot` sender keyed by `"{channel_id}\x1f{thread_ts}"`.
The operator is asked to reply in thread; `push_events.rs` routes the first
authorized reply through the oneshot, resolving the pending prompt/wait/clearance.

**Security and reliability properties:**
- Only the session's registered `owner_user_id` can resolve a fallback (stored
  in the map at registration time — not accepted from caller at routing time)
- Duplicate registration is silently dropped to preserve the original waiter
  (mobile double-tap guard — LC-04)
- Buttons are replaced with ⏳ status immediately on fallback activation (FR-022)
- 5-minute timeout: spawned waiter tasks exit cleanly on `tokio::time::timeout`
- Zombie-waiter prevention: waiter is not spawned if the fallback Slack message
  fails to post
- Session-termination cleanup: `cleanup_session_fallbacks` drops all senders for
  a terminated session, unblocking any waiting tasks
- Composite key (`"{channel_id}\x1f{thread_ts}"`) prevents cross-channel collisions
- `Err` from `route_thread_reply` returns early — operator text is never injected
  into the steering pipeline

### Phase 8 Post-Review Improvements

- **T057**: Extracted `activate_thread_reply_fallback` helper, eliminating ~240 lines
  of triplication across `prompt.rs`, `wait.rs`, and `approval.rs`
- **T058**: Added unit tests S036–S038 for thread-reply negative paths (duplicate
  registration, sender drop, no-match lookup)
- **T059**: `deliver_queued_messages` returns delivered count; `StreamActivity` only
  emitted for successful deliveries
- **T060**: `register_thread_reply_fallback` guards against silent duplicate key
  overwrites with a `warn!` + early return
- **T061**: `SessionStatus::as_str()` added; all 8 hardcoded SQL status string literals
  in `session_repo.rs` replaced with bound parameters

## Test Coverage

| Area | Tests Added |
|------|-------------|
| F-06 steering delivery | 7 unit (incl. S008 delivered-count) |
| F-07 capacity enforcement | 7 unit + 3 contract |
| F-10 workspace routing | 4 unit + 3 contract |
| F-13 correlation IDs | 4 unit |
| F-16/F-17 fallback | 11 unit + 1 integration (incl. S036–S038) |
| T061 SessionStatus::as_str | 1 unit |
| **Total new** | **41 tests** |

471 tests pass total. Zero warnings from `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`.

## Quality Gates

- ✅ `cargo check` — clean
- ✅ `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` — clean (zero warnings)
- ✅ `cargo fmt --all -- --check` — clean
- ✅ `cargo test --all-targets` — 471 passed, 0 failed
```

## PR Comments Ready for Submission

### File: `src/slack/handlers/thread_reply.rs`

#### Comment 1 (Lines 88–98) — Duplicate registration guard (T060)

* Category: Reliability
* Severity: Approved ✅

> **T060 / LC-04**: `register_thread_reply_fallback` now checks `contains_key` before
> inserting. On a duplicate key (e.g., mobile double-tap sending two button events),
> the new `tx` is dropped — making its `rx` resolve to `Err` — and the original waiter
> is preserved. This prevents the first legitimate registration from being silently
> overwritten by a racing second call.

---

#### Comment 2 (Lines 222–312) — `activate_thread_reply_fallback` helper (T057)

* Category: Maintainability
* Severity: Approved ✅

> **T057 / TQ-008**: The ~80-line fallback setup block previously triplicated across
> `prompt.rs`, `wait.rs`, and `approval.rs` is now encapsulated in
> `activate_thread_reply_fallback`. Each caller supplies a typed async resolution
> callback (`FnOnce(String) -> impl Future`) for its specific action (DB update +
> driver resolution). The helper handles: duplicate-guarded registration, FR-022 button
> replacement, zombie-guard post, timeout-wrapped waiter spawn. `#[allow(clippy::too_many_arguments)]`
> annotates the 10-parameter signature with a rationale comment.

---

### File: `src/acp/reader.rs`

#### Comment 3 (Lines 433–519) — `deliver_queued_messages` returns count (T059)

* Category: Correctness
* Severity: Approved ✅

> **T059 / LC-05**: `deliver_queued_messages` now returns `usize` (delivered count,
> incremented only on `Ok(())` from `send_prompt`). `flush_queued_messages` uses
> `for _ in 0..delivered` to emit `StreamActivity` — previously it iterated over all
> queued messages including failures, potentially sending false "activity" signals to
> the stall detector for messages that were never received by the agent.

---

### File: `src/models/session.rs` + `src/persistence/session_repo.rs`

#### Comment 4 (session.rs lines 116–128; session_repo.rs 8 callsites) — `SessionStatus::as_str()` (T061)

* Category: Maintainability
* Severity: Approved ✅

> **T061 / CS-06**: `SessionStatus::as_str()` returns `&'static str` values matching
> the database `snake_case` column values. All 8 raw SQL string literals for session
> status in `session_repo.rs` are replaced with bound parameters (`.bind(SessionStatus::X.as_str())`),
> making typos a compile-time concern rather than a silent runtime query mismatch.

---

### File: `tests/unit/thread_reply_fallback.rs`

#### Comment 5 (Lines 316–450) — Negative-path unit tests S036–S038 (T058)

* Category: Test Quality
* Severity: Approved ✅

> **T058 / TQ-009**: Three new tests cover the previously untested negative paths:
> - **S036**: Duplicate registration preserves original sender; second `rx` resolves to `Err`
> - **S037**: Sender dropped via cleanup causes `rx` to resolve to `Err` (verifies the
>   observer side of the `cleanup_session_fallbacks` path)
> - **S038**: `route_thread_reply` with no pending entry returns `Ok(false)` (not `Err`,
>   not `true`) — the "reply arrives after timeout" race condition path

---

## Review Summary by Category

* Security Issues Fixed: 2 (auth no-op, TQ-004 misrouting)
* Reliability Issues Fixed: 5 (timeout, cleanup, zombie-waiter, capacity, duplicate registration)
* Correctness Issues Fixed: 3 (composite key, Paused capacity, StreamActivity for failures)
* Maintainability Issues Fixed: 2 (fallback triplication, hardcoded SQL literals)
* Test Quality Issues Fixed: 1 (negative-path test coverage)
* Deferred: 0 ✅ (all resolved before PR)

## Instruction Compliance

* ✅ `AGENTS.md` / `constitution.instructions.md`: No `unwrap`/`expect`, pedantic clippy clean, TDD discipline maintained, path safety, session ownership (FR-031/FR-010) enforced in fallback path
* ✅ All 471 tests pass (`cargo test --all-targets`)
* ✅ Zero warnings (`cargo clippy --all-targets -- -D warnings -D clippy::pedantic`)
* ✅ Format clean (`cargo fmt --all -- --check`)


## PR Overview

Implements five ACP correctness fixes identified during HITL testing, plus a
new thread-reply fallback mechanism for Slack modal inaccessibility on mobile
and Socket Mode environments.

* Branch: `007-acp-correctness-mobile`
* Base Branch: `main`
* Final Commit: `a764786`
* Total Files Changed: 66 (src + tests)
* Total Review Comments: 8 fixed, 5 deferred

## PR Title (suggested)

```
fix(acp): correctness fixes + thread-reply fallback for modal-inaccessible environments
```

## PR Description (suggested)

```markdown
## Summary

Addresses five correctness issues found during ACP HITL testing and adds a
thread-reply fallback path for Slack environments where `views.open` is
unavailable (mobile, Socket Mode trigger_id expiry).

## Fixes Included

### F-06 — Steering delivery reliability
`deliver_queued_messages` in `acp/reader.rs` now marks a message consumed
only after successful delivery. Previously, a transient send error could cause
the message to be silently dropped while still marked consumed in the DB.

### F-07 — ACP session capacity enforcement
`count_active_acp()` now correctly counts `paused` sessions as occupying a
capacity slot (they hold a live child process). Previously an operator could
pause N sessions and start N more, bypassing the configured limit.

### F-10 — Workspace-only channel routing
SSE connections no longer accept an operator-supplied `channel_id` parameter.
Channel is resolved exclusively from the workspace-to-channel mapping, preventing
agents from routing messages to arbitrary channels.

### F-13 — Unique ACP correlation IDs
`generate_correlation_id()` now generates a fresh `Uuid::new_v4()` per call.
Previously a static string was reused, making log correlation across concurrent
sessions impossible.

### F-16 / F-17 — Thread-reply modal fallback
When `views.open` fails (trigger_id expired after Socket Mode relay delay),
handlers register a `oneshot` sender keyed by `"{channel_id}\x1f{thread_ts}"`.
The operator is asked to reply in thread; `push_events.rs` routes the first
authorized reply through the oneshot, resolving the pending prompt/wait/clearance.

**Security and reliability properties:**
- Only the session's registered `owner_user_id` can resolve a fallback (stored
  in the map at registration time, not supplied by the caller)
- Buttons are replaced with ⏳ status immediately on fallback activation (FR-022)
- 5-minute timeout: spawned waiter tasks exit cleanly on `tokio::time::timeout`
- Zombie-waiter prevention: waiter is not spawned if the fallback Slack message
  fails to post
- Session-termination cleanup: `cleanup_session_fallbacks` drops all senders for
  a terminated session, unblocking any waiting tasks
- Composite key prevents cross-channel collisions when two channels share a timestamp
- `Err` from `route_thread_reply` returns early — operator text is never injected
  into the steering pipeline

## Test Coverage

| Area | Tests Added |
|------|-------------|
| F-06 steering delivery | 6 unit |
| F-07 capacity enforcement | 7 unit + 3 contract |
| F-10 workspace routing | 4 unit + 3 contract |
| F-13 correlation IDs | 4 unit |
| F-16/F-17 fallback | 8 unit + 1 integration |
| **Total new** | **36 tests** |

466 tests pass total.

## Known Deferred Items (non-blocking, follow-up tickets planned)

- **TQ-008**: Fallback logic is triplicated across `prompt.rs`, `wait.rs`,
  `approval.rs`. A shared `spawn_thread_reply_fallback()` helper would reduce
  divergence risk. Planned for a follow-up refactor.
- **TQ-009**: Missing push_event integration tests for negative paths (unauthorized
  reply, timeout expiry, duplicate registration). Follow-up ticket.
- **LC-05**: `StreamActivity` events emitted in `deliver_queued_messages` for all
  queued messages including those that failed delivery. Could send false stall-detector
  signals. Low frequency in practice; follow-up ticket.
- **CS-06**: Hardcoded SQL status strings in `count_active_acp`. No behavioral impact;
  future refactor to use enum constants.
```

## PR Comments Ready for Submission

### File: `src/slack/handlers/thread_reply.rs`

#### Comment 1 (Lines 44–45) — Design note: 3-tuple rationale

* Category: Documentation
* Severity: Informational

> **Design note**: The `PendingThreadReplies` map stores a 3-tuple
> `(session_id, authorized_user_id, Sender)`. The `session_id` enables
> `cleanup_session_fallbacks` to purge all entries for a terminated session
> without needing to know which threads it owned. The `authorized_user_id`
> is the session's `owner_user_id` captured at registration time — it is
> **not** accepted from the caller at routing time, preventing any authorized
> user from hijacking a prompt meant for the session owner.

---

#### Comment 2 (Lines 107–158) — Security: auth enforced from map, not caller

* Category: Security
* Severity: Approved ✅

> The `authorized_user_id` is extracted from the stored map value (line 122–127),
> not from a caller-supplied argument. This closes the tautology that previously
> made the ownership check always pass when the caller passed the sender's own ID
> for both parameters.

---

### File: `src/slack/push_events.rs`

#### Comment 3 (Lines 139–145) — TQ-004: Err branch exits early

* Category: Correctness
* Severity: Approved ✅

> **Previously**: `Err` from `route_thread_reply` fell through to
> `steer::store_from_slack`, treating the operator's fallback reply text as an
> agent steering command.
>
> **Now**: `Err` returns `Ok(())` immediately — the message targeted a pending
> fallback entry (the receiver was dropped due to timeout or session end) and
> must not be re-routed into the steering pipeline.

---

### File: `src/slack/handlers/prompt.rs` (also `wait.rs`, `approval.rs`)

#### Comment 4 (Lines 166–180) — Zombie-waiter prevention

* Category: Reliability
* Severity: Approved ✅

> Waiter task is only spawned after `slack.enqueue(fallback_msg)` succeeds.
> If the post fails, the pending map entry is removed and `Err` is returned.
> Without this guard, operators would receive no prompt but the system would
> wait indefinitely for a reply that could never come.

---

#### Comment 5 (Lines 184–220) — Timeout prevents indefinite task accumulation

* Category: Reliability
* Severity: Approved ✅

> `tokio::time::timeout(FALLBACK_REPLY_TIMEOUT, rx)` (300 seconds) wraps the
> `rx.await`. On timeout, the task exits cleanly. Without this, every failed
> modal over the server's lifetime would accumulate a suspended `Arc<AppState>`
> reference, preventing clean shutdown.

---

### File: `src/main.rs`

#### Comment 6 (Lines 890–896) — Session-termination cleanup

* Category: Reliability
* Severity: Approved ✅

> `cleanup_session_fallbacks` drops all `oneshot::Sender`s owned by the
> terminated session. The corresponding `rx` in each spawned task receives
> `RecvError`, causing the timeout arm to fire and the task to exit cleanly.
> Without this, stale senders for a terminated session could intercept replies
> intended for a new session that reuses the same Slack thread.

---

### File: `src/persistence/session_repo.rs`

#### Comment 7 (Lines 483–494) — capacity fix includes Paused

* Category: Correctness
* Severity: Approved ✅

> `count_active_acp` now counts `status IN ('active', 'created', 'paused')`.
> A `Paused` session retains its child process and holds a real capacity slot.
> Excluding it allowed an operator to pause N sessions and start N more,
> effectively doubling the configured session limit.

---

## Deferred Items Summary (for PR description)

| Item | Severity | Plan |
|------|----------|------|
| TQ-008: Fallback triplication | MEDIUM | Follow-up refactor — `spawn_thread_reply_fallback()` helper |
| TQ-009: Push_event negative-path tests | MEDIUM | Follow-up ticket |
| LC-05: StreamActivity for failed deliveries | MEDIUM | Follow-up ticket |
| CS-06: Hardcoded SQL status strings | LOW | Future refactor |

## Review Summary by Category

* Security Issues Fixed: 2 (auth no-op, TQ-004 misrouting)
* Reliability Issues Fixed: 4 (timeout, cleanup, zombie-waiter, capacity)
* Correctness Issues Fixed: 2 (composite key, Paused capacity)
* Deferred: 4 (all MEDIUM or LOW, non-blocking)

## Instruction Compliance

* ✅ `AGENTS.md` / `constitution.instructions.md`: No `unwrap`/`expect`, pedantic clippy clean, TDD discipline, path safety, session ownership (FR-031/FR-010) enforced in fallback path
* ✅ All 466 tests pass (`cargo test --all-targets`)
* ✅ Zero clippy warnings (`cargo clippy --all-targets -- -D warnings -D clippy::pedantic`)
* ✅ Format clean (`cargo fmt --all -- --check`)
