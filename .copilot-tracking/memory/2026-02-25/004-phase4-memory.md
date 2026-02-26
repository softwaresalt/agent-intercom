# Phase 4 Memory — Server Startup Reliability (T024-T027)

**Date**: 2026-02-25
**Spec**: 004-intercom-advanced-features
**Phase**: 4
**Commit**: f6aeb6e

## What Was Built

### New Functions in `src/mcp/sse.rs`

- `bind_http(state: &AppState) -> Result<TcpListener>`: Eagerly binds the TCP port. Returns `AppError::Config` on failure.
- `serve_with_listener(listener, state, ct) -> Result<()>`: Inner serve logic accepting a pre-bound listener. Contains all router/service setup.
- `serve_http(state, ct) -> Result<()>`: Thin wrapper calling `bind_http` then `serve_with_listener` (backward-compatible).

### `src/main.rs` Changes

Moved bind from spawned task to pre-spawn eager check:
```rust
match sse::bind_http(&state).await {
    Ok(listener) => Some(tokio::spawn(async move {
        sse::serve_with_listener(listener, sse_state, sse_ct).await
    })),
    Err(err) => {
        error!(%err, "failed to bind HTTP transport — shutting down and exiting");
        // Abort Slack runtime
        if let Some(ref rt) = slack_runtime { rt.queue_task.abort(); }
        std::process::exit(1);
    }
}
```

### Tests (`tests/integration/startup_tests.rs`)

4 integration tests covering S023-S026:
- `bind_http_succeeds_on_free_port` — port 0, expect success
- `bind_http_fails_on_occupied_port` — pre-bound port, expect error
- `bind_http_returns_config_error_variant` — verify error is `AppError::Config`
- `second_bind_on_same_port_fails` — simulate second instance

**Key fix**: Use `std::env::temp_dir()` for workspace root in test configs (not `/tmp` which doesn't exist on Windows).

## Test Count

596 total tests, 0 failures (222 integration, 181 contract, 163 unit, 29 lib, 1 extra).

## Design Decision

Chose `bind_http` + `serve_with_listener` split (not modifying `serve_http` signature) so:
1. All existing callers of `serve_http` continue to work unchanged
2. `main.rs` gets early bind detection
3. Tests can call `bind_http` directly

## Phase 5 Next

Task Inbox (T028-T035): 8 tasks, builds on `inbox_repo.rs` (already created in Phase 2).
- `recover_state` tool fetches and delivers pending inbox items
- `/intercom task <text>` slash command
- IPC `task` command and CTL subcommand
