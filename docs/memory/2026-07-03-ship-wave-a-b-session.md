# Ship Session Memory — Wave A (F.4) + Wave B (F.2)

**Date**: 2026-07-03  
**Session type**: Ship execution  
**Shipment**: 001-S (partial — Waves A and B only; F.3 and F.5 gated for later)

---

## Items Completed

### Wave A — 013.002-F: .intercom numbered-queue slash command
| Task | Status | Commit |
|---|---|---|
| 013.002.001-T | done | 866a5b0f |
| 013.002.002-T | done | 866a5b0f |
| 013.002.003-T | done | 866a5b0f |
| 013.002.004-T | done | 866a5b0f |
| 013.002-F (feature) | done | 866a5b0f |

**Branch**: `feat/013.002-intercom-queue`  
**PR**: https://github.com/softwaresalt/agent-intercom/pull/18  
**Head SHA**: 6dae961 (includes backlog archive commit)  
**Reviewed HEAD**: 866a5b0f  
**Review outcome**: READY_WITH_FOLLOWUPS  
**Tests**: 635 pass (624 baseline + 11 new)

### Wave B — 013.001-F: ACP correctness fixes
| Task | Status | Commit | Notes |
|---|---|---|---|
| 013.001.001-T | done | ccc1043b | Was already on main (prior session) |
| 013.001.002-T | done | ccc1043b | Was already on main (prior session) |
| 013.001.003-T | done | ccc1043b | Was already on main (prior session) |
| 013.001.004-T | done | ccc1043b | Proactive is_thread_context added to wait.rs |
| 013.001-F (feature) | done | ccc1043b | |

**Branch**: `feat/013.001-acp-correctness`  
**PR**: https://github.com/softwaresalt/agent-intercom/pull/19  
**Head SHA**: c58adb4 (includes backlog archive commit)  
**Reviewed HEAD**: ccc1043b  
**Review outcome**: READY_WITH_FOLLOWUPS  
**Tests**: 626 pass (624 baseline + 2 new)

---

## Branch States

- `feat/013.002-intercom-queue` → pushed, PR #18 open, Copilot review requested
- `feat/013.001-acp-correctness` → pushed, PR #19 open, Copilot review requested
- `main` → at 835228c (unchanged)

---

## Key Decisions

1. **Wave A all 4 tasks in one commit**: Tasks 001-T through 004-T were implemented and delivered together (tight interdependency; persistence repo is prerequisite for all commands). Single conventional commit with `Closes` trailers for all four.

2. **Wave B — T1/T2/T3 already done**: Confirmed by code evidence (`src/acp/reader.rs` F-06 fix comment, `session_repo.rs` `count_active_acp()`, `src/acp/handshake.rs` UUID-based IDs). The wave only needed T4 new code.

3. **T4 gap confirmed**: `wait.rs` had reactive fallback but missing proactive `is_thread_context` detection. `prompt.rs` had the proactive check; mirrored it to `wait.rs`.

4. **Helper function pattern**: Added `pub fn message_is_in_thread(thread_ts: Option<&str>) -> bool` to `thread_reply.rs` instead of inlining in both `prompt.rs` and `wait.rs`. Enables direct unit testing of the detection predicate.

5. **Review P2 fix applied inline**: Changed `thread_ts_opt` in `wait.rs` proactive block to use `map_or_else(|| m.origin.ts.0.clone(), |ts| ts.0.clone())` pattern matching `prompt.rs`.

---

## Follow-up Items (stash candidates)

1. **Queue robustness** (from Wave A P1 review findings):
   - Add per-path Mutex to `IntercomQueueRepo` to prevent concurrent add/replace/remove races
   - Improve `transfer` partial-failure error message (distinguish "transferred but not removed" case)

2. **Queue transfer test coverage** (Wave A P2):
   - Add `queue_transfer` command tests: happy path (backlogit on PATH), `backlogit` not found, nonexistent item number

3. **Wave A list guard** (already applied as P2 fix): Changed `args.len() != 1` → `args.len() > 1`

4. **Queue save atomicity** (Wave A P3): Replace `fs::write()` with write-to-temp + rename for crash safety

5. **handler-level test for wait.rs proactive path** (Wave B P1): Testing `handle_wait_action` with `wait_resume_instruct` + threaded message requires Slack client mock infrastructure. Current tests only cover the detection helper predicate.

6. **Backlogit archive diffs on feature branch**: Both Wave A and Wave B feature branches contain `.backlogit/archive/` files. These should be reviewed before merge to ensure archive state is consistent with main.

---

## Pending Merge Approvals

**STOP — DO NOT MERGE**

Both PRs require operator approval per repo ruleset. Copilot review requested on both.

- PR #18: https://github.com/softwaresalt/agent-intercom/pull/18 — Wave A queue feature
- PR #19: https://github.com/softwaresalt/agent-intercom/pull/19 — Wave B ACP fixes

---

## Scope Preserved

- F.3 (`013.003-F`) — NOT touched (gated, later)
- F.5 (`013.005-F`) — NOT touched (ADR-0016 conformance migration, gated)
- MCP/rmcp — NOT removed or changed
- Wire protocol — NOT changed (F.2 fixes behavior bugs only)
