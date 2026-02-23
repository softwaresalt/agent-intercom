# Phase 2 Memory — Policy Hot-Reload Tests (001-002-integration-test)

**Date**: 2026-02-22
**Phase**: 2 of 5 (US6 — Policy Hot-Reload, FR-007)
**Tasks**: T004–T011
**Commit**: daa32a9

## Task Overview

Phase 2 goal: populate `tests/integration/policy_watcher_tests.rs` with 6 integration tests
covering the `PolicyWatcher` hot-reload lifecycle (S045–S050), then verify all pass and
clippy/fmt are clean.

## What Was Done

### Tests Written (T004–T009)

All 6 test functions written in `tests/integration/policy_watcher_tests.rs`:

| Test | Scenario | What it verifies |
|------|----------|-----------------|
| `register_loads_initial_policy` | S045 | `register()` reads settings.json and populates cache |
| `policy_file_modification_detected` | S046 | Writing updated JSON triggers hot-reload within 2s |
| `policy_file_deletion_falls_back_to_deny_all` | S047 | Removing settings.json triggers deny-all fallback |
| `malformed_policy_file_uses_deny_all` | S048 | Invalid JSON during register → deny-all immediately |
| `unregister_stops_watching` | S049 | After unregister, file changes don't update cache |
| `multiple_workspaces_independent_policies` | S050 | Two workspaces watch independently |

### Helpers Created

- `write_policy_file(dir, json)` — creates `.monocoque/settings.json` in a tempdir
- `poll_until(watcher, root, timeout_ms, pred)` — async polling helper (50ms interval / configurable timeout)

### Verification (T010–T011)

- `cargo test --test integration policy_watcher_tests`: 6/6 pass
- `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`: zero warnings (after fixing `needless_raw_string_hashes`)
- `cargo fmt --all -- --check`: clean (after auto-fix for 100-char line width violations)
- Full suite: 487 tests, 0 failed

## Files Modified

| File | Change |
|------|--------|
| `tests/integration/policy_watcher_tests.rs` | Replaced stub with 6 full test functions + helpers |
| `specs/001-002-integration-test/tasks.md` | T004–T011 marked `[X]` complete |

## Important Discoveries

1. **FR-011 command filtering**: `PolicyLoader::load()` silently strips commands not in `global_commands`
   allowlist. Tests verifying `commands` field must pass the command in
   `PolicyWatcher::new(global_commands)`. Tests only checking `enabled` can use `HashMap::new()`.

2. **Pedantic clippy: `needless_raw_string_hashes`**: Raw strings like `r#"text without quotes"#`
   must use `r"text without quotes"` if they don't contain double-quotes. Fixed in T007.

3. **rustfmt line width**: The `max_width = 100` setting in `rustfmt.toml` reformats long function
   signatures (the `poll_until` generic function) and long assert messages into multi-line form.
   Always run `cargo fmt --all` before each commit.

4. **Watcher deferred on missing dir**: `PolicyWatcher::register()` succeeds even when `.monocoque/`
   doesn't exist yet — it logs "watcher deferred" and stores the watcher (which won't fire events).
   Tests must pre-create `.monocoque/settings.json` before calling `register()`.

## Next Steps

**Phase 3** (US7 — IPC Server command dispatch):
- Read `src/ipc/server.rs` and `src/ipc/socket.rs` to understand public spawn API
- Key concern: unique pipe names per test to avoid cross-test conflicts on Windows
- 8 test functions: auth enforcement (valid/invalid/missing token) + command dispatch
  (list, approve, reject, resume, mode)
- IPC tests need `AppState` with real in-memory SQLite for session/approval queries

## Context to Preserve

**Spec location**: `specs/001-002-integration-test/`
**Test entry point**: `tests/integration.rs`
**Test helpers**: `tests/integration/test_helpers.rs`
**Policy module**: `src/policy/watcher.rs`, `src/policy/loader.rs`, `src/models/policy.rs`
**IPC module (Phase 3)**: `src/ipc/server.rs`, `src/ipc/socket.rs`
**Branch**: `001-002-integration-test`
**Commit**: daa32a9
