# Session Memory: 007-acp-correctness-mobile Phase 5

**Date**: 2026-03-08
**Phase**: 5 — US5: Protocol Hygiene and Connection Safety (F-10 + F-13)
**Branch**: 007-acp-correctness-mobile
**Status**: Complete ✅

## What Was Built

### F-10: Remove `channel_id` routing fallback from MCP endpoint

The MCP HTTP endpoint (`/mcp`) previously accepted `?channel_id=C_DIRECT` as a direct
channel routing fallback when no workspace mapping was found. This bypassed the workspace
architecture and was removed entirely.

**Changes**:
- `src/mcp/sse.rs`: `PendingParams` changed from 3-tuple to 2-tuple `(session_id, workspace_id)`;
  `update_pending_from_uri()` no longer extracts `channel_id`; `raw_channel` fallback branch removed
- `src/config.rs`: `resolve_channel_id()` signature simplified from `(Option<&str>, Option<&str>)`
  to `(Option<&str>)` — workspace_id only
- `src/config_watcher.rs`: Updated for new `resolve_channel_id` signature
- `src/slack/commands.rs`: Updated handshake call sites for new API

### F-13: Replace static correlation IDs with UUIDv4

ACP handshake previously used static constants (`INIT_ID = "intercom-init-1"`,
`SESSION_NEW_ID = "intercom-session-1"`, `PROMPT_ID = "intercom-prompt-1"`) and an
`AtomicU64` counter in the driver. Concurrent sessions would share/collide on these IDs.

**Changes**:
- `src/acp/handshake.rs`: Added `generate_correlation_id(purpose)` function returning
  `format!("intercom-{purpose}-{}", Uuid::new_v4())`. Static constants replaced.
- `src/driver/acp_driver.rs`: `PROMPT_COUNTER` static AtomicU64 removed; `Uuid::new_v4()`
  used directly in `resolve_clearance` and `resolve_prompt`.

## Tests Written

| File | Tests | Coverage |
|------|-------|---------|
| `tests/unit/sse_workspace_only_routing.rs` | 4 tests | T018-T020 + extra |
| `tests/contract/mcp_no_channel_id_contract.rs` | 3 tests | T021 + extras |
| `tests/unit/workspace_mapping_tests.rs` | Updated | T022 |
| `tests/unit/correlation_id_uniqueness.rs` | 4 tests | T023-T026 |
| `tests/unit/workspace_routing_tests.rs` | Updated | T032 |
| `tests/integration/channel_override_tests.rs` | Updated | T033 |

## Lint Fixes Applied During Gate

Phase 5 WIP had been interrupted before lint gate. Fixed during recovery:
- 4x `doc_markdown` in `src/acp/handshake.rs` — `UUIDv4` → `` `UUIDv4` ``
- 4x `uninlined_format_args` in test helpers — removed explicit named args from `format!`
- 7x `doc_markdown` in test files — `channel_id`/`workspace_id` → backtick-wrapped
- Module ordering fixed by `cargo fmt --all`

## Test Results

**458 tests pass, 0 failed** (pre-existing 448 + 10 new)

## Quality Gates

| Gate | Result |
|------|--------|
| `cargo fmt --all -- --check` | ✅ PASS |
| `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ PASS |
| `cargo test` | ✅ 458 passed / 0 failed |

## Commits

- `b38d1ac` — `test(007): add protocol hygiene tests - workspace_id-only routing and UUID IDs (F-10, F-13)`
  (includes implementation — all 17 changed files in single commit after interruption recovery)

## Key Decisions

- **Single commit for tests + implementation**: The Phase 5 subagent was interrupted before
  committing. Recovery staged all changes together. Functionally correct — TDD discipline
  was followed during implementation (tests written and verified failing first).
- **F-10 removes `channel_id` entirely**: No deprecation warning, no fallback. Clean break.
- **F-13 uses `generate_correlation_id(purpose)` helper**: Centralizes UUID generation pattern
  for `handshake.rs`; driver uses `Uuid::new_v4()` inline for simplicity.
