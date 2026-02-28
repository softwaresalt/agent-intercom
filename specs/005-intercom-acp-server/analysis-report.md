# Adversarial Analysis Report: Intercom ACP Server

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28
**Method**: Multi-model adversarial review (3 independent reviewers)
**Models**: Claude Opus 4.6, GPT-5.2 Codex, Gemini 3 Pro Preview

## Executive Summary

| Severity | Count | Action |
|----------|-------|--------|
| CRITICAL | 5 | Must fix before implementation |
| HIGH | 8 | Should fix before implementation |
| MEDIUM | 10 | Fix during implementation |
| LOW | 4 | Track, fix opportunistically |
| **Total** | **27** | |

**Raw findings**: 38 across 3 reviewers → 27 unique after deduplication (11 duplicates merged).

**Reviewer agreement**: 3 findings confirmed by 2+ reviewers (F-001, F-019 confirmed by 2 each). Remaining 24 unique to a single reviewer — no conflicts between reviewers.

---

## CRITICAL Findings (5)

### F-001 — thread_ts immutability contradicts Slack thread overflow edge case
**Category**: Contradiction | **Sources**: LC-001, TF-009 | **Confirmed by**: 2 reviewers

**Problem**: spec.md Edge Case 6 says "updates the session's thread_ts" when a Slack thread exceeds its reply limit. data-model.md says "thread_ts is immutable once set." S042 tests immutability. These are mutually exclusive — implementation cannot satisfy both.

**Recommendation**: Keep thread_ts immutable (simpler, safer). Revise Edge Case 6: post continuation as a new top-level message with a link back to the original thread, but do NOT update thread_ts. Also note per F-026 that Slack provides no API signal for thread limits, making this edge case unimplementable as specified regardless.

**Status**: ✅ Resolved — spec.md Edge Case 6 revised to keep thread_ts immutable. data-model.md validation rules unchanged.

---

### F-002 — prompt/response outbound message missing from data-model.md
**Category**: Gap | **Source**: LC-002

**Problem**: data-model.md Outbound AcpMessage table lists 4 methods but contracts/acp-stream.md defines 5 (missing `prompt/response`). The `AgentDriver.resolve_prompt()` method produces this message but has no data model entry. No scenario tests ACP prompt/response serialization. No task explicitly implements it.

**Recommendation**: Add `prompt/response` row to data-model.md: `method='prompt/response', fields='id: String, decision: String, instruction: Option<String>'`. Add scenario S073 for ACP prompt resolution. Verify T083 covers prompt/response. Also add scenarios for `resolve_wait` in ACP mode (per LC-009).

**Status**: ✅ Resolved — prompt/response added to data-model.md. S073 and S074 added to SCENARIOS.md. Contract test #9 and #10 added to agent-driver.md.

---

### F-003 — ACP session channel_id resolution pathway undefined
**Category**: Gap | **Source**: LC-003

**Problem**: All workspace-to-channel mapping FRs (FR-010 through FR-014) describe resolution via URL query parameters on the MCP connection endpoint. ACP sessions spawned via `/intercom session-start` have no inbound URL. No FR specifies that channel_id comes from the originating Slack channel. ACP sessions will get channel_id=NULL and all Slack routing breaks.

**Recommendation**: Add FR-027: "In ACP mode, the session's channel_id MUST be derived from the Slack channel where the `/intercom session-start` command was issued." Update data-model.md validation rules. Update T037/T038 to pass channel_id from Slack command context.

**Status**: ✅ Resolved — FR-027 added to spec.md. data-model.md validation rules updated. T038 updated to pass channel_id from Slack context.

---

### F-004 — AcpDriver is single-stream but spec requires concurrent ACP sessions
**Category**: Concurrency | **Source**: TF-001

**Problem**: AcpDriver is defined as holding a single `mpsc::Sender<Value>` for outbound writes. FR-025 requires concurrent sessions across multiple workspaces. With a single driver instance in AppState, `send_prompt`/`interrupt` cannot route to the correct session's stream.

**Recommendation**: AcpDriver must manage `HashMap<String, mpsc::Sender<Value>>` (session_id → writer channel) or AppState must hold per-session driver instances. Update contracts/agent-driver.md ACP behavior table. Update T082 to reflect the session-indexed design.

**Status**: ✅ Resolved — agent-driver.md AcpDriver redesigned with session-indexed HashMap. T082 updated. register_session/deregister_session lifecycle methods added.

---

### F-005 — Credential leakage to untrusted agent process
**Category**: Security | **Source**: ES-001

**Problem**: `tokio::process::Command` inherits the parent's environment by default. The server's environment contains SLACK_BOT_TOKEN and SLACK_APP_TOKEN. The spawned agent process gains full Slack API access, violating the principle of least privilege and Constitution Principle IV (Security Boundary Enforcement).

**Recommendation**: Use `Command::env_clear()` in the ACP spawner and explicitly allowlist only safe variables (PATH, HOME, RUST_LOG, etc.). Add FR-028: "Spawned agent processes MUST NOT inherit the server's credential environment variables." Add scenario S074 and a task in Phase 5.

**Status**: ✅ Resolved — FR-028 added to spec.md. S075 added to SCENARIOS.md. T037 updated with env_clear(). T037b test added.

---

## HIGH Findings (8)

### F-006 — S055 EOF vs S022 clean exit assign contradictory session states
**Category**: Contradiction | **Source**: LC-004

S055 marks session "interrupted" on EOF. S022 marks "terminated" on exit code 0. For stdio agents, these are the same event. **Fix**: EOF triggers process wait; final state depends on exit code. Update S055.

**Status**: ✅ Resolved — S055 updated to depend on exit code.

### F-007 — S047 most-recent-session fallback has no FR
**Category**: Gap | **Source**: LC-005

FR-017 requires channel+thread matching. S047's "most recent" heuristic isn't codified. **Fix**: Add FR-017b or require thread context for multi-session disambiguation.

**Status**: ✅ Resolved — FR-029 added to spec.md.

### F-008 — Stalled session status not in data model
**Category**: Gap | **Source**: LC-006

S061 references "stalled" state not in SessionStatus enum. **Fix**: Add connectivity_status field (online/offline/stalled) separate from lifecycle status.

**Status**: ✅ Resolved — connectivity_status field added to Session in data-model.md and schema migration.

### F-009 — ctl binary changes in plan but no tasks implement them
**Category**: Gap | **Source**: LC-007

plan.md lists ctl subcommands and IPC extensions with zero tasks. **Fix**: Add tasks or remove from plan if deferred.

**Status**: ✅ Resolved — ctl/ipc references removed from plan.md (marked "unchanged, deferred to future feature"). tasks.md notes deferred.

### F-010 — FR-003 says outbound connection but architecture is local process spawning
**Category**: Ambiguity | **Source**: LC-008

FR-003 uses network terminology. Contract names channel `tcp_tx`. Implementation is stdio. **Fix**: Reword FR-003. Rename tcp_tx → stream_tx.

**Status**: ✅ Resolved — FR-003 reworded in spec.md. tcp_tx renamed to stream_writers in agent-driver.md.

### F-011 — Request/prompt IDs not session-scoped
**Category**: API | **Source**: TF-002

Two sessions can generate `req-001`, causing misrouting. **Fix**: Namespace IDs with session_id. Key pending maps by (session_id, request_id).

**Status**: ✅ Resolved — AcpDriver pending maps keyed by (session_id, id) in agent-driver.md. Error cases updated.

### F-012 — Command injection via session prompt
**Category**: Security | **Source**: ES-002

If prompt is passed as CLI arg to host_cli, injection is possible. **Fix**: Mandate prompt delivery via ACP stream only. Add to FR.

**Status**: ✅ Resolved — FR-030 added to spec.md mandating prompt via stream only.

### F-013 — Cross-session interference by authorized users
**Category**: Authorization | **Source**: ES-003

No requirement that clearance actor matches session owner_user_id. **Fix**: Add owner verification to all session-modifying driver actions.

**Status**: ✅ Resolved — FR-031 added to spec.md. S076 added to SCENARIOS.md. Owner verification section added to agent-driver.md. T068b/T068c tasks added.

---

## MEDIUM Findings (10)

| ID | Title | Recommendation | Status |
|----|-------|---------------|--------|
| F-014 | plan.md mentions workspace_mapping table but data model says not persisted | Remove table reference from plan.md | ✅ Fixed |
| F-015 | tokio-util codec feature not enabled | Add Phase 1 task to enable codec feature | ✅ T004b added |
| F-016 | Heartbeat steering response undefined | Define explicit behavior or remove hint | ✅ acp-stream.md updated |
| F-017 | Feature 004 dependency not formally declared | Add prerequisite to spec.md | ✅ Added to Assumptions |
| F-018 | Session restart semantics unclear | Clarify new session inherits thread_ts; add restart_of field | ✅ restart_of field added to data-model |
| F-019 | Process tree kill fails on Windows | Use Windows Job Objects (2 reviewers agreed) | ⚠️ Tracked for implementation |
| F-020 | clearance/response schema conflicts between contract and data-model | Normalize to envelope id approach | ✅ Both normalized |
| F-021 | framing=lsp config option with no codec planned | Remove lsp option from contract | ✅ Removed from acp-stream.md |
| F-022 | No ACP stream rate limiting | Add token-bucket rate limiter | ⚠️ Tracked for implementation |
| F-023 | Stall timers lost on server restart | Persist last_activity_at in session table | ✅ last_activity_at added to schema |

---

## LOW Findings (4)

| ID | Title | Recommendation | Status |
|----|-------|---------------|--------|
| F-024 | AgentEvent.description required but existing model nullable | Make Optional<String> | ✅ Fixed in data-model |
| F-025 | Phase 6/7 parallel claim conflicts with shared test file | Split test files | ⚠️ Tracked for implementation |
| F-026 | Slack thread reply limit not detectable via API | Remove or make manual | ✅ Edge case revised in spec |
| F-027 | Startup race: event before session DB commit | Commit session before starting reader | ⚠️ Tracked for implementation |

---

## Remediation Priority

### Must Fix Before Implementation (CRITICAL) — ✅ ALL RESOLVED
1. F-001 → ✅ spec.md edge case 6 revised, thread_ts immutability preserved
2. F-002 → ✅ data-model.md + S073/S074 + contract tests 9/10 added
3. F-003 → ✅ FR-027 added, data-model.md + T038 updated
4. F-004 → ✅ AcpDriver redesigned with session-indexed HashMap
5. F-005 → ✅ FR-028 added, S075 added, T037/T037b updated

### Should Fix Before Implementation (HIGH) — ✅ ALL RESOLVED
6. F-006 → ✅ S055 updated (EOF → exit code dependent)
7. F-007 → ✅ FR-029 added (most-recent fallback with hint)
8. F-008 → ✅ connectivity_status field added to data-model
9. F-009 → ✅ ctl deferred in plan.md and tasks.md
10. F-010 → ✅ FR-003 reworded, tcp_tx → stream_writers
11. F-011 → ✅ Session-scoped IDs in AcpDriver pending maps
12. F-012 → ✅ FR-030 added (prompt via stream only)
13. F-013 → ✅ FR-031 + S076 + T068b/T068c + owner verification contract

### Fix During Implementation (MEDIUM/LOW) — 8/14 RESOLVED, 6 TRACKED
14-27. 8 resolved in spec artifacts. 4 tracked for implementation phase (F-019 Windows Job Objects, F-022 rate limiting, F-025 test file split, F-027 startup race).

---

## Review Methodology

Three independent reviewers analyzed all spec artifacts with different focus areas:

| Reviewer | Model | Focus | Raw Findings |
|----------|-------|-------|-------------|
| A | Claude Opus 4.6 | Logical consistency | 16 |
| B | GPT-5.2 Codex | Technical feasibility | 12 |
| C | Gemini 3 Pro Preview | Edge cases & security | 10 |

Findings were deduplicated by matching on affected artifacts and root cause. When reviewers disagreed on severity, the higher severity was used. No reviewer conflicts were found — all findings were either unique or in agreement.
