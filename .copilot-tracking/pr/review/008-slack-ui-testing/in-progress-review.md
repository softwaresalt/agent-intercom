<!-- markdownlint-disable-file -->
# PR Review Status: 008-slack-ui-testing

## Review Status

* Phase: 4 — Complete
* Last Updated: 2026-03-18
* Summary: US17 text-only thread prompts + @-mention routing + comprehensive Slack UI test suite

## Branch and Metadata

* Normalized Branch: `008-slack-ui-testing`
* Source Branch: `008-slack-ui-testing`
* Base Branch: `main`
* Prior Adversarial Review: `final-adversarial-review.md` (remediated through commit `8a14dc9`)
* New work since adversarial review: commits `b5a3f15` through `ee04791`

## Diff Mapping — Production Source Files

| File | Type | Change Summary |
|------|------|----------------|
| `src/mcp/tools/ask_approval.rs` | M | US17 thread approval, session_id_override, snippet support |
| `src/mcp/tools/forward_prompt.rs` | M | US17 thread prompt + thread-reply fallback |
| `src/mcp/tools/wait_for_instruction.rs` | M | US17 thread wait + thread-reply fallback |
| `src/slack/blocks.rs` | M | US17 text-only builders |
| `src/slack/commands.rs` | M | dispatch_command widened to pub (FR-005, deferred) |
| `src/slack/handlers/prompt.rs` | M | Proactive F-16/F-17 thread-context detection |
| `src/slack/handlers/thread_reply.rs` | M | activate_thread_reply_fallback, cleanup, parse_thread_decision |
| `src/slack/push_events.rs` | M | F-16/F-17 fallback routing for AppMention |
| `Cargo.toml` | M | live-slack-tests feature gate |

## Review Items

### 🔍 In Review

#### RI-001 — `forward_prompt` / `wait_for_instruction` ignore `session_id_override` — HIGH

* File: `src/mcp/tools/forward_prompt.rs`
* Lines: 98–105 (same pattern in `wait_for_instruction.rs` lines 98–105)
* Category: Functional Correctness / ACP Multi-Session
* Severity: HIGH
* User Decision: ✅ APPROVED

**Description**

`ask_approval.rs` correctly checks `context.service.session_id_override()` (lines 130–149) to
pin the tool call to the right ACP session when `?session_id=<id>` is present in the MCP URL.
`forward_prompt` and `wait_for_instruction` skip this check and always call
`session_repo.list_active().next()`, which returns whichever session is first in the DB — wrong
in any multi-session ACP scenario. The correct session silently loses the prompt/wait while
another session's flow is interrupted.

**Current Code** (`forward_prompt.rs` lines 98–105, identical in `wait_for_instruction.rs`)

```rust
let session_repo = SessionRepo::new(Arc::clone(&state.db));
let sessions = session_repo.list_active().await.map_err(|err| {
    rmcp::ErrorData::internal_error(format!("failed to query active sessions: {err}"), None)
})?;
let session = sessions
    .into_iter()
    .next()
    .ok_or_else(|| rmcp::ErrorData::internal_error("no active session found", None))?;
```

**Suggested Fix**

```rust
let session_repo = SessionRepo::new(Arc::clone(&state.db));
let session = if let Some(sid) = context.service.session_id_override() {
    session_repo
        .get_by_id(sid)
        .await
        .map_err(|err| rmcp::ErrorData::internal_error(
            format!("failed to query session: {err}"), None))?
        .ok_or_else(|| rmcp::ErrorData::internal_error("session not found", None))?
} else {
    let sessions = session_repo.list_active().await.map_err(|err| {
        rmcp::ErrorData::internal_error(format!("failed to query active sessions: {err}"), None)
    })?;
    sessions
        .into_iter()
        .next()
        .ok_or_else(|| rmcp::ErrorData::internal_error("no active session found", None))?
};
```

Apply the same pattern in `wait_for_instruction.rs`.

---

#### RI-002 — Thread-reply fallback uses `authorized_user_ids.first()` not `session.owner_user_id` — HIGH

* File: `src/mcp/tools/forward_prompt.rs`
* Lines: 207–213 (same pattern in `wait_for_instruction.rs` lines 177–183)
* Category: Security / Session Isolation
* Severity: HIGH
* User Decision: ✅ APPROVED

**Description**

The `authorized_user` passed to `register_thread_reply_fallback` comes from
`state.config.authorized_user_ids.first()` — the first entry in the global allowlist.
When multiple sessions are active (ACP mode), each session has its own `owner_user_id` field.
If `authorized_user_ids` contains more than one entry, the first user could approve thread
prompts belonging to a session they do not own. Should use `session.owner_user_id` to
bind each fallback to the session's actual owner.

**Current Code** (`forward_prompt.rs` lines 207–213)

```rust
let authorized_user = state
    .config
    .authorized_user_ids
    .first()
    .cloned()
    .unwrap_or_default();
```

**Suggested Fix**

```rust
let authorized_user = session.owner_user_id.clone();
```

Apply the same change in `wait_for_instruction.rs` lines 177–183.

---

#### RI-003 — `wait_for_instruction` timeout posts to channel root in threaded sessions — MEDIUM

* File: `src/mcp/tools/wait_for_instruction.rs`
* Lines: 268–284
* Category: Reliability / Thread Routing
* Severity: MEDIUM
* User Decision: ✅ APPROVED

**Description**

When `wait_for_instruction` times out it posts the notification with `thread_ts: None`, routing
it to the channel root even when the session lives in a thread. `forward_prompt.rs` handles
this correctly by using `thread_ts: session_thread_ts.clone()` in its timeout path
(lines 293–295). The inconsistency means wait-timeout noise appears at channel level while
all other session messages are in the thread.

**Current Code** (`wait_for_instruction.rs` lines 268–284)

```rust
let msg = SlackMessage {
    channel,
    text: Some(format!(...)),
    blocks: Some(vec![...]),
    thread_ts: None,   // ← always channel root
};
```

**Suggested Fix**

```rust
let msg = SlackMessage {
    channel,
    text: Some(format!(...)),
    blocks: Some(vec![...]),
    thread_ts: session_thread_ts.clone(),  // route to session thread
};
```

---

#### RI-004 — `build_text_only_approval` embeds full diff with no size gate — MEDIUM

* File: `src/slack/blocks.rs`
* Lines: 603–640
* Category: Reliability / Slack Message Limits
* Severity: MEDIUM
* User Decision: ✅ APPROVED (MODIFIED)

**Modification**: Use the same `INLINE_DIFF_THRESHOLD` + `upload_file(thread_ts)` approach as
the main-channel path rather than a character-count truncation. Change `diff: &str` to
`diff: Option<&str>` in `build_text_only_approval`; `None` renders a placeholder noting the
diff is attached. In `ask_approval.rs` threaded path, pass `Some(diff)` for short diffs and
`None` + `slack.upload_file(..., session_thread_ts, Some("text"))` for long ones.

**Description**

`build_text_only_approval` embeds the raw `diff` string directly in a triple-backtick block
with no length check (line 631: `` parts.push(format!("```\n{diff}\n```")); ``).
Slack enforces a 4,000-character per-block limit and a ~40 KB total message payload limit.
A large diff will be silently truncated or rejected. `build_text_only_prompt` does not
have this issue because it does not embed arbitrary external content. The main-channel
`ask_approval` path sidesteps this by uploading snippets, but the threaded path has no
equivalent guard.

**Suggested Fix**

Add a truncation guard mirroring `truncate_text` before embedding:

```rust
const MAX_DIFF_CHARS: usize = 3_000;

let diff_display = if diff.len() > MAX_DIFF_CHARS {
    format!(
        "{}\n… (diff truncated — {} chars total; review full diff in the commit)",
        &diff[..MAX_DIFF_CHARS],
        diff.len(),
    )
} else {
    diff.to_owned()
};
parts.push(format!("```\n{diff_display}\n```"));
```

---

#### RI-005 — `activate_thread_reply_fallback` posts message when LC-04 duplicate guard fires — MEDIUM

* File: `src/slack/handlers/thread_reply.rs`
* Lines: 246–326
* Category: Reliability / Duplicate Messages
* Severity: MEDIUM
* User Decision: ✅ APPROVED

**Description**

When a second caller invokes `activate_thread_reply_fallback` for an already-registered key,
the LC-04 guard in `register_thread_reply_fallback` (lines 96–107) silently drops the new `tx`,
returning `()`. `activate_thread_reply_fallback` has no way to detect this and proceeds to:
1. Post the fallback instruction message to Slack (a duplicate the operator sees)
2. Spawn a waiter that immediately exits with `Err(_)` (sender already dropped)

The spawned waiter logs a warning but the operator receives a second "please reply in this
thread" message, which is confusing.

**Suggested Fix** — return a bool from `register_thread_reply_fallback`:

```rust
// register_thread_reply_fallback returns true if registered, false if LC-04 guard fired
pub async fn register_thread_reply_fallback(...) -> bool {
    ...
    if guard.contains_key(&key) {
        warn!(...);
        return false;  // tx dropped here
    }
    guard.insert(key, (session_id, authorized_user_id, tx));
    true
}

// In activate_thread_reply_fallback:
let registered = register_thread_reply_fallback(...).await;
if !registered {
    // LC-04: original entry already waiting — skip duplicate post and waiter
    return Ok(());
}
```

This silently deduplicates without breaking any callers that already tolerate `false`.

---

#### RI-006 — `dispatch_command` widened to `pub` (deferred from FR-005) — LOW

* File: `src/slack/commands.rs`
* Lines: 118–124
* Category: API Surface / Conventions
* Severity: LOW
* User Decision: ✅ APPROVED (Option 1 — add doc comment noting intentional pub visibility for test access)

**Description**

`dispatch_command` was promoted to `pub` to allow external test access. The adversarial
review raised this as FR-005 (MEDIUM) and deferred it because tightening visibility would
require broader test architecture changes. The question for this review is whether to
document the intentional public visibility (e.g., `// pub for integration test access —
not part of the stable API`), accept it as-is, or track a follow-up ticket.

**Options**

1. Add a doc comment acknowledging the visibility decision (minimal effort)
2. Leave as-is — visibility is public, no comment needed
3. Open a follow-up issue to refactor tests toward `pub(crate)` or a test-only feature gate

### ✅ Approved for PR Comment

### ❌ Rejected / No Action

## Next Steps

* [ ] Present RI-001 sequentially, capture decision, move to approved/rejected
* [ ] Repeat for RI-002 through RI-006
* [ ] Generate handoff.md after all decisions
