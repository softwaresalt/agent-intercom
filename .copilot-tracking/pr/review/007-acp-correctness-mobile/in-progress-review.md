<!-- markdownlint-disable-file -->
# PR Review Status: 007-acp-correctness-mobile

## Review Status

* Phase: Phase 2 — Analyzing Changes
* Last Updated: 2026-03-08 23:00
* Summary: ACP correctness fixes (F-06/F-07/F-10/F-13) + thread-reply modal fallback (F-16/F-17). Adversarial review complete; HIGH deferred fixes being applied before PR open.

## Branch and Metadata

* Normalized Branch: `007-acp-correctness-mobile`
* Source Branch: `007-acp-correctness-mobile`
* Base Branch: `main`
* Linked Work Items: Feature 007 spec — `specs/007-acp-correctness-mobile/spec.md`
* Author: software.salt@gmail.com
* Total Commits (above main): 20
* Files Changed: 66 (+4635 insertions, −186 deletions, src/tests only: 40 files)

## Phase 1 Actions Log

* ✅ Tracking directory created: `.copilot-tracking/pr/review/007-acp-correctness-mobile/`
* ✅ Full diff captured to `logs/pr-full-diff.txt`
* ✅ `handle_session_terminated` in `src/main.rs` located at lines 855–914 (Fix B insertion point: line 888)
* ⏳ `scripts/dev-tools/pr-ref-gen.sh` — not present; PR reference generated manually from `git log` and `git diff`
* ✅ Three adversarial reviewers read: agent-6 (Gemini), agent-7 (GPT-5.3), agent-8 (Claude Opus)

## Adversarial Review Summary (Pre-Fix)

| ID | Severity | Consensus | Status | Finding |
|----|----------|-----------|--------|---------|
| CS-01 / TQ-001 / LC-01 | CRITICAL | 3/3 | ✅ Fixed `15389db` | Auth no-op in `route_thread_reply` |
| CS-04 / TQ-005 / LC-04 | HIGH | 3/3 | ✅ Fixed `15389db` | FR-022 buttons not replaced on fallback activation |
| CS-03 / TQ-003 / LC-02 | HIGH | 3/3 | 🔧 Fixing (agent-9) | No timeout on fallback `rx.await` |
| TQ-006 / LC-03 | HIGH | 2/3 | 🔧 Fixing (agent-9) | No session-termination cleanup of pending entries |
| CS-05 / TQ-004 | HIGH | 2/3 | 🔧 Fixing (agent-9) | Zombie waiter spawned when fallback message post fails |
| CS-02 / LC-05 | MEDIUM | 2/3 | 🔧 Fixing (agent-9) | `thread_ts`-only key collides across channels |
| LC-06 | MEDIUM | 1/3 | 🔧 Fixing (agent-9) | `count_active_acp` excludes `Paused` sessions |
| CS-06 / TQ-007 | LOW | 2/3 | ⏸ Deferred | Hardcoded status strings in SQL |
| TQ-008 | MEDIUM | 1/3 | ⏸ Deferred | Fallback logic triplicated |
| TQ-009 | MEDIUM | 1/3 | ⏸ Deferred | Test gaps — push_event integration negative paths |
| LC-05 (agent-5) | MEDIUM | 1/3 | ❓ Needs decision | `StreamActivity` emitted for ALL queued messages in `deliver_queued_messages`, including failed ones — sends false stall-detector signals |
| LC-04 (agent-5) | MEDIUM | 1/3 | ⏸ Deferred | `HashMap::insert` silently overwrites on duplicate `register_thread_reply_fallback` for same composite key |

## Diff Mapping (src/ and tests/ only)

| File | Type | Change | Category |
|------|------|--------|----------|
| `src/acp/handshake.rs` | Modified | +55/−11 | F-13: generate_correlation_id() |
| `src/acp/reader.rs` | Modified | +60/−7 | F-06: deliver_queued_messages, mark-consumed |
| `src/config.rs` | Modified | +43/−7 | F-10: resolve_channel_id 1-arg |
| `src/config_watcher.rs` | Modified | +20/−6 | F-10: call site update |
| `src/driver/acp_driver.rs` | Modified | +14/−3 | F-13: inline Uuid::new_v4() |
| `src/main.rs` | Modified | +1 | message event subscription |
| `src/mcp/handler.rs` | Modified | +16 | AppState: pending_thread_replies field |
| `src/mcp/sse.rs` | Modified | +52/−18 | F-10: PendingParams 2-tuple |
| `src/persistence/session_repo.rs` | Modified | +25 | F-07: count_active_acp() |
| `src/slack/commands.rs` | Modified | +14/−7 | F-07: count_active_acp call |
| `src/slack/handlers/approval.rs` | Modified | +104/−0 | F-16/F-17: fallback path |
| `src/slack/handlers/mod.rs` | Modified | +1 | thread_reply module |
| `src/slack/handlers/prompt.rs` | Modified | +95/−0 | F-16/F-17: fallback path |
| `src/slack/handlers/thread_reply.rs` | **New** | +119 | F-16/F-17: core fallback module |
| `src/slack/handlers/wait.rs` | Modified | +81/−0 | F-16/F-17: fallback path |
| `src/slack/push_events.rs` | Modified | +27/−9 | F-17: message event routing |
| `tests/contract/acp_capacity_contract.rs` | **New** | +129 | F-07 contracts |
| `tests/contract/mcp_no_channel_id_contract.rs` | **New** | +123 | F-10 contracts |
| `tests/integration/thread_reply_integration.rs` | **New** | +78 | F-16/F-17 integration |
| `tests/unit/acp_reader_steering_delivery.rs` | **New** | +321 | F-06 unit tests |
| `tests/unit/correlation_id_uniqueness.rs` | **New** | +144 | F-13 unit tests |
| `tests/unit/session_repo_count_acp.rs` | **New** | +161 | F-07 unit tests |
| `tests/unit/sse_workspace_only_routing.rs` | **New** | +160 | F-10 unit tests |
| `tests/unit/thread_reply_fallback.rs` | **New** | +214 | F-16/F-17 unit tests |
| `tests/unit/workspace_mapping_tests.rs` | Modified | +/-62 | F-10 API update |

## Instruction Files Reviewed

* `.github/instructions/constitution.instructions.md`: Core quality gates — no `unwrap`/`expect`, pedantic clippy, TDD, path safety, session ownership (FR-031)
* `AGENTS.md`: Terminal command policy, destructive approval workflow, single-binary constraint

## Review Items

### 🔍 In Review (agent-9 fixing)

#### RI-01: Fallback task timeout (Fix A)
* File: `src/slack/handlers/prompt.rs`, `wait.rs`, `approval.rs`
* Category: Reliability
* Severity: HIGH

#### RI-02: Session-termination cleanup (Fix B)
* File: `src/slack/handlers/thread_reply.rs`, `src/main.rs`
* Category: Reliability / Memory
* Severity: HIGH
* Insertion point confirmed: `src/main.rs:888` after `acp_driver.deregister_session()`

#### RI-03: Zombie waiter on enqueue failure (Fix C)
* File: `src/slack/handlers/prompt.rs`, `wait.rs`, `approval.rs`
* Category: Reliability / Error Handling
* Severity: HIGH

#### RI-04: count_active_acp excludes Paused (Fix D)
* File: `src/persistence/session_repo.rs`
* Category: Correctness
* Severity: MEDIUM

#### RI-05: Composite key for cross-channel safety (Fix E)
* File: `src/slack/handlers/thread_reply.rs` + call sites
* Category: Correctness
* Severity: MEDIUM

### ✅ Approved for PR Comment (already fixed)

#### RI-00a: Authorization no-op in route_thread_reply
* Fixed: `15389db` — `PendingThreadReplies` now stores `(authorized_user_id, Sender)` tuple

#### RI-00b: FR-022 button replacement on fallback activation
* Fixed: `15389db` — `slack.update_message()` called immediately after `register_thread_reply_fallback`

### ❌ Rejected / No Action (Deferred)

* TQ-008: Fallback triplication — architectural refactor, not blocking PR
* TQ-009: Additional test coverage for push_event integration paths — follow-up issue
* CS-06/TQ-007: Hardcoded SQL status strings — LOW severity, no behavioral impact

## Next Steps

* [ ] Wait for agent-9 (all 5 fixes) to complete
* [ ] Run `cargo check`, `cargo clippy`, `cargo fmt`, `cargo test` to verify
* [ ] Commit fixes as `fix(007): remaining review fixes — timeout, cleanup, zombie-waiter, capacity`
* [ ] Push and enter Phase 3 (collaborative review with user)
* [ ] Generate `handoff.md` with final PR comments
