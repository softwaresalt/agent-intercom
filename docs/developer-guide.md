# Developer Guide

Instructions for building, testing, and contributing to agent-intercom.

## Prerequisites

- **Rust stable** (edition 2021) — install via [rustup](https://rustup.rs/)
- **Windows**, **Linux**, or **macOS** — all platforms supported
- **PowerShell 7+** (`pwsh`) for running scripts on all platforms

## Project Structure

```text
src/
  config.rs               # GlobalConfig, credential loading, TOML parsing
  errors.rs               # AppError enum — all error variants
  lib.rs                  # Crate root — re-exports GlobalConfig, AppError, Result
  main.rs                 # CLI bootstrap, tokio runtime, server startup
  diff/                   # Unified diff parsing, patch application, atomic writes
    applicator.rs, patcher.rs, path_safety.rs, writer.rs
  ipc/                    # IPC server for agent-intercom-ctl
    server.rs, socket.rs
  mcp/                    # MCP protocol layer
    handler.rs            # AppState, IntercomServer, ToolRouter wiring
    context.rs            # Per-request context
    sse.rs                # HTTP/SSE transport (axum)
    transport.rs          # Stdio transport for direct agent connections
    tools/                # 9 MCP tool handlers
      accept_diff, ask_approval, check_auto_approve, forward_prompt,
      heartbeat, recover_state, remote_log, set_operational_mode,
      wait_for_instruction
    resources/            # MCP resource providers
      slack_channel
  models/                 # Domain models
    approval, checkpoint, policy, progress, prompt, session, stall
  orchestrator/           # Session lifecycle management
    session_manager, checkpoint_manager, spawner, stall_detector
  persistence/            # SQLite repository layer (sqlx 0.8)
    db.rs, schema.rs, approval_repo, checkpoint_repo, prompt_repo,
    session_repo, stall_repo, retention
  policy/                 # Workspace auto-approve rules
    evaluator, loader, watcher
  slack/                  # Slack Socket Mode integration
    blocks.rs             # Block Kit message builders
    client.rs             # SlackService (rate-limited message queue)
    commands.rs           # Slash command handlers
    events.rs             # Event dispatcher with authorization guard
    handlers/             # Per-event-type handlers
ctl/
  main.rs                 # agent-intercom-ctl companion CLI
tests/
  unit/                   # 150+ unit tests
  contract/               # 170+ MCP tool contract tests
  integration/            # 210+ end-to-end flow tests
docs/
  adrs/                   # Architecture Decision Records
specs/                    # Feature specifications
```

### Binaries

| Binary | Path | Description |
|---|---|---|
| `agent-intercom` | `src/main.rs` | MCP remote agent server |
| `agent-intercom-ctl` | `ctl/main.rs` | Local CLI companion (IPC client) |

## Building

```powershell
# Debug build (fastest)
cargo build

# Release build (optimized)
cargo build --release
```

Binaries land in `target/debug/` or `target/release/`.

## Running Locally

Use the included debug script, which loads credentials from user-level environment variables:

```powershell
cargo build
pwsh ./run-debug.ps1
```

Or run directly:

```powershell
$env:SLACK_APP_TOKEN  = [System.Environment]::GetEnvironmentVariable("SLACK_APP_TOKEN", "User")
$env:SLACK_BOT_TOKEN  = [System.Environment]::GetEnvironmentVariable("SLACK_BOT_TOKEN", "User")
$env:SLACK_TEAM_ID    = [System.Environment]::GetEnvironmentVariable("SLACK_TEAM_ID", "User")
$env:SLACK_MEMBER_IDS = [System.Environment]::GetEnvironmentVariable("SLACK_MEMBER_IDS", "User")
$env:RUST_LOG         = "debug"

.\target\debug\agent-intercom.exe --config config.toml
```

## Quality Gates

Every code change must pass all five gates in order:

### Gate 1 — Compilation

```powershell
cargo check
```

### Gate 2 — Lint Compliance

```powershell
cargo clippy -- -D warnings
```

The workspace sets `clippy::pedantic = "deny"`, `clippy::unwrap_used = "deny"`, and `clippy::expect_used = "deny"`. Zero warnings allowed.

### Gate 3 — Formatting

```powershell
cargo fmt --all -- --check
```

Fix violations with `cargo fmt --all`. Format config: `max_width = 100`, `edition = "2021"` (see `rustfmt.toml`).

### Gate 4 — Tests

```powershell
cargo test
```

All unit, contract, and integration tests must pass. Redirect output if it may be truncated:

```powershell
cargo test 2>&1 | Out-File logs\test-results.txt
Get-Content logs\test-results.txt | Select-String "test result"
```

### Gate 5 — TDD Discipline

1. Write the test first.
2. Run it and **confirm it fails** (red).
3. Implement the production code.
4. Run the test and confirm it passes (green).

Never write production code before the corresponding test exists and has been observed to fail.

## Code Conventions

### Error Handling

- All fallible operations return `Result<T, AppError>` (type alias in `src/errors.rs`).
- Never use `unwrap()` or `expect()` in library or production code.
- Map external errors via `From` impls or `.map_err()`.
- Error messages are lowercase and do not end with a period.

### Naming

- Module files: `src/{module}/mod.rs` pattern for directories.
- Default visibility: `pub(crate)` unless the item needs to be public API.

### Documentation

- All public items require `///` doc comments.
- Module-level `//!` doc comments on every `mod.rs` or standalone module file.

### Database

- All DB access goes through `persistence/` repository modules — no raw queries elsewhere.
- Use in-memory SQLite for tests (`:memory:`).
- Schema uses idempotent DDL (`CREATE TABLE IF NOT EXISTS`).

### Async

- Drop `MutexGuard`/`RwLockGuard` before `.await` points.
- Use `tokio::task::spawn_blocking` for CPU-bound or blocking I/O.
- Use `tokio_util::sync::CancellationToken` for graceful shutdown coordination.

### Session Lifecycle Internals

The MCP handler (`src/mcp/handler.rs`) uses `on_initialized` to manage session creation:

- **Case 1 — Spawned agent:** The transport URL contains `?session_id=<id>`. The handler looks up the pre-created session and binds to it. No stale cleanup runs.
- **Case 2 — Primary agent (direct connection):** No `session_id` parameter. The handler terminates all existing `agent:local` Active sessions (stale cleanup), then creates a new session with `owner_user_id = "agent:local"`.

Key implementation details:

- `session_db_id` is stored in a `OnceLock<i64>` on the handler. It is set once during `on_initialized` and read by every tool handler to identify the current session. It is never overwritten.
- The `Drop` impl on the handler sets the session status to `Terminated` by spawning a blocking task on the Tokio runtime. If the runtime is unavailable (process exit, test teardown), the stale is cleaned up on the next `on_initialized`.
- `LOCAL_AGENT_OWNER` is a `pub(crate)` constant (`"agent:local"`) used as the owner for all primary agent sessions.
- `ActiveChildren` (`Arc<Mutex<HashMap<String, Child>>>`) tracks spawned agent processes for cleanup on server shutdown.

These details are relevant when writing tests that exercise session transitions. Tests using `AppState` must provide `active_children: Arc::default()` and handle the case where `session_db_id` may not be set if `on_initialized` was not called.

## Testing

Three test tiers live under `tests/`:

| Tier | Directory | Scope |
|---|---|---|
| Unit | `tests/unit/` | Isolated logic tests |
| Contract | `tests/contract/` | MCP tool response contract verification |
| Integration | `tests/integration/` | End-to-end flows with real SSE/DB |

Run individual tiers:

```powershell
cargo test --test unit
cargo test --test contract
cargo test --test integration
```

Run a specific test by name:

```powershell
cargo test contract::check_clearance
```

## Architecture Decision Records

Numbered markdown files in `docs/adrs/` record key architectural decisions (ADR-0001 through ADR-0012). Read these to understand why specific design choices were made.

## Contribution Workflow

1. Create a feature branch: `git checkout -b feature/your-feature`
2. Run the baseline: `cargo test`
3. Write a failing test (TDD red).
4. Implement the feature (TDD green).
5. Run all gates: `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt --all`, `cargo test`
6. Commit with a descriptive message and push.
7. Open a pull request.

## Release Builds

```powershell
cargo build --release
```

The release binaries are at:
- `target/release/agent-intercom.exe` (Windows)
- `target/release/agent-intercom` (Linux/macOS)
- `target/release/agent-intercom-ctl.exe` (Windows)
- `target/release/agent-intercom-ctl` (Linux/macOS)
