# Implementation Plan: @-Mention Thread-Reply Input (F-16/F-17 @-mention routing)

## Overview

Route Slack `AppMention` events that arrive inside a thread through the
`pending_thread_replies` fallback map before forwarding to normal steering,
so operators can complete a Refine (or any input-requiring) prompt by
tagging `@agent-intercom` in the thread.

---

## Problem Statement

When a Block-Kit **Refine** button is clicked inside a Slack thread:

1. `handle_prompt_action` calls `slack.open_modal()`.
2. Slack silently suppresses the modal (Socket Mode limitation in threads).
3. The existing F-16/F-17 fallback fires: `activate_thread_reply_fallback`
   posts a plain-text prompt and registers a oneshot in `pending_thread_replies`.
4. The operator sees the prompt and replies with `@agent-intercom <instructions>`.
5. **Bug**: the `AppMention` handler in `push_events.rs` calls
   `ingest_app_mention` **without** first consulting `pending_thread_replies`,
   so the reply is routed to normal steering instead of resolving the Refine prompt.

Plain message (non-@) thread replies already work — they go through
`route_thread_reply` at `push_events.rs:117-152`.  Only @-mention replies
are broken.

---

## Objectives

* **Required – Fix the routing gap**: Make `AppMention` events inside a
  thread consult `pending_thread_replies` first, identical to the existing
  plain-message path.
* **Required – Expose `strip_mention`**: Change its visibility from
  `fn` to `pub(crate)` so `push_events.rs` can strip the bot-mention prefix
  before calling `route_thread_reply`.
* **Recommended – Proactive skip**: When the button message is itself
  inside a thread, skip the doomed `views.open` call and activate the
  fallback immediately, with a message that explicitly tells the operator
  to use `@agent-intercom`.
* All changes pass `cargo check`, `cargo clippy -- -D warnings`, and
  `cargo test`.

---

## Context References

| Artefact | Relevance |
|---|---|
| `src/slack/push_events.rs` | AppMention arm (lines 44-58); Message arm (lines 114-152 — the working reference pattern) |
| `src/slack/handlers/steer.rs` | `strip_mention` fn (line 259); `ingest_app_mention` (line 240) |
| `src/slack/handlers/prompt.rs` | `handle_prompt_action` `prompt_refine` branch (lines 95-184) |
| `src/slack/handlers/thread_reply.rs` | `route_thread_reply` signature (line 125); `activate_thread_reply_fallback` (line 223) |
| `.copilot-tracking/research/thread-reply-input.md` | Full code-path trace and design rationale |
| `.github/instructions/constitution.instructions.md` | Safety-first Rust, test-first, clippy pedantic, no `unwrap` |

---

## Implementation Phases

---

### Phase 1 — Make `strip_mention` pub(crate)

<!-- parallelizable: true -->

**File**: `src/slack/handlers/steer.rs`  
**Line**: 259

**Current code (line 259):**
```rust
fn strip_mention(text: &str) -> &str {
```

**New code:**
```rust
pub(crate) fn strip_mention(text: &str) -> &str {
```

**Why**: `push_events.rs` (Phase 2) needs to call `strip_mention` to
remove the `<@UXXXXX>` prefix before passing text to `route_thread_reply`.
The function already has unit tests at `steer.rs:282-302`; no new tests
are needed for the visibility change — existing tests still compile and pass.

**Validation:**
```
cargo check
cargo clippy -- -D warnings
cargo test -p agent-intercom handlers::steer
```

---

### Phase 2 — Route @mentions through `pending_thread_replies` first

<!-- parallelizable: false (depends on Phase 1) -->

**File**: `src/slack/push_events.rs`  
**Lines to modify**: 44–58 (the `AppMention` arm)

#### Exact change

Replace the entire `AppMention` match arm (lines 44–58) with the following:

```rust
SlackEventCallbackBody::AppMention(mention) => {
    let user_id = mention.user.to_string();

    if !is_authorized(&user_id, &app) {
        return Ok(());
    }

    let channel_id = mention.channel.to_string();
    let thread_ts = mention.origin.thread_ts.clone();
    let text = mention.content.text.as_deref().unwrap_or_default();

    info!(user_id, channel_id, "push event: app mention received");

    // F-16/F-17: When the @-mention arrives inside a thread, check for a
    // pending thread-reply fallback first (e.g., a Refine prompt whose
    // modal could not be opened).  This mirrors the plain-message path at
    // lines 114-152 and closes the routing gap for @-mention replies.
    if let Some(ref ts) = thread_ts {
        let stripped = handlers::steer::strip_mention(text).trim();
        match crate::slack::handlers::thread_reply::route_thread_reply(
            &channel_id,
            ts.0.as_str(),
            &user_id,
            stripped,
            Arc::clone(&app.pending_thread_replies),
        )
        .await
        {
            Ok(true) => {
                info!(
                    user = user_id,
                    channel = channel_id,
                    "push event: @mention in thread captured by modal fallback (F-16/F-17)"
                );
                post_ack(&app, &channel_id, Some(ts)).await;
                return Ok(());
            }
            Ok(false) => {
                // No pending fallback — fall through to normal steering.
            }
            Err(err) => {
                warn!(
                    %err,
                    channel = channel_id,
                    "push event: thread-reply fallback routing error on @mention; \
                     skipping steering (TQ-004)"
                );
                return Ok(());
            }
        }
    }

    handlers::steer::ingest_app_mention(text, &channel_id, &app).await;
    post_ack(&app, &channel_id, thread_ts.as_ref()).await;
}
```

#### Design notes

* `stripped` is used for `route_thread_reply` so the resolved `reply_text`
  reaching the prompt's `resolve` callback does not contain the bot-mention
  token.  When the fallback is **not** triggered (`Ok(false)`), the original
  `text` (still containing the mention) is passed to `ingest_app_mention`,
  which calls `strip_mention` internally — no double-strip.
* The `Err` branch follows the same TQ-004 pattern as the plain-message path
  at `push_events.rs:139-151`: the @-mention was targeting a pending fallback
  entry that has since timed out or been dropped; do **not** route it to steering.
* The `#[allow(clippy::too_many_lines)]` at line 27 already covers the
  function; no new attribute is needed.

#### Tests to write (test-first per constitution)

Add to `src/slack/push_events.rs` (or `tests/unit/push_events_test.rs`)
a unit test that:

1. Creates a `PendingThreadReplies` map.
2. Registers a oneshot via `register_thread_reply_fallback` for a known
   `(channel, thread_ts)` pair.
3. Calls `route_thread_reply` directly with an @-mention-stripped text
   and the same channel/thread_ts.
4. Asserts `Ok(true)` is returned and the oneshot receiver yields the text.

This validates the wiring logic without requiring a full
`SlackClientEventsUserState` mock.

**Validation:**
```
cargo check
cargo clippy -- -D warnings
cargo test
```

---

### Phase 3 — Proactive thread detection in prompt.rs *(Recommended)*

<!-- parallelizable: false (sequential after Phase 2; touches prompt.rs independently but logically follows) -->

**File**: `src/slack/handlers/prompt.rs`  
**Lines**: 95–184 (the `prompt_refine` branch)

Skip the doomed `views.open` call when the button itself was clicked
inside a Slack thread, and update the fallback message to tell the operator
to use `@agent-intercom`.

#### 3a — Insert proactive detection block

After line 101 (`let callback_id = format!("prompt_refine:{prompt_id}");`),
inside the `if let Some(ref slack) = state.slack {` block, insert:

```rust
// F-16/F-17 proactive: Slack silently suppresses views.open when the
// triggering message lives inside a thread (origin.thread_ts is Some).
// Skip the doomed views.open call and activate the fallback immediately.
let is_thread_context = message.map_or(false, |m| m.origin.thread_ts.is_some());
if is_thread_context {
    let thread_ts_opt = message.map(|m| {
        m.origin
            .thread_ts
            .as_ref()
            .map_or_else(|| m.origin.ts.0.clone(), |ts| ts.0.clone())
    });
    let chan_id_opt = channel.map(|c| c.id.to_string());
    if let (Some(thread_ts), Some(chan_id)) = (thread_ts_opt, chan_id_opt) {
        let button_msg_ts = message.map(|m| m.origin.ts.clone());
        let state_clone = Arc::clone(state);
        let prompt_id_owned = prompt_id.to_owned();
        crate::slack::handlers::thread_reply::activate_thread_reply_fallback(
            chan_id.as_str(),
            thread_ts.as_str(),
            prompt_session_id.clone(),
            user_id.to_owned(),
            "Please tag `@agent-intercom` in this thread with your revised instructions.",
            button_msg_ts,
            slack,
            Arc::clone(&state.pending_thread_replies),
            prompt_id,
            move |reply_text| async move {
                let repo = PromptRepo::new(Arc::clone(&state_clone.db));
                if let Err(db_err) = repo
                    .update_decision(
                        &prompt_id_owned,
                        PromptDecision::Refine,
                        Some(reply_text.clone()),
                    )
                    .await
                {
                    warn!(
                        prompt_id = prompt_id_owned,
                        %db_err,
                        "thread-reply fallback: failed to update prompt decision in DB"
                    );
                }
                if let Err(driver_err) = state_clone
                    .driver
                    .resolve_prompt(&prompt_id_owned, "refine", Some(reply_text))
                    .await
                {
                    warn!(
                        prompt_id = prompt_id_owned,
                        %driver_err,
                        "thread-reply fallback: failed to resolve prompt via driver"
                    );
                }
            },
        )
        .await?;
        return Ok(());
    }
    return Err("thread context: missing channel or thread_ts for Refine fallback".into());
}
```

#### 3b — Update the existing modal-failure fallback message (line 142)

The `activate_thread_reply_fallback` call in the `open_modal` failure path
(currently at ~line 137 of `prompt.rs`) still has the old wording.
Update its `fallback_text` argument:

```rust
// Before:
"Modal unavailable \u{2014} please reply in this thread with your revised instructions.",

// After:
"Modal unavailable \u{2014} please tag `@agent-intercom` in this thread with your revised instructions.",
```

This ensures the operator receives the correct instruction in the
still-reachable fallback path (e.g., non-thread messages where `open_modal`
fails for other reasons like an expired `trigger_id`).

#### Note on `clippy::too_many_lines`

`handle_prompt_action` already carries `#[allow(clippy::too_many_lines)]`
(line 39). The added block does not require a new attribute.

**Validation:**
```
cargo check
cargo clippy -- -D warnings
cargo test
```

---

### Phase 4 — Full Validation

<!-- parallelizable: false -->

Run the full quality gate:

```bash
cargo check
cargo clippy -- -D warnings
cargo test
```

If any clippy lint fires on the new code:
- Prefer `map_or` / `map_or_else` over chained `.map().unwrap_or()`.
- Ensure all `warn!` calls use structured fields (`%err`, not `{err}`).
- Do **not** use `unwrap()` or `expect()` anywhere in new code.

If test failures arise in unrelated modules, report them without
attempting large-scale fixes.

---

## Constitution Compliance Check

| Principle | Compliance |
|---|---|
| **I. Safety-first Rust** | No `unsafe`, no `unwrap`/`expect`. All error branches use `warn!` + `return Ok(())` or propagate via `?`. The new `Err` branch in the AppMention arm follows the established TQ-004 pattern. |
| **II. MCP Protocol Fidelity** | Changes are Slack-layer only; MCP tool surface and protocol handling are untouched. |
| **III. Test-first development** | Phase 2 specifies a unit test (oneshot wiring) to be written **before** the push_events.rs change. Existing `strip_mention` tests remain. |
| **IV. Security Boundary** | No new file-system or credential access. Authorization check (`is_authorized`) runs before the new `route_thread_reply` call — unauthorized users are rejected at line 47, never reaching the fallback routing. |
| **Clippy pedantic** | No new `allow` attributes needed. Existing `too_many_lines` suppressions cover the modified functions. |

---

## File Summary

| File | Change | Phase |
|---|---|---|
| `src/slack/handlers/steer.rs` | `fn strip_mention` → `pub(crate) fn strip_mention` (line 259) | 1 |
| `src/slack/push_events.rs` | AppMention arm: insert `route_thread_reply` check before `ingest_app_mention` (lines 44–58 replaced) | 2 |
| `src/slack/handlers/prompt.rs` | Insert `is_thread_context` proactive branch after line 101; update fallback text at ~line 142 | 3 (optional) |

Total changed lines: ~15 (Phase 1+2, required) + ~45 (Phase 3, recommended).

---

## Success Criteria

- [ ] `cargo check` clean.
- [ ] `cargo clippy -- -D warnings` zero warnings.
- [ ] `cargo test` all tests pass (no regressions).
- [ ] A unit test exercises `route_thread_reply` with @-mention-stripped text and confirms `Ok(true)`.
- [ ] An @-mention reply in a thread with a registered pending fallback resolves the Refine prompt (verifiable via HITL test or manual smoke test).
- [ ] An @-mention reply in a thread **without** a registered pending fallback still reaches normal steering (`ingest_app_mention` is called).
