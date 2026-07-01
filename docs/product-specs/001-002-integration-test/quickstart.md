# Quickstart: Integration Test Full Coverage

**Feature**: 001-002-integration-test | **Date**: 2026-02-22

## Prerequisites

- Rust stable toolchain (edition 2021)
- `cargo` on PATH

## Build

No additional dependencies are required. All test infrastructure uses existing workspace dependencies (`tokio`, `tempfile`, `sqlx`, `interprocess`, `notify`).

```powershell
cargo check
```

## Run All Tests

```powershell
cargo test
```

Or with output capture for full results:

```powershell
cargo test 2>&1 | Out-File logs\test-results.txt
```

## Run Specific Test Modules

New integration test files:

```powershell
# Policy hot-reload tests
cargo test --test integration policy_watcher_tests

# IPC server command dispatch tests
cargo test --test integration ipc_server_tests

# MCP transport dispatch tests (if added)
cargo test --test integration mcp_dispatch_tests
```

## Verify Quality Gates

```powershell
cargo check
cargo clippy -- -D warnings
cargo fmt --all -- --check
cargo test
```

## Test Structure

All new tests are in `tests/integration/` and registered in `tests/integration.rs`:

| Module | Tests | Spec Coverage |
|---|---|---|
| `policy_watcher_tests` | Hot-reload, deletion fallback, malformed file | FR-007, US6 |
| `ipc_server_tests` | Auth enforcement, list/approve/reject/resume/mode | FR-008, US7 |
| `mcp_dispatch_tests` | Full call_tool dispatch via transport (if feasible) | FR-001, US1 |

## Key Patterns

- **Database**: `db::connect_memory()` — fresh in-memory SQLite per test
- **Filesystem**: `tempfile::tempdir()` — isolated temp directory per test
- **IPC pipes**: Unique name per test via UUID suffix
- **Timeouts**: All async assertions use `tokio::time::timeout()` with 2-5s bounds
- **Policy watcher**: Poll-with-timeout pattern (50ms interval, 2s max) for `notify` event convergence
