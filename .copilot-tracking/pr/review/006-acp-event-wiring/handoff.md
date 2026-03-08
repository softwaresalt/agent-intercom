# PR Review Handoff: 006-acp-event-wiring

**Branch**: `006-acp-event-wiring`  
**Base**: `main`  
**Completed**: 2026-03-08

## Summary

The `006-acp-event-wiring` feature wires ACP agent events (`ClearanceRequested`,
`PromptForwarded`) through the Slack approval pipeline. The PR review identified 3 findings;
2 were applied and 1 was deferred.

## Quality Gate Results (Post-Review)

| Gate | Result |
|------|--------|
| `cargo fmt --all -- --check` | ✅ Clean |
| `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ 0 warnings |
| `cargo test` | ✅ 996 passed, 0 failed |
| Working tree | ✅ Clean |

## Review Findings

| ID | Severity | Status | Description |
|----|----------|--------|-------------|
| RI-001 | Medium | ✅ Applied | ACP handler now calls `upload_file` when diff ≥ 20 lines (mirrors MCP path) |
| RI-002 | Low | ✅ Applied | Empty description guard — `""` mapped to `None` before `build_approval_blocks` |
| RI-003 | Low | ⏭ Deferred | Unescaped title/description in Slack mrkdwn — low risk, follow-up recommendation |

## Files Modified in PR Review Fixes

| File | Change |
|------|--------|
| `src/main.rs` | RI-001: `upload_file` call; RI-002: empty description guard; `cargo fmt` applied |

## Key Architecture Points

- `handle_clearance_requested` and `handle_prompt_forwarded` are private async fns in `src/main.rs`
- Both follow: DB persist → driver register → Slack post
- `INLINE_DIFF_THRESHOLD = 20` in `blocks.rs` controls upload-vs-inline decision
- `set_thread_ts` uses `WHERE thread_ts IS NULL` for idempotent concurrent first-write
- D2: `post_message_direct` when `thread_ts=None`, `enqueue` when `thread_ts=Some`

## Open Follow-ups (Not Blocking PR)

1. **RI-003** — Add `slack_escape()` helper and apply to `title`/`description` in `build_approval_blocks`.
   Touches both MCP and ACP paths; should be a separate PR.
2. **AcpDriver resource leak** — `pending_clearances` / `pending_prompts_acp` HashMaps leak entries
   when a session terminates without completing all pending requests. `deregister_session` does not
   clean them up. Low risk in practice (bounded by session count).

## Commit Sequence

| Commit | Description |
|--------|-------------|
| `5c34d89` | Phase 1: setup baseline (919 tests) |
| `3b886ff` | Phase 2: shared block builders in `blocks.rs` |
| `00b611f` | Phase 3: `handle_clearance_requested` wired (US1) |
| `cf79ce1` | Phase 4: `handle_prompt_forwarded` wired (US2) |
| `4c4ee93` | Phase 5: integration tests S036–S041 (US3 thread continuity) |
| `3114d21` | Phase 6: concurrent/lifecycle integration tests |
| `5a8427a` | Phase 6: tracking files |
| `ba13287` | Phase 6: tracking commit |
| `ca27d23` | chore: backlog update |
| *(pending)* | fix: PR review fixes — diff upload and empty description guard |
