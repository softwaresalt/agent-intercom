# Phase 6 Memory: 007-acp-correctness-mobile — US4 Mobile Operator Approval Workflow

**Date**: 2026-03-08  
**Branch**: 007-acp-correctness-mobile  
**Phase**: 6 (F-16/F-17: Thread-Reply Fallback for Modal Failures)  
**Tasks**: T039–T051 ✅ All complete

---

## What Was Implemented

### Feature: Thread-Reply Fallback (F-16, F-17)

When Slack modals cannot be opened (e.g. `trigger_id` expiry in Socket Mode, which
affects Slack iOS), all three interactive flows now fall back to thread-reply collection:

- **Prompt Refine** (`handle_prompt_action` → `prompt_refine` branch)
- **Wait Resume with Instructions** (`handle_wait_action` → `wait_resume_instruct` branch)  
- **Approval Rejection Reason** (`handle_approval_action` → `approve_reject` branch)

### TDD Order

Tests (T039–T044) were written first and confirmed to fail (compilation errors) before
implementation (T045–T050). The TDD gate was satisfied.

---

## Files Created

| File | Purpose |
|------|---------|
| `src/slack/handlers/thread_reply.rs` | Core module: `register_thread_reply_fallback` and `route_thread_reply` |
| `tests/unit/thread_reply_fallback.rs` | 5 unit tests (S029–S033) |
| `tests/integration/thread_reply_integration.rs` | 1 integration test (full fallback flow) |

---

## Files Modified

| File | Change |
|------|--------|
| `src/mcp/handler.rs` | Added `PendingThreadReplies` type alias + `pending_thread_replies` field to `AppState` |
| `src/main.rs` | Added `pending_thread_replies: Arc::default()` to `AppState` construction |
| `src/slack/handlers/mod.rs` | Added `pub mod thread_reply` |
| `src/slack/handlers/prompt.rs` | Added F-16/F-17 fallback to `prompt_refine` branch |
| `src/slack/handlers/wait.rs` | Added F-16/F-17 fallback to `wait_resume_instruct` branch |
| `src/slack/handlers/approval.rs` | Added F-16/F-17 fallback to `approve_reject` branch |
| `src/slack/push_events.rs` | Added fallback check before steering in `Message` event handler |
| `tests/unit.rs` | Added `mod thread_reply_fallback` |
| `tests/integration.rs` | Added `mod thread_reply_integration` |
| 11 integration test files | Added `pending_thread_replies: Arc::default()` to `AppState` constructions |

---

## Key Design Decisions

### `PendingThreadReplies` Type

```rust
pub type PendingThreadReplies = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;
// Keyed by thread_ts; value is oneshot sender for operator reply text
```

Used `Arc<Mutex<HashMap<...>>>` (not `DashMap`) since `dashmap` is not in `Cargo.toml`.

### Authorization in push_events.rs

`route_thread_reply` takes an explicit `authorized_user_id`. In `push_events.rs`, the
sender is already verified via `is_authorized()` before reaching the fallback check, so
`&user_str` is passed for both sender and authorized user. This means any authorized
Slack operator can complete a fallback interaction.

### Test for S033 (unauthorized rejection)

Unit test uses a separate sender and authorized user to verify unauthorized replies are
silently ignored and the pending entry is preserved.

### No `unwrap()` in production code

All production code uses `?`, `map_err(...)`, or explicit match arms. The `#[allow(clippy::too_many_lines)]` attribute was added to `handle_wait_action` (140 lines) and `handle_push_event` (114 lines).

---

## Test Results

```
test result: ok. 463 passed; 0 failed  (unit: 463, integration: 272 + 2 new)
cargo clippy --all-targets -- -D warnings -D clippy::pedantic  → PASS
cargo fmt --all -- --check                                      → PASS
```

**Delta**: +5 tests (458 → 463 unit tests passing)

---

## Commits

1. `61a5dde` — `test(007): add thread-reply fallback tests (F-16, F-17)` 
2. `4f003df` — `chore(007): mark T039-T051 complete in tasks.md (Phase 6)`

(Implementation was included in commit 1 as part of TDD verification cycle.)

---

## Architecture Pattern

```
Modal failure (trigger_id expired / Socket Mode / iOS)
    │
    ▼
register_thread_reply_fallback(thread_ts, tx, pending_thread_replies)
    │
    ├─► post fallback message in Slack thread ("please reply here")
    │
    └─► spawn tokio task waiting on rx.await
            │
            ▼ (operator replies in Slack thread)
        push_events::handle_push_event receives Message event
            │
            ├─► route_thread_reply checks pending_thread_replies
            │       • authorized? → remove entry, send through oneshot
            │       • unauthorized? → silently ignore, entry remains
            │       • no entry? → Ok(false), fall through to steering
            │
            └─► spawned task resumes with reply_text
                    ├─► update DB (PromptDecision / ApprovalStatus)
                    ├─► audit log (approval only)
                    └─► driver.resolve_*(reply_text) → unblocks waiting caller
```
