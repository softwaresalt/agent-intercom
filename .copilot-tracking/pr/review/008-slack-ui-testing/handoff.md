<!-- markdownlint-disable-file -->
# PR Review Handoff: 008-slack-ui-testing

## PR Overview

Adds US17 text-only thread prompts (forward_prompt, wait_for_instruction, ask_approval) and
@-mention thread-reply routing for operator decisions without Block Kit buttons. Also introduces
the F-16/F-17 thread-reply fallback mechanism for when Slack modals cannot be opened, and a
comprehensive Slack UI test suite (live Tier 2 + visual Playwright Tier 3). Post-adversarial
remediation commits (8a14dc9) fixed FR-001 through FR-004; this review covers new work from
b5a3f15 through ee04791.

* Branch: `008-slack-ui-testing`
* Base Branch: `main`
* Total Files Changed: 9 production source files
* Total Review Comments: 6 (2 HIGH, 3 MEDIUM, 1 LOW)

---

## PR Comments Ready for Submission

### File: `src/mcp/tools/forward_prompt.rs`

#### Comment 1 — RI-001 (Lines 98–105)

* Category: Functional Correctness / ACP Multi-Session
* Severity: ⚠️ HIGH

`ask_approval` (lines 130–149) correctly checks `context.service.session_id_override()` to
pin the tool call to the right ACP session when `?session_id=<id>` is present in the MCP URL.
`forward_prompt` skips this and always calls `session_repo.list_active().next()`, which
silently routes to the first session in the DB. In multi-session ACP mode this means prompts
land in the wrong session.

Apply the same guard pattern used in `ask_approval`:

**Suggested Change**

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
        rmcp::ErrorData::internal_error(
            format!("failed to query active sessions: {err}"), None)
    })?;
    sessions
        .into_iter()
        .next()
        .ok_or_else(|| rmcp::ErrorData::internal_error("no active session found", None))?
};
```

Apply the same change to `src/mcp/tools/wait_for_instruction.rs` lines 98–105.

---

### File: `src/mcp/tools/forward_prompt.rs`

#### Comment 2 — RI-002 (Lines 207–213)

* Category: Security / Session Isolation
* Severity: ⚠️ HIGH

The `authorized_user` passed to `register_thread_reply_fallback` is taken from
`state.config.authorized_user_ids.first()` — the first entry in the global allowlist. In a
multi-user or multi-session ACP environment this allows the first configured user to approve
thread prompts belonging to any session, including sessions they don't own. The resolved
`session` is already in scope and carries `owner_user_id`, which is the correct binding.

**Suggested Change**

```rust
// Before (lines 207–213):
let authorized_user = state
    .config
    .authorized_user_ids
    .first()
    .cloned()
    .unwrap_or_default();

// After:
let authorized_user = session.owner_user_id.clone();
```

Apply the same change to `src/mcp/tools/wait_for_instruction.rs` lines 177–183, and to
`src/mcp/tools/ask_approval.rs` lines 391–396 which has the same pattern.

---

### File: `src/mcp/tools/wait_for_instruction.rs`

#### Comment 3 — RI-003 (Lines 268–284)

* Category: Reliability / Thread Routing
* Severity: MEDIUM

When `wait_for_instruction` times out it posts the notification with `thread_ts: None`,
routing it to the channel root even when the session lives in a thread. `forward_prompt`'s
timeout path (lines 293–295) correctly uses `thread_ts: session_thread_ts.clone()`. The
inconsistency causes wait-timeout noise at channel level while all other session messages
appear in the thread.

**Suggested Change**

```rust
// Line 281 — change thread_ts: None to:
thread_ts: session_thread_ts.clone(),
```

`session_thread_ts` is already in scope at line 109.

---

### File: `src/slack/blocks.rs` + `src/mcp/tools/ask_approval.rs`

#### Comment 4 — RI-004 (blocks.rs lines 603–640, ask_approval.rs lines 222–239)

* Category: Reliability / Slack Message Limits
* Severity: MEDIUM

`build_text_only_approval` embeds the raw `diff` string directly into a triple-backtick block
with no length check (line 631). Slack enforces a 4,000-character per-block limit; a large
diff will be silently truncated or the API call will fail entirely, leaving the operator with
a garbled or missing approval request. The main-channel path already solves this via
`upload_file` when `diff_line_count > INLINE_DIFF_THRESHOLD`; apply the same approach to
the threaded path.

**Suggested Change — `src/slack/blocks.rs`**

Change `diff: &str` to `diff: Option<&str>` in `build_text_only_approval`. When `None`,
render a placeholder instead of the inline block:

```rust
pub fn build_text_only_approval(
    title: &str,
    diff: Option<&str>,   // None = diff uploaded as thread attachment
    file_path: &str,
    risk_level: &RiskLevel,
    description: Option<&str>,
) -> String {
    // ... existing header/description/file_path lines unchanged ...

    match diff {
        Some(d) => parts.push(format!("```\n{d}\n```")),
        None => parts.push(
            "_Diff uploaded as a file attachment in this thread._".to_owned()
        ),
    }

    // ... existing @-mention footer unchanged ...
}
```

**Suggested Change — `src/mcp/tools/ask_approval.rs` (threaded path, replace lines 222–239)**

```rust
if is_threaded {
    let diff_line_count = input.diff.lines().count();
    let inline_diff = (diff_line_count <= blocks::INLINE_DIFF_THRESHOLD)
        .then_some(input.diff.as_str());

    let text_body = blocks::build_text_only_approval(
        &input.title,
        inline_diff,
        &input.file_path,
        &input.risk_level,
        input.description.as_deref(),
    );
    let msg = SlackMessage {
        channel: channel.clone(),
        text: Some(text_body),
        blocks: None,
        thread_ts: session_thread_ts.clone(),
    };
    if let Err(err) = slack.enqueue(msg).await {
        warn!(%err, "failed to enqueue text-only approval message");
    }

    // Mirror main-channel path: upload large diffs as a file snippet
    // pinned to the session thread (upload_file already accepts thread_ts).
    if diff_line_count > blocks::INLINE_DIFF_THRESHOLD {
        let upload_span =
            info_span!("slack_upload_diff_thread", request_id = %request_id);
        async {
            let sanitized = input.file_path.replace(['/', '.'], "_");
            let filename = format!("{sanitized}.diff.txt");
            if let Err(err) = slack
                .upload_file(
                    channel.clone(),
                    &filename,
                    &input.diff,
                    session_thread_ts.clone(),
                    Some("text"),
                )
                .await
            {
                warn!(%err, "failed to upload diff snippet to thread");
            }
        }
        .instrument(upload_span)
        .await;
    }
}
```

---

### File: `src/slack/handlers/thread_reply.rs`

#### Comment 5 — RI-005 (Lines 86–107, 231–326)

* Category: Reliability / Duplicate Messages
* Severity: MEDIUM

When `activate_thread_reply_fallback` is called for a key that already has a pending entry,
the LC-04 guard in `register_thread_reply_fallback` silently drops the new `tx` and returns
`()`. `activate_thread_reply_fallback` has no way to detect this and proceeds to post a
"please reply in this thread" message to Slack and spawn a waiter task that immediately exits.
The operator sees a duplicate fallback prompt.

**Suggested Change**

Change `register_thread_reply_fallback` to return `bool` indicating whether the entry was
actually registered:

```rust
pub async fn register_thread_reply_fallback(
    channel_id: &str,
    thread_ts: String,
    session_id: String,
    authorized_user_id: String,
    tx: oneshot::Sender<String>,
    pending: PendingThreadReplies,
) -> bool {   // ← was ()
    let key = fallback_map_key(channel_id, &thread_ts);
    let mut guard = pending.lock().await;
    if guard.contains_key(&key) {
        warn!(
            channel_id, thread_ts,
            "thread-reply fallback: duplicate registration — dropping new sender (LC-04)"
        );
        return false;  // tx dropped here
    }
    guard.insert(key, (session_id, authorized_user_id, tx));
    true
}
```

Then short-circuit in `activate_thread_reply_fallback` immediately after the registration
call:

```rust
let registered = register_thread_reply_fallback(
    chan_id, thread_ts.to_owned(), session_id,
    authorized_user_id, tx, Arc::clone(&pending),
).await;
if !registered {
    // LC-04 duplicate guard fired — original entry is still waiting.
    // Skip posting and spawning to avoid a duplicate operator message.
    return Ok(());
}
// ... rest of function unchanged ...
```

Note: Clippy's `#[must_use]` will now flag call sites that ignore the return value, which is
desirable — callers in `prompt.rs`, `wait.rs`, and `approval.rs` that call `activate_*`
already propagate the `Result`, so no changes are needed there.

---

### File: `src/slack/commands.rs`

#### Comment 6 — RI-006 (Line 118)

* Category: API Surface / Conventions
* Severity: 💡 LOW

`dispatch_command` was widened to `pub` to support integration test access from `tests/`.
Add a doc comment acknowledging this so future maintainers understand the visibility is
intentional and not part of the stable API surface:

**Suggested Change**

```rust
/// Dispatch a parsed command to the correct handler.
///
/// Routes `command` (the word after `/intercom`) to the appropriate sub-handler,
/// passing `args`, the acting `user_id`, and the originating `channel_id`.
///
/// `pub` visibility is intentional — required for integration test access from
/// `tests/`. Not part of the stable public API; prefer `pub(crate)` once the
/// test harness can be restructured to avoid direct invocation.
///
/// # Errors
///
/// Returns `AppError` if the underlying sub-handler fails (e.g., database error,
/// missing session, path validation failure). Mode-mismatch responses are returned
/// as `Ok(String)` rather than as errors.
pub async fn dispatch_command(
```

---

## Review Summary by Category

| Category | Count |
|---|---:|
| ⚠️ Security / Session Isolation | 1 |
| ⚠️ Functional Correctness | 1 |
| Reliability | 3 |
| 💡 API Surface / Conventions | 1 |
| **Total** | **6** |

## Instruction Compliance

* ✅ `AGENTS.md` — Security Boundary Enforcement (Principle IV): RI-002 addresses session
  owner binding for thread-reply authorization.
* ✅ `AGENTS.md` — Structured Observability (Principle V): all suggested changes preserve
  existing `info_span!` / `warn!` tracing patterns.
* ✅ `AGENTS.md` — MCP Protocol Fidelity (Principle II): RI-001 ensures tool calls route to
  the correct session per the `session_id_override` contract.
* ⚠️ `AGENTS.md` — Principle I (Safety-First): RI-001 and RI-002 were not present in the
  prior adversarial review; they surfaced from new ACP multi-session commits post-remediation.

## Outstanding Strategic Recommendations

* **FR-006 (deferred)** — Live tests assert before cleanup; failures can strand Slack test
  messages. Consider introducing `defer`-style cleanup guards in a follow-up pass.
* **FR-007 (deferred)** — `Control+K` quick-switch in `slack-nav.ts` is not portable to macOS.
  Low priority while execution environment is Windows-only.
* **RI-002 scope** — The same `authorized_user_ids.first()` pattern appears in
  `ask_approval.rs` lines 391–396. That call site was not in this PR's diff but should be
  included in the same fix commit for consistency.
