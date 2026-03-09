<!-- markdownlint-disable-file -->
# PR Review Status: 007-acp-correctness-mobile

## Review Status

* Phase: Phase 4 — Handoff Complete
* Last Updated: 2026-03-08 23:30
* Summary: All CRITICAL and HIGH findings fixed across two fix passes. 466 tests pass. Branch pushed at `a764786`. Ready for PR.

## Branch and Metadata

* Normalized Branch: `007-acp-correctness-mobile`
* Source Branch: `007-acp-correctness-mobile`
* Base Branch: `main`
* Linked Work Items: Feature 007 spec — `specs/007-acp-correctness-mobile/spec.md`
* Author: software.salt@gmail.com
* Total Commits (above main): 21
* Files Changed: 66 (+4635 insertions, −186 deletions, src/tests only: 40 files)
* Final commit: `a764786`

## Phase 1 Actions Log

* ✅ Tracking directory created: `.copilot-tracking/pr/review/007-acp-correctness-mobile/`
* ✅ Full diff captured to `logs/pr-full-diff.txt`
* ✅ `handle_session_terminated` in `src/main.rs` located at lines 855–914 (Fix B insertion point: line 888)
* ⏳ `scripts/dev-tools/pr-ref-gen.sh` — not present; PR reference generated manually from `git log` and `git diff`
* ✅ Six adversarial reviewers read: agent-3/5 (first pass), agent-6/7/8 (second pass, post-fixes), agent-4

## Adversarial Review Summary

| ID | Severity | Consensus | Status | Finding |
|----|----------|-----------|--------|---------|
| CS-01 / TQ-001 / LC-01 | CRITICAL | 3/3 | ✅ Fixed `15389db` | Auth no-op in `route_thread_reply` |
| CS-04 / TQ-005 / LC-04 | HIGH | 3/3 | ✅ Fixed `15389db` | FR-022 buttons not replaced on fallback activation |
| LC-02 / TQ-002 (timeout) | HIGH | 3/3 | ✅ Fixed `a764786` | No timeout on fallback `rx.await` |
| LC-03 / TQ-002 (cleanup) | HIGH | 2/3 | ✅ Fixed `a764786` | No session-termination cleanup of pending entries |
| CS-05 / zombie-waiter | HIGH | 2/3 | ✅ Fixed `a764786` | Zombie waiter spawned when fallback message post fails |
| TQ-004 (misrouting) | HIGH | 1/3 | ✅ Fixed `a764786` | Err from route_thread_reply falls through to steering |
| CS-02 / LC-05 (composite key) | MEDIUM | 2/3 | ✅ Fixed `a764786` | `thread_ts`-only key collides across channels |
| LC-06 (count_active_acp) | MEDIUM | 1/3 | ✅ Fixed `a764786` | `count_active_acp` excludes `Paused` sessions |
| LC-07 (duplicate type alias) | LOW | 1/3 | ✅ Fixed `a764786` | PendingThreadReplies 3-tuple sync in handler.rs |
| CS-06 / TQ-007 | LOW | 2/3 | ⏸ Deferred | Hardcoded status strings in SQL |
| TQ-008 | MEDIUM | 1/3 | ⏸ Deferred | Fallback logic triplicated across 3 handlers |
| TQ-009 | MEDIUM | 1/3 | ⏸ Deferred | Missing push_event integration negative-path tests |
| LC-05 (agent-5 StreamActivity) | MEDIUM | 1/3 | ⏸ Deferred | StreamActivity emitted for failed deliveries |
| LC-04 (agent-5 overwrite) | MEDIUM | 1/3 | ⏸ Deferred | HashMap::insert silently overwrites on duplicate key |

## Diff Mapping (src/ and tests/ only)

| File | Type | Change | Category |
|------|------|--------|----------|
| `src/acp/handshake.rs` | Modified | +55/−11 | F-13: generate_correlation_id() |
| `src/acp/reader.rs` | Modified | +60/−7 | F-06: deliver_queued_messages, mark-consumed |
| `src/config.rs` | Modified | +43/−7 | F-10: resolve_channel_id 1-arg |
| `src/config_watcher.rs` | Modified | +20/−6 | F-10: call site update |
| `src/driver/acp_driver.rs` | Modified | +14/−3 | F-13: inline Uuid::new_v4() |
| `src/main.rs` | Modified | +9 | message event subscription + cleanup_session_fallbacks |
| `src/mcp/handler.rs` | Modified | +17 | AppState: pending_thread_replies 3-tuple |
| `src/mcp/sse.rs` | Modified | +52/−18 | F-10: PendingParams 2-tuple |
| `src/persistence/session_repo.rs` | Modified | +40 | F-07: count_active_acp() incl. paused |
| `src/slack/commands.rs` | Modified | +14/−7 | F-07: count_active_acp call |
| `src/slack/handlers/approval.rs` | Modified | +158/−0 | F-16/F-17: fallback path w/ timeout+zombie guard |
| `src/slack/handlers/mod.rs` | Modified | +1 | thread_reply module |
| `src/slack/handlers/prompt.rs` | Modified | +151/−0 | F-16/F-17: fallback path w/ timeout+zombie guard |
| `src/slack/handlers/thread_reply.rs` | **New** | +170 | F-16/F-17: composite key, 3-tuple, timeout const, cleanup |
| `src/slack/handlers/wait.rs` | Modified | +127/−0 | F-16/F-17: fallback path w/ timeout+zombie guard |
| `src/slack/push_events.rs` | Modified | +38/−9 | F-17: routing + TQ-004 Err early-return |
| `tests/contract/acp_capacity_contract.rs` | **New** | +129 | F-07 contracts |
| `tests/contract/mcp_no_channel_id_contract.rs` | **New** | +123 | F-10 contracts |
| `tests/integration/thread_reply_integration.rs` | **New** | +94 | F-16/F-17 integration |
| `tests/unit/acp_reader_steering_delivery.rs` | **New** | +321 | F-06 unit tests |
| `tests/unit/correlation_id_uniqueness.rs` | **New** | +144 | F-13 unit tests |
| `tests/unit/session_repo_count_acp.rs` | **New** | +203 | F-07 unit tests incl. paused |
| `tests/unit/sse_workspace_only_routing.rs` | **New** | +160 | F-10 unit tests |
| `tests/unit/thread_reply_fallback.rs` | **New** | +408 | F-16/F-17 unit tests incl. timeout+cleanup |
| `tests/unit/workspace_mapping_tests.rs` | Modified | +/-62 | F-10 API update |

## Review Items

### ✅ Approved for PR Comment (all fixed)

#### RI-00a: Authorization no-op in route_thread_reply
* Fixed: `15389db` — `PendingThreadReplies` stores `(session_id, authorized_user_id, Sender)`; `route_thread_reply` reads owner from map, not caller parameter

#### RI-00b: FR-022 button replacement on fallback activation
* Fixed: `15389db` — `slack.update_message()` called immediately after `register_thread_reply_fallback`

#### RI-01: Fallback task timeout (Fix A)
* Fixed: `a764786` — `tokio::time::timeout(FALLBACK_REPLY_TIMEOUT=300s, rx)` in all three handlers

#### RI-02: Session-termination cleanup (Fix B)
* Fixed: `a764786` — `cleanup_session_fallbacks(session_id, &state.pending_thread_replies)` in `handle_session_terminated`

#### RI-03: Zombie waiter on enqueue failure (Fix C)
* Fixed: `a764786` — pending map entry removed and Err returned if `slack.enqueue()` fails; waiter not spawned

#### RI-04: count_active_acp excludes Paused (Fix D)
* Fixed: `a764786` — SQL now: `status IN ('active', 'created', 'paused')`

#### RI-05: Composite key for cross-channel safety (Fix E)
* Fixed: `a764786` — `fallback_map_key("{channel_id}\x1f{thread_ts}")`; `PendingThreadReplies` is 3-tuple

#### RI-06: Err from route_thread_reply falls through to steering (TQ-004)
* Fixed: `a764786` — `Err` arm returns `Ok(())` early; operator text not injected as steering command

### ❌ Deferred (document in PR description)

* TQ-008: Fallback triplication — architectural refactor, follow-up ticket
* TQ-009: Additional push_event integration tests for negative paths — follow-up ticket
* CS-06/TQ-007: Hardcoded SQL status strings — LOW severity, no behavioral impact
* LC-05 (StreamActivity for failed deliveries): MEDIUM, follow-up ticket
* LC-04 (silent overwrite on duplicate registration): MEDIUM, follow-up ticket

## Quality Gates

* ✅ `cargo check` — clean
* ✅ `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` — clean
* ✅ `cargo fmt --all -- --check` — clean
* ✅ `cargo test --all-targets` — 466 passed, 0 failed

## Next Steps

* [x] Fix all CRITICAL and HIGH review findings
* [x] Push branch at `a764786`
* [ ] Open PR: `007-acp-correctness-mobile` → `main`
* [ ] Post PR comments from `handoff.md`
