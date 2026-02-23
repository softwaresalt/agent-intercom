# Phase 1 Memory — Setup: Test Module Registration (001-002-integration-test)

**Date**: 2026-02-22
**Phase**: 1 of 5 (Setup)
**Tasks**: T001–T003
**Commit**: 12c6749

## Task Overview

Phase 1 goal: register three new integration test modules in the test harness entry
point (`tests/integration.rs`) so that Phase 2–4 can populate them with test functions.

## What Was Done

### T001 — Register `policy_watcher_tests`

- Created stub file `tests/integration/policy_watcher_tests.rs` with doc header
  covering FR-007 scenarios (S045–S050)
- Registered `mod policy_watcher_tests;` in `tests/integration.rs`

### T002 — Register `ipc_server_tests`

- Created stub file `tests/integration/ipc_server_tests.rs` with doc header
  covering FR-008 scenarios (S053–S064)
- Registered `mod ipc_server_tests;` in `tests/integration.rs`

### T003 — Register `mcp_dispatch_tests`

- Created stub file `tests/integration/mcp_dispatch_tests.rs` with doc header
  covering FR-001 scenarios (S001–S007)
- Registered `mod mcp_dispatch_tests;` in `tests/integration.rs`

## Files Modified

| File | Change |
|------|--------|
| `tests/integration.rs` | Added 3 new `mod` declarations after `stall_escalation_tests` |
| `tests/integration/policy_watcher_tests.rs` | Created — empty stub with doc header (FR-007) |
| `tests/integration/ipc_server_tests.rs` | Created — empty stub with doc header (FR-008) |
| `tests/integration/mcp_dispatch_tests.rs` | Created — empty stub with doc header (FR-001) |
| `specs/001-002-integration-test/tasks.md` | Marked T001, T002, T003 as `[X]` complete |

## Quality Gates

- `cargo check --test integration`: PASS (empty modules compile)
- `cargo fmt --all -- --check`: PASS
- `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`: PASS (one fix: backticks on tool names in doc comments)
- `cargo test`: 464 tests pass (140 unit + 138 contract + 185 integration + 1 doc-test), 0 failed

## Important Discoveries

- Clippy pedantic (`doc_markdown` lint) requires backtick wrapping of snake_case identifiers
  in doc comments even in test stub files. Fixed `set_operational_mode` and `recover_state`
  in `mcp_dispatch_tests.rs`.
- The existing integration test suite has 185 tests across 23 modules (not 150+ as noted in the spec).
- Module registration order in `tests/integration.rs` must be alphabetical to match coding style
  (new modules inserted alphabetically: `ipc_server_tests`, `mcp_dispatch_tests`, `policy_watcher_tests`).

## Next Steps

**Phase 2** (US6 — Policy Hot-Reload):
- Populate `tests/integration/policy_watcher_tests.rs` with 6 test functions (T004–T009)
- Key public APIs: `PolicyWatcher::register()`, `PolicyWatcher::unregister()`, `PolicyWatcher::get_policy()`
- Use `tempfile::tempdir()` for workspace isolation; poll with 50 ms interval / 2 s timeout for async hot-reload
- Run `cargo test --test integration policy_watcher_tests` after writing tests

**Phase 3** (US7 — IPC Server):
- Populate `tests/integration/ipc_server_tests.rs` with 8 test functions (T012–T019)
- Key risk: IPC test helpers need unique pipe names to avoid cross-test conflicts

**Phase 4** (US1 — MCP Dispatch):
- Populate `tests/integration/mcp_dispatch_tests.rs` with 5 test functions (T022–T026)
- Key risk: rmcp `RequestContext` has no public constructor — tests must go through SSE transport layer

## Context to Preserve

**Spec location**: `specs/001-002-integration-test/`
**Key spec files**: `plan.md` (constitution table), `research.md` (gap analysis), `SCENARIOS.md` (behavioral matrix)
**Test entry point**: `tests/integration.rs`
**Test helpers**: `tests/integration/test_helpers.rs` — shared infrastructure for all integration tests
**Policy module**: `src/policy/` — `watcher.rs`, `evaluator.rs`, `loader.rs`
**IPC module**: `src/ipc/` — `server.rs`, `socket.rs`
**MCP SSE transport**: `src/mcp/sse.rs`
**Branch**: `001-002-integration-test`
