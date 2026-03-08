# Session Memory — 007-acp-correctness-mobile Adversarial Review

**Date**: 2026-03-08  
**Phase**: Final adversarial review + fix application  
**Branch**: `007-acp-correctness-mobile`  
**Final commit**: `15389db`

## What Was Done

Three adversarial code-review subagents (Gemini 3 Pro, GPT-5.3 Codex, Claude Opus 4.6)
were dispatched for independent review of all F-06/F-07/F-10/F-13/F-16/F-17 implementation
files. Results were synthesized via direct code inspection (read_agent not available in
this session due to context compaction across sessions).

### Unified Findings

| ID | Sev | File | Summary | Resolution |
|----|-----|------|---------|------------|
| 1 | HIGH | push_events.rs / thread_reply.rs | `authorized_user_id` no-op — sender passed as both sender AND authorized args; any globally-authorized user could complete any session's fallback | **FIXED** — map now stores (user_id, Sender) tuple; user captured at registration time |
| 2 | HIGH | prompt.rs / wait.rs / approval.rs | FR-022 button replacement missing on fallback activation — handlers returned Ok(()) before replacement code | **FIXED** — `slack.update_message()` with ⏳ status called immediately after `register_thread_reply_fallback` in all three handlers |
| 3 | HIGH | prompt.rs / wait.rs / approval.rs | No timeout on `tokio::spawn` fallback tasks — `rx.await` is unbounded | **DEFERRED** — TODO(F-20) comment added; requires new `GlobalConfig` field and architectural change |
| 4 | HIGH | AppState / thread_reply.rs | No cleanup of `pending_thread_replies` on session termination | **DEFERRED** — TODO(F-20) comment added; requires session lifecycle hooks and map restructure |
| 5 | MED | prompt.rs / wait.rs / approval.rs | Fallback ~80-line block triplicated across three handler files | Documented; future refactor candidate |
| 6 | MED | session_repo.rs | `count_active_acp()` hardcodes 'active'/'created' status strings | Documented; acceptable for now |
| 7 | LOW | wait.rs | `_status = "resumed"` for `wait_stop` action — unused misleading variable | Not fixed — unused variable, no behavioral impact |
| 8 | LOW | thread_reply.rs | Functions declared `pub` instead of `pub(crate)` | Not changed — external test crates require `pub` |

## Files Changed in Review Fix Commit (`15389db`)

| File | Change |
|------|--------|
| `src/slack/handlers/thread_reply.rs` | `PendingThreadReplies` type alias changed to `HashMap<String, (String, oneshot::Sender<String>)>`; `register_thread_reply_fallback` gains `authorized_user_id: String` param; `route_thread_reply` removes `authorized_user_id` param (now from map) |
| `src/mcp/handler.rs` | `PendingThreadReplies` type reference updated |
| `src/slack/handlers/prompt.rs` | Registration call site updated (+user_id); FR-022 `update_message` added on fallback activation; TODO(F-20) comments |
| `src/slack/handlers/wait.rs` | Same as prompt.rs |
| `src/slack/handlers/approval.rs` | Same as prompt.rs |
| `src/slack/push_events.rs` | `route_thread_reply` call updated (removed now-redundant authorized_user_id arg) |
| `tests/unit/thread_reply_fallback.rs` | Updated for new API signatures |
| `tests/integration/thread_reply_integration.rs` | Updated for new API signatures |

## Final Quality Gate Results

| Gate | Result |
|------|--------|
| `cargo check` | ✅ |
| `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ |
| `cargo fmt --all -- --check` | ✅ |
| `cargo test --all-targets` | ✅ 463 passed, 0 failed |

## Known Gaps (Tracked as TODO(F-20))

Both were identified and documented but NOT fixed in this session:

1. **No timeout on fallback tasks** — `tokio::spawn` tasks await `rx` indefinitely. In a long-running server, if the operator never replies, these tasks accumulate. Fix requires a `fallback_reply_timeout_seconds` config field (default 300s) and `tokio::time::timeout` wrapping `rx.await` in each handler.

2. **No cleanup on session termination** — `pending_thread_replies` entries are never removed when a session ends. Fix requires storing `session_id` alongside the sender in the map, and a cleanup call in the session-termination path (likely in `orchestrator/session_manager.rs`).

## Feature Complete Summary

| Phase | Focus | Tests Added | Commit |
|-------|-------|-------------|--------|
| 3 | F-06: Steering delivery reliability | 6 | 9c6c4bd |
| 4 | F-07: ACP capacity counting | 7 | 586c6a8 |
| 5 | F-10 + F-13: Protocol hygiene | 10 | b38d1ac |
| 6 | F-16/F-17: Thread-reply fallback | 6 | 4b2d072 |
| 7 | Polish | 0 | 2481b50 |
| Review | Auth fix + FR-022 | 0 (existing updated) | 15389db |

**Total**: 56 tasks, 14 feature commits, 463 tests, branch pushed.
