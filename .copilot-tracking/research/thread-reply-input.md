<!-- markdownlint-disable-file -->
# Research: @-Mention Thread Reply Input — Replacing `views.open` Modal for Refine in Threads

## Problem Statement

When a Block Kit **Refine** button is clicked inside a Slack thread, `slack.open_modal()` (which calls `views.open` in Socket Mode) **fails silently** because Slack suppresses modal opens triggered from thread-context messages. The current codebase already handles this failure with a thread-reply fallback (F-16/F-17), posting a plain-text prompt into the thread and routing the first authorized reply through a `oneshot` channel.

The proposed @-mention variant would change step 3 of the fallback: instead of routing *any* plain-text reply, it would only route a reply that **@-mentions the bot** (e.g. `@agent-intercom refine the search logic`). This document maps the existing code, identifies what already works, and specifies the minimal delta to implement the @-mention variant.

---

## 1. Current Modal Flow — Full Code Path

### 1.1 Button click → `handle_prompt_action`

**File:** `src/slack/handlers/prompt.rs`

```
SlackInteractionEvent::BlockActions
  → events.rs::handle_interaction()          (lines 104-228)
      dispatch: action_id starts_with("prompt_")
      → handlers::prompt::handle_prompt_action()  (line 175)
```

**Signature** (`prompt.rs:40`):
```rust
pub async fn handle_prompt_action(
    action: &SlackInteractionActionInfo,
    user_id: &str,
    trigger_id: &SlackTriggerId,
    channel: Option<&SlackBasicChannelInfo>,
    message: Option<&SlackHistoryMessage>,
    state: &Arc<AppState>,
) -> Result<(), String>
```

### 1.2 `prompt_refine` branch

**`prompt.rs:95-184`** — when `action_id == "prompt_refine"`:

1. Build `callback_id = format!("prompt_refine:{prompt_id}")` (line 100).
2. Cache `(channel_id, message_ts)` in `state.pending_modal_contexts` (lines 107-110).
3. Build modal via `blocks::instruction_modal(&callback_id, "Refine", "...")` (line 112).
4. Call `slack.open_modal(trigger_id.clone(), modal).await` (line 117).
5. **On success** → return early; wait for `ViewSubmission` event.
6. **On failure** (line 118 `warn!`) → clean up `pending_modal_contexts`, then activate thread-reply fallback (lines 122-178).

### 1.3 Thread-reply fallback activation

**`prompt.rs:122-178`** (called when `open_modal` fails):

```rust
crate::slack::handlers::thread_reply::activate_thread_reply_fallback(
    chan_id.as_str(),
    thread_ts.as_str(),
    prompt_session_id.clone(),
    user_id.to_owned(),
    "Modal unavailable — please reply in this thread with your revised instructions.",
    button_msg_ts,
    slack,
    Arc::clone(&state.pending_thread_replies),
    prompt_id,
    move |reply_text| async move {
        // update DB + resolve via driver
        repo.update_decision(&prompt_id_owned, PromptDecision::Refine, Some(reply_text.clone())).await;
        state_clone.driver.resolve_prompt(&prompt_id_owned, "refine", Some(reply_text)).await;
    },
).await?;
```

The `thread_ts` used as the map key is computed at **`prompt.rs:126-131`**:
```rust
let thread_ts_opt = message.map(|m| {
    m.origin
        .thread_ts
        .as_ref()
        .map_or_else(|| m.origin.ts.0.clone(), |ts| ts.0.clone())
});
```
This means: use `message.origin.thread_ts` if present (button is inside a thread reply), otherwise use `message.origin.ts` (button is the thread root).

### 1.4 `activate_thread_reply_fallback` — `thread_reply.rs:223-318`

**Signature:**
```rust
pub async fn activate_thread_reply_fallback<F, Fut>(
    chan_id: &str,
    thread_ts: &str,
    session_id: String,
    authorized_user_id: String,
    fallback_text: &str,
    button_msg_ts: Option<SlackTs>,
    slack: &SlackService,
    pending: PendingThreadReplies,
    log_context: &str,
    resolve: F,
) -> Result<(), String>
```

**Steps:**
1. Create `(tx, rx) = oneshot::channel::<String>()`.
2. Call `register_thread_reply_fallback(chan_id, thread_ts, session_id, authorized_user_id, tx, pending)` (line 240).
3. Optionally update button message to ⏳ indicator (FR-022) — lines 251-265.
4. Post `fallback_text` as a thread reply in `thread_ts` (lines 268-291).
5. Zombie-guard: only spawn waiter task if post succeeded (line 276).
6. Spawn tokio task that `timeout(FALLBACK_REPLY_TIMEOUT, rx).await` and calls `resolve(reply_text)` on success.

`FALLBACK_REPLY_TIMEOUT = 300s` (`thread_reply.rs:39`).

### 1.5 Modal submission resolution (alternate path)

**File:** `src/slack/handlers/modal.rs:35-94`

When `views.open` **succeeds**, the operator types in the modal and submits. `handle_view_submission` is called, extracting the instruction text from:
```
view.state_params.state.values["instruction_block"]["instruction_text"].value
```
Then `resolve_prompt(prompt_id, ...)` updates the DB and calls `state.driver.resolve_prompt(prompt_id, "refine", Some(instruction))`.

---

## 2. Thread Reply Routing — `PendingThreadReplies` and `route_thread_reply`

### 2.1 Type definition

**`thread_reply.rs:48-49`:**
```rust
pub type PendingThreadReplies =
    Arc<Mutex<HashMap<String, (String, String, oneshot::Sender<String>)>>>;
```
Value tuple: `(session_id, authorized_user_id, Sender<String>)`.

**Re-exported** from `mcp/handler.rs:103`:
```rust
pub use crate::slack::handlers::thread_reply::PendingThreadReplies;
```

### 2.2 Map key format

**`thread_reply.rs:59-61`:**
```rust
pub fn fallback_map_key(channel_id: &str, thread_ts: &str) -> String {
    format!("{channel_id}\x1f{thread_ts}")
}
```
ASCII Unit Separator (`\x1f`) prevents cross-channel collisions (CS-02/LC-05).

### 2.3 `route_thread_reply` — `thread_reply.rs:125-177`

**Signature:**
```rust
pub async fn route_thread_reply(
    channel_id: &str,
    thread_ts: &str,
    sender_user_id: &str,
    text: &str,
    pending: PendingThreadReplies,
) -> Result<bool, String>
```

**Behaviour:**
- Looks up `fallback_map_key(channel_id, thread_ts)` in the map.
- If not found: returns `Ok(false)` — no pending fallback.
- If found but `sender_user_id != authorized_user_id`: logs warning and returns `Ok(false)` — **entry stays** for the authorized user.
- If found and authorized: removes entry, sends `text` through oneshot, returns `Ok(true)`.

### 2.4 `register_thread_reply_fallback` — `thread_reply.rs:78-99`

Inserts `(session_id, authorized_user_id, tx)` into the map keyed by composite key.
- Duplicate guard (LC-04): if key already exists, drops the new `tx` (making `rx` resolve to `Err` immediately).

### 2.5 `cleanup_session_fallbacks` — `thread_reply.rs:184-187`

```rust
pub async fn cleanup_session_fallbacks(session_id: &str, pending: &PendingThreadReplies) {
    let mut guard = pending.lock().await;
    guard.retain(|_key, entry| entry.0.as_str() != session_id);
}
```
Called when a session terminates (F-20).

---

## 3. App Mention Handling — `push_events.rs`

### 3.1 Dispatch

**`push_events.rs:44-58`** — `SlackEventCallbackBody::AppMention` branch:
```rust
SlackEventCallbackBody::AppMention(mention) => {
    let user_id = mention.user.to_string();
    // authorization check
    let channel_id = mention.channel.to_string();
    let thread_ts = mention.origin.thread_ts.clone();
    let text = mention.content.text.as_deref().unwrap_or_default();
    handlers::steer::ingest_app_mention(text, &channel_id, &app).await;
    post_ack(&app, &channel_id, thread_ts.as_ref()).await;
}
```

**Critical observation:** The `AppMention` handler calls `ingest_app_mention`, which routes to the **steering** path, not to `route_thread_reply`. This means `AppMention` events **bypass the pending thread-reply fallback check** entirely.

### 3.2 `ingest_app_mention` — `steer.rs:240-253`

```rust
pub async fn ingest_app_mention(text: &str, channel_id: &str, state: &Arc<AppState>) {
    let stripped = strip_mention(text).trim().to_owned();
    if stripped.is_empty() { return; }
    match store_from_slack(&stripped, Some(channel_id), None, state).await {
        Ok(msg) => info!(...),
        Err(err) => warn!(...),
    }
}
```

Note: `thread_ts` is **not passed** to `store_from_slack` from the `AppMention` path (line 249 passes `None` for thread_ts).

### 3.3 Message events with thread context — `push_events.rs:61-169`

The `SlackEventCallbackBody::Message` handler **does** check `route_thread_reply` first (lines 117-152):
```rust
match route_thread_reply(&channel_str, thread_ts.0.as_str(), &user_str, text, ...).await {
    Ok(true) => { post_ack(...); return Ok(()); }  // fallback consumed
    Ok(false) => { /* fall through to steering */ }
    Err(err) => { return Ok(()); }  // TQ-004: don't steer on routing error
}
```
Only **thread replies** (messages with `origin.thread_ts` set) reach this path.

---

## 4. Thread Context Detection from Block Actions Payload

### 4.1 What the Slack API provides

In a `block_actions` payload, Slack includes a `container` object:
```json
{
  "container": {
    "type": "message",
    "message_ts": "...",
    "channel_id": "...",
    "thread_ts": "...",      // present only if the button is inside a thread
    "is_ephemeral": false
  }
}
```

### 4.2 Current extraction in `prompt.rs`

The code does NOT directly access `container.thread_ts`. Instead it reads from the `message` object:

**`prompt.rs:126-131`:**
```rust
let thread_ts_opt = message.map(|m| {
    m.origin
        .thread_ts
        .as_ref()
        .map_or_else(|| m.origin.ts.0.clone(), |ts| ts.0.clone())
});
```

- `message.origin.thread_ts` is `Some(ts)` when the button message is **inside** a thread (the Slack API includes `thread_ts` on the message when it is a threaded reply).
- `message.origin.thread_ts` is `None` when the button message is at the **channel root** (not in a thread).

This means **thread detection is already implicitly implemented**: if `message.origin.thread_ts.is_some()`, the button was clicked inside a thread.

### 4.3 How `slack-morphism` models this

The `SlackHistoryMessage` type (from `slack_morphism`) includes:
- `message.origin.ts: SlackTs` — the message's own timestamp.
- `message.origin.thread_ts: Option<SlackTs>` — present when the message is part of a thread.

There is no separate `container` struct parsed in the current codebase; thread context is inferred from `message.origin.thread_ts`.

### 4.4 `is_thread_context` detection pattern

```rust
let is_thread_context = message
    .and_then(|m| m.origin.thread_ts.as_ref())
    .is_some();
```

Or equivalently (as done in `prompt.rs:126-131`), the `map_or_else` pattern picks `thread_ts.0` when present and `origin.ts.0` when absent—serving dual purpose as both detection and key selection.

---

## 5. `AppState` Fields Involved

**`mcp/handler.rs:111-160` (approximate):**

```rust
pub struct AppState {
    pub config: Arc<GlobalConfig>,
    pub db: Arc<SqlitePool>,
    pub slack: Option<Arc<SlackService>>,
    pub pending_approvals: PendingApprovals,
    pub pending_prompts: PendingPrompts,       // Arc<Mutex<HashMap<String, oneshot::Sender<PromptResponse>>>>
    pub pending_waits: PendingWaits,
    pub pending_thread_replies: PendingThreadReplies,  // Arc<Mutex<HashMap<String, (session_id, user_id, Sender<String>)>>>
    pub pending_modal_contexts: PendingModalContexts,
    // ...
}
```

The `pending_thread_replies` map is the critical shared state.

---

## 6. Gap Analysis — @-Mention Thread Reply Variant

### What already works

| Feature | Status | Location |
|---------|--------|----------|
| Thread-reply fallback registration | ✅ Complete | `thread_reply.rs:78-99` |
| Routing plain-text thread replies to oneshot | ✅ Complete | `push_events.rs:117-152` |
| Authorization check in reply routing | ✅ Complete | `thread_reply.rs:147-156` |
| Thread context detection from `message.origin.thread_ts` | ✅ Complete | `prompt.rs:126-131` |
| Fallback fallback_text posted to thread | ✅ Complete | `thread_reply.rs:267-291` |
| Timeout and cleanup on session termination | ✅ Complete | `thread_reply.rs:296-315`, `cleanup_session_fallbacks` |

### What is missing for the @-mention variant

| Gap | Description |
|-----|-------------|
| **G-1: AppMention does not check pending_thread_replies** | `push_events.rs:44-58` sends all app mentions to `ingest_app_mention` (steering), never to `route_thread_reply`. If an operator replies with `@agent-intercom refine the search logic`, the text goes to steering, not to the pending refine fallback. |
| **G-2: text stripping** | When routing via AppMention, the `<@UBOT_ID>` prefix must be stripped before delivering to the `resolve` callback. The `strip_mention` function in `steer.rs:259-267` already does this but is not called in the AppMention→thread-reply path. |
| **G-3: Fallback message wording** | The current fallback text says "please reply in this thread with your revised instructions." The @-mention variant needs to say "tagging `@agent-intercom` to submit." |
| **G-4: No `thread_ts` passed for AppMention** | `ingest_app_mention` at `steer.rs:249` calls `store_from_slack(&stripped, Some(channel_id), None, state)` — the `None` means no thread_ts disambiguation. For the @-mention path in a thread, we need `thread_ts` to look up the correct pending entry. The `AppMention` event at `push_events.rs:52` already captures `mention.origin.thread_ts` but does not pass it downstream. |

---

## 7. Implementation Sketch

### Option A — Intercept in AppMention handler (preferred)

Modify `push_events.rs` to check `pending_thread_replies` **before** steering when an `AppMention` arrives inside a thread.

**File: `src/slack/push_events.rs`**

```rust
SlackEventCallbackBody::AppMention(mention) => {
    let user_id = mention.user.to_string();
    if !is_authorized(&user_id, &app) { return Ok(()); }

    let channel_id = mention.channel.to_string();
    let thread_ts = mention.origin.thread_ts.clone();
    let text = mention.content.text.as_deref().unwrap_or_default();

    // NEW: If the mention is inside a thread and there is a pending
    // thread-reply fallback for this (channel, thread), route it there
    // instead of steering. Strip the @-mention prefix first.
    if let Some(ref ts) = thread_ts {
        let stripped = crate::slack::handlers::steer::strip_mention(text).trim().to_owned();
        if !stripped.is_empty() {
            match crate::slack::handlers::thread_reply::route_thread_reply(
                &channel_id,
                ts.0.as_str(),
                &user_id,
                &stripped,
                Arc::clone(&app.pending_thread_replies),
            ).await {
                Ok(true) => {
                    info!(user_id, channel_id, "app mention in thread captured by fallback");
                    post_ack(&app, &channel_id, Some(ts)).await;
                    return Ok(());
                }
                Ok(false) => { /* no pending fallback, fall through to steering */ }
                Err(err) => {
                    warn!(%err, "app mention: thread-reply routing error");
                    return Ok(());
                }
            }
        }
    }

    // Existing path: treat as steering.
    handlers::steer::ingest_app_mention(text, &channel_id, &app).await;
    post_ack(&app, &channel_id, thread_ts.as_ref()).await;
}
```

> **Note:** `strip_mention` is currently a private function in `steer.rs`. It must be made `pub(crate)` to be callable here, or the stripping logic can be inlined.

**File: `src/slack/handlers/steer.rs`**

```rust
// Change line 259: fn strip_mention → pub(crate) fn strip_mention
pub(crate) fn strip_mention(text: &str) -> &str {
```

### Option B — Proactive, always-use-thread-reply (skipping modal attempt)

When `is_thread_context` is detected **before** calling `open_modal`, skip the modal entirely and go straight to `activate_thread_reply_fallback`. This avoids the round-trip failure.

**File: `src/slack/handlers/prompt.rs`** — in the `prompt_refine` branch:

```rust
} else if action_id == "prompt_refine" {
    if let Some(ref slack) = state.slack {
        let callback_id = format!("prompt_refine:{prompt_id}");

        let thread_ts_opt = message.map(|m| {
            m.origin.thread_ts.as_ref()
                .map_or_else(|| m.origin.ts.0.clone(), |ts| ts.0.clone())
        });
        let chan_id_opt = channel.map(|c| c.id.to_string());
        let is_thread_context = message.and_then(|m| m.origin.thread_ts.as_ref()).is_some();

        // NEW: if button was clicked inside a thread, skip modal and go
        // straight to thread-reply fallback (@-mention variant).
        if is_thread_context {
            if let (Some(thread_ts), Some(chan_id)) = (thread_ts_opt, chan_id_opt) {
                let button_msg_ts = message.map(|m| m.origin.ts.clone());
                let state_clone = Arc::clone(state);
                let prompt_id_owned = prompt_id.to_owned();
                activate_thread_reply_fallback(
                    chan_id.as_str(),
                    thread_ts.as_str(),
                    prompt_session_id.clone(),
                    user_id.to_owned(),
                    "✏️ Please reply in this thread with your refinement instructions, tagging `@agent-intercom` to submit.",
                    button_msg_ts,
                    slack,
                    Arc::clone(&state.pending_thread_replies),
                    prompt_id,
                    move |reply_text| async move { /* same resolve callback */ },
                ).await?;
                return Ok(());
            }
        }

        // Original modal path (non-thread context).
        // ... existing modal open code ...
    }
}
```

### Recommended Approach

**Combine both options:**

1. **Option B** (proactive): Detect `is_thread_context` in `prompt.rs` and skip `open_modal` entirely, using a better fallback message with `@agent-intercom` tagging instruction.
2. **Option A** (intercept): Modify `push_events.rs` AppMention handler to check `pending_thread_replies` before steering. This ensures @-mentions in threads route correctly.

This combination means:
- In thread context: Refine button → immediately posts "@-mention" prompt, no failed modal attempt.
- AppMention in that thread: stripped text → `route_thread_reply` → resolves oneshot → agent gets refined instruction.
- AppMention **not** in a thread-reply context (or no pending fallback): falls through to steering as before.

### Concrete file changes required

| File | Change | Lines affected |
|------|--------|---------------|
| `src/slack/handlers/steer.rs` | Make `strip_mention` `pub(crate)` | line 259 |
| `src/slack/handlers/prompt.rs` | Add `is_thread_context` detection; call fallback directly instead of `open_modal` when true | lines 95-184 |
| `src/slack/push_events.rs` | In `AppMention` arm, check `pending_thread_replies` before steering; pass stripped text | lines 44-58 |
| `src/slack/handlers/wait.rs` | Same pattern as `prompt.rs` for `wait_resume_instruct` (optional, same issue) | lines 86-160 |

### Zero-risk properties of this change

- **No new state**: uses `pending_thread_replies` already in `AppState`.
- **Fallback to existing routing**: if `route_thread_reply` returns `Ok(false)` (no pending entry), AppMention falls through to steering unchanged.
- **Auth preserved**: `route_thread_reply` already enforces `authorized_user_id` match.
- **Duplicate guard preserved**: `register_thread_reply_fallback` LC-04 guard unchanged.
- **Timeout preserved**: 300s waiter task unchanged.
- **No new Slack API calls**: `ingest_app_mention` already works; we just intercept before it.

---

## 8. Appendix — Key Symbol Index

| Symbol | File | Line |
|--------|------|------|
| `handle_prompt_action` | `src/slack/handlers/prompt.rs` | 40 |
| `activate_thread_reply_fallback` | `src/slack/handlers/thread_reply.rs` | 223 |
| `register_thread_reply_fallback` | `src/slack/handlers/thread_reply.rs` | 78 |
| `route_thread_reply` | `src/slack/handlers/thread_reply.rs` | 125 |
| `cleanup_session_fallbacks` | `src/slack/handlers/thread_reply.rs` | 184 |
| `fallback_map_key` | `src/slack/handlers/thread_reply.rs` | 59 |
| `PendingThreadReplies` type | `src/slack/handlers/thread_reply.rs` | 48 |
| `FALLBACK_REPLY_TIMEOUT` | `src/slack/handlers/thread_reply.rs` | 39 |
| `handle_push_event` | `src/slack/push_events.rs` | 28 |
| `ingest_app_mention` | `src/slack/handlers/steer.rs` | 240 |
| `strip_mention` | `src/slack/handlers/steer.rs` | 259 |
| `handle_interaction` | `src/slack/events.rs` | 92 |
| `handle_view_submission` | `src/slack/handlers/modal.rs` | 35 |
| `instruction_modal` | `src/slack/blocks.rs` | 292 |
| `AppState.pending_thread_replies` | `src/mcp/handler.rs` | ~103 |
| `forward_prompt::handle` | `src/mcp/tools/forward_prompt.rs` | 47 |
