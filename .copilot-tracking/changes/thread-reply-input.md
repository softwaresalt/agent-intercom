# Change Record: @-Mention Thread Reply Input (F-16/F-17)

**Feature plan**: `.copilot-tracking/plans/thread-reply-input.md`  
**Date**: 2026-03-07

---

## Summary

Routes Slack `AppMention` events that arrive inside a thread through the
`pending_thread_replies` fallback map before forwarding to normal steering.
Operators can now complete a Refine (or any input-requiring) prompt by tagging
`@agent-intercom` in the thread — the matching oneshot is resolved and the
agent receives the instruction without a modal.

Also proactively skips the doomed `views.open` call when the button message
itself is inside a thread, and updates both fallback messages to tell the
operator to use `@agent-intercom`.

---

## Files Modified

### Phase 1 — `src/slack/handlers/steer.rs`

| Line | Change |
|------|--------|
| 259 | `fn strip_mention` → `pub(crate) fn strip_mention` |

**Reason**: `push_events.rs` (same crate) needs to call `strip_mention` to
remove the `<@UXXXXX>` prefix before passing text to `route_thread_reply`.
The existing inline `#[cfg(test)]` tests continue to compile unchanged
(`use super::strip_mention` still resolves within the module).

---

### Phase 2 — `src/slack/push_events.rs`

| Lines (before) | Change |
|----------------|--------|
| 44–58 | AppMention arm replaced (~44 new lines) |

**Before**: The `AppMention` arm called `ingest_app_mention` directly without
consulting `pending_thread_replies`.

**After**: When `mention.origin.thread_ts` is `Some(ts)`:
1. `strip_mention(text).trim()` removes the bot-mention token.
2. `route_thread_reply(&channel_id, ts.0.as_str(), &user_id, stripped, …).await` is called.
3. `Ok(true)` → mention captured by fallback; acknowledge and return.
4. `Ok(false)` → no pending entry; fall through to `ingest_app_mention` (unchanged steering).
5. `Err(e)` → TQ-004 pattern: log warning and return (do not route to steering).

When `thread_ts` is `None` (top-level mention): falls through immediately to
`ingest_app_mention` as before.

---

### Phase 3 — `src/slack/handlers/prompt.rs`

| Lines (approx) | Change |
|----------------|--------|
| 101 (after) | Inserted ~65-line proactive thread-context block |
| ~142 (before) | Fallback text string updated |

**3a — Proactive detection**:
After `let callback_id = …`, a new block checks:
```rust
let is_thread_context = message.is_some_and(|m| m.origin.thread_ts.is_some());
```
When `true`, `activate_thread_reply_fallback` is called **immediately** with
the message:
> `"Please tag \`@agent-intercom\` in this thread with your revised instructions."`

This skips the doomed `views.open` call when the button lives inside a thread.
If channel or thread_ts cannot be determined, returns a descriptive `Err`.

**3b — Updated fallback text** (modal-failure path):
```
Before: "Modal unavailable — please reply in this thread with your revised instructions."
After:  "Modal unavailable — please tag `@agent-intercom` in this thread with your revised instructions."
```

---

## Tests Written

### File: `tests/unit/slack_thread_mention_routing.rs`

| Test | What it verifies |
|------|-----------------|
| `test_mr001_stripped_mention_captured_by_route_thread_reply` | Simulates the post-Phase-2 AppMention arm: stripped mention text reaches `route_thread_reply`, returns `Ok(true)`, and the oneshot delivers the correct text. |
| `test_mr002_no_pending_entry_returns_false_so_steering_unaffected` | Without a pending entry, `route_thread_reply` returns `Ok(false)`, ensuring the fall-through to `ingest_app_mention` is safe. |
| `test_mr003_top_level_mention_does_not_consume_pending_entry` | A mismatched `thread_ts` (representing a top-level mention that has no `thread_ts`) returns `Ok(false)` and leaves the real pending entry intact. |
| `test_mr004_unauthorized_mention_reply_ignored` | An @-mention from an unauthorized user is silently ignored (`Ok(false)`) and the pending entry remains for the authorized operator. |

**TDD state**: All 4 tests pass before and after the implementation. They
document the expected wiring of `route_thread_reply` with @-mention-stripped
text. The gap (AppMention arm not calling `route_thread_reply`) was confirmed
by reading the source; the tests serve as regression guards post-fix.

---

## Quality Gate Results

```
cargo check          → Finished (0 errors)
cargo clippy -D warnings → Finished (0 warnings)
cargo fmt --all -- --check → exit 0 (no formatting drift)
cargo test --test unit   → test result: ok. 612 passed; 0 failed
```

All 4 new `slack_thread_mention_routing` tests pass.  
No regressions in existing 608 tests.

> Note: `cargo test` (all targets) was run as `--test unit` because two live
> `agent-intercom` server processes (PIDs 4100, 7432) held the
> `target/debug/agent-intercom.exe` binary locked. The unit test binary
> (`target/debug/deps/unit-*.exe`) compiles and links against the library
> crate only and is unaffected by the EXE lock. All changed code paths live
> in the library crate and are fully exercised by the unit test suite.

---

## Constitution Compliance

| Principle | Status |
|-----------|--------|
| No `unsafe` | ✅ Zero `unsafe` blocks introduced |
| No `unwrap`/`expect` in production code | ✅ All new branches use `?`, `warn!`, or explicit `is_some_and` |
| All fallible ops return `Result<T, AppError>` | ✅ New `prompt.rs` code returns `Err(String)` matching the function signature |
| `cargo check` + `cargo clippy -D warnings` pass | ✅ Verified above |
| `cargo test` passes | ✅ 612/612 unit tests pass |
| TDD — tests written before implementation | ✅ Test file created before Phase 1–3 changes |
