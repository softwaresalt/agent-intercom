---
description: Shared Monocoque Agent RC development guidelines for custom agents.
maturity: stable
---
# monocoque-agent-rc Development Guidelines

Last updated: 2026-02-15

## Active Technologies

| Dependency | Version | Purpose |
|---|---|---|
| Rust | stable, edition 2021 | Language toolchain |
| `rmcp` | 0.5 | MCP SDK (`ServerHandler`, `ToolRouter`, `ToolRoute`) |
| `axum` | 0.8 | HTTP/SSE transport (`StreamableHttpService` on `/mcp`) |
| `slack-morphism` | 2.17 | Slack Socket Mode client |
| `tokio` | 1.37 | Async runtime (full feature set) |
| `sqlx` | 0.8 | SQLite async driver (file-based prod, in-memory tests) |
| `diffy` | 0.4 | Unified diff parsing & patch application |
| `interprocess` | 2.0 | IPC named pipes (Windows) / Unix domain sockets |
| `clap` | 4.5 | CLI argument parsing |
| `notify` | 6.1 | Filesystem watcher (policy hot-reload) |
| `serde` / `serde_json` | 1.0 | Serialization |
| `sha2` | 0.10 | Content integrity hashing |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | Structured logging |
| `chrono` | 0.4 | Timestamps |
| `uuid` | 1.7 | Entity IDs |
| `keyring` | 3 | OS keychain credential access |
| `tokio-util` | 0.7 | `CancellationToken` for graceful shutdown |
| `reqwest` | 0.13 | HTTP client (rustls) |
| `glob` | 0.3 | Glob pattern matching |
| `toml` | 0.8 | TOML config file parsing |
| `tempfile` | 3.10 | Atomic file writes |

## Project Structure

```text
src/
  config.rs               # GlobalConfig, credential loading, TOML parsing
  errors.rs               # AppError enum (Config, Db, Slack, Mcp, Diff, Policy,
                           #   Ipc, PathViolation, PatchConflict, NotFound,
                           #   Unauthorized, AlreadyConsumed)
  lib.rs                  # Crate root — re-exports GlobalConfig, AppError, Result
  main.rs                 # CLI bootstrap, tokio runtime, server startup
  diff/                   # Unified diff parsing, patch application, atomic writes
    applicator.rs, patcher.rs, path_safety.rs, writer.rs
  ipc/                    # IPC server (named pipes / Unix sockets) for monocoque-ctl
    server.rs, socket.rs
  mcp/                    # MCP protocol layer
    handler.rs            # AppState, AgentRcServer, ToolRouter wiring
    context.rs            # Per-request context
    sse.rs                # HTTP/SSE transport (axum)
    transport.rs          # Stdio transport for direct agent connections
    tools/                # 9 MCP tool handlers
      accept_diff, ask_approval, check_auto_approve, forward_prompt,
      heartbeat, recover_state, remote_log, set_operational_mode,
      wait_for_instruction
    resources/             # MCP resource providers
      slack_channel
  models/                 # Domain models
    approval, checkpoint, policy, progress, prompt, session, stall
  orchestrator/           # Session lifecycle management
    session_manager, checkpoint_manager, spawner, stall_detector
  persistence/            # sqlite (sqlx) repository layer
    db.rs                 # connect(), schema bootstrap
    schema.rs             # SQL DDL (idempotent CREATE TABLE IF NOT EXISTS)
    approval_repo, checkpoint_repo, prompt_repo, session_repo,
    stall_repo, retention
  policy/                 # Workspace auto-approve rules
    evaluator, loader, watcher
  slack/                  # Slack Socket Mode integration
    blocks.rs             # Block Kit message builders
    client.rs             # SlackService (rate-limited message queue)
    commands.rs           # Slash command handlers
    events.rs             # Event dispatcher with authorization guard
    handlers/             # Per-event-type handlers
ctl/
  main.rs                 # monocoque-ctl companion CLI
lib/
  hve-core/               # External library (separate project)
tests/
  unit/                   # Unit tests (15 modules)
  contract/               # Contract tests (10 modules)
  integration/            # Integration tests (8 modules)
docs/
  adrs/                   # Architecture Decision Records (0001–0011)
specs/
  001-mcp-remote-agent-server/   # Feature specification
  002-sqlite-migration/          # Persistence migration spec
config.toml              # Runtime configuration
rustfmt.toml             # max_width = 100, edition = 2021
```

### Binaries

| Binary | Path | Description |
|---|---|---|
| `monocoque-agent-rc` | `src/main.rs` | MCP remote agent server |
| `monocoque-ctl` | `ctl/main.rs` | Local CLI companion (IPC client) |

## Quality Gates

Every code change must pass these gates in order. Do not skip any gate.

### Gate 1 — Compilation

```powershell
cargo check
```

All code must compile without errors. Run after every meaningful edit.

### Gate 2 — Lint Compliance

```powershell
cargo clippy -- -D warnings
```

The workspace sets `clippy::pedantic = "deny"`, `clippy::unwrap_used = "deny"`, and `clippy::expect_used = "deny"` via `[workspace.lints.clippy]` in `Cargo.toml`. Zero warnings allowed.

### Gate 3 — Formatting

```powershell
cargo fmt --all -- --check
```

If violations exist, run `cargo fmt --all` and re-check. Format config: `max_width = 100`, `edition = "2021"` (see `rustfmt.toml`).

### Gate 4 — Tests

```powershell
cargo test
```

All unit, contract, and integration tests must pass. If output may be truncated, redirect:

```powershell
cargo test 2>&1 | Out-File logs\test-results.txt
```

### Gate 5 — TDD Discipline

When adding new functionality:
1. Write the test first
2. Run it and **confirm it fails** (red)
3. Implement the production code
4. Run the test and confirm it passes (green)

Never write production code before the corresponding test exists and has been observed to fail.

## Code Style and Conventions

### Crate-Level Attributes

* `#![forbid(unsafe_code)]` — no unsafe anywhere (both `src/main.rs` and `ctl/main.rs`)
* `[workspace.lints.rust]`: `unsafe_code = "deny"`, `missing_docs = "warn"`
* `[workspace.lints.clippy]`: `pedantic = "deny"`, `unwrap_used = "deny"`, `expect_used = "deny"`

### Error Handling

* All fallible operations return `Result<T, AppError>` (type alias in `src/errors.rs`)
* `AppError` variants: `Config`, `Db`, `Slack`, `Mcp`, `Diff`, `Policy`, `Ipc`, `PathViolation`, `PatchConflict`, `NotFound`, `Unauthorized`, `AlreadyConsumed`
* Map external errors via `From` impls or `.map_err()` — never `unwrap()` or `expect()` in library/production code
* Error messages are lowercase and do not end with a period

### Naming

* Module files: `src/{module}/mod.rs` pattern for directories
* Struct IDs: prefixed strings (`task:uuid`, `context:uuid`, `spec:uuid`)
* Status values: `snake_case` (`todo`, `in_progress`, `done`, `blocked`)
* Default visibility: `pub(crate)` unless the item needs to be public API

### Documentation

* All public items require `///` doc comments
* Module-level `//!` doc comments on every `mod.rs` or standalone module file

### Database (SQLite)

* All DB access goes through `persistence/` repository modules — no raw queries elsewhere
* File-based SQLite for production, in-memory SQLite for tests (controlled by `connect(path, use_memory)`)
* Schema uses idempotent DDL (`CREATE TABLE IF NOT EXISTS`) in `persistence/schema.rs`

### MCP (rmcp 0.5)

* Implements `ServerHandler` trait on `IntercomServer` in `mcp/handler.rs`
* Tools registered via `ToolRouter` / `ToolRoute::new_dyn()` — no `#[tool]` proc macros
* All 9 tools always registered and visible; inapplicable calls return descriptive errors
* Blocking tools (`check_clearance`, `transmit`, `standby`) use `tokio::sync::oneshot` channels
* HTTP transport: axum `StreamableHttpService` on `/mcp` endpoint
* Stdio transport: `rmcp::transport::io::stdio()` for direct agent connections

### Slack (slack-morphism)

* Socket Mode — outbound-only WebSocket (no inbound firewall ports)
* All message posting routes through the rate-limited in-memory queue with exponential backoff
* Respect `Retry-After` headers from the Slack API
* Use Block Kit builders from `slack/blocks.rs` for all messages
* Centralized authorization guard in `slack/events.rs` — unauthorized users silently ignored
* Double-submission prevention via `chat.update` replacing buttons before handler dispatch

### Async (tokio)

* `tokio` 1 with `full` feature set
* `tokio::task::spawn_blocking` for CPU-bound or blocking I/O (e.g., `keyring` credential lookups)
* Drop `MutexGuard`/`RwLockGuard` before `.await` points
* `tokio_util::sync::CancellationToken` for graceful shutdown coordination
* `tokio::process::Command` with `kill_on_drop(true)` for agent session processes

### Path Security

* All file paths canonicalized and validated via `starts_with(workspace_root)` in `diff/path_safety.rs`
* Reject paths outside the workspace root — return `AppError::PathViolation`

### Workspace Policy

* Auto-approve rules in `.agentrc/settings.json` per workspace
* Hot-reloaded via `notify` file watcher in `policy/watcher.rs`
* Evaluated by `policy/evaluator.rs`

### IPC

* `interprocess` crate — named pipes (Windows) / Unix domain sockets
* JSON-line protocol (one JSON object per line, newline-delimited)

### Testing

* TDD required: write tests first, verify they fail, then implement
* Three test tiers in `tests/` directory (not inline):
  * `unit/` — isolated logic tests (12 modules)
  * `contract/` — MCP tool response contract verification (10 modules)
  * `integration/` — end-to-end flows with real SSE/DB (7 modules)
* Test DB: always use in-memory SQLite (`":memory:"`)
* Use `serial_test` crate for tests requiring sequential execution

## Architecture Reference

| Concern | Approach |
|---|---|
| MCP SDK | `rmcp` 0.5 — `ServerHandler` trait, `ToolRouter` / `ToolRoute::new_dyn()` |
| Transport (stdio) | `rmcp::transport::io::stdio()` for direct agent connections |
| Transport (HTTP) | axum 0.8 with `StreamableHttpService` on `/mcp` |
| Slack | `slack-morphism` 2.17 Socket Mode |
| Database | SQLite via `sqlx` 0.8 — file-based prod, in-memory tests, idempotent DDL |
| Configuration | TOML (`config.toml`) → `GlobalConfig`, credentials via keyring with env fallback |
| Workspace policy | JSON auto-approve rules (`.agentrc/settings.json`), hot-reloaded via `notify` |
| Diff safety | `diffy` 0.4 for unified diff parsing, `sha2` for integrity hashing, atomic writes via `tempfile` |
| Path security | All paths canonicalized and validated via `starts_with(workspace_root)` |
| IPC | `interprocess` 2.0 — named pipes (Windows) / Unix domain sockets for `monocoque-ctl` |
| Shutdown | `CancellationToken` — persist state, notify Slack, terminate children gracefully |
| ADRs | Numbered markdown files in `docs/adrs/` (currently 0001–0011) |

## Remote Approval Workflow for File Changes

When the monocoque-agent-rc MCP server is running, agents **must** route all file modifications through the remote approval workflow instead of writing files directly. This allows the operator to review and approve every change via Slack before it touches the filesystem.

Additionally, **do not write multiple files in a single proposal.** Each file change must be proposed, reviewed, and approved separately to ensure clear audit trails and granular control.  Further, when modifying existing files, always generate a unified diff rather than sending the full file content. This provides better context for reviewers and reduces the risk of unintended changes.

For terminal commands, **never chain multiple commands together**. Each command must be submitted separately to the `evaluate_command` tool for proper policy evaluation and approval. If the terminal command is **not** already auto-approved for the current workspace or current working directory**, it may be executed directly without approval, but still must not be chained with other commands unless those commands are effectively piping output.

### Required Call Sequence

```text
1. auto_check       →  Can this change bypass approval?
2. check_clearance   →  Submit the proposal (blocks until operator responds)
3. check_diff        →  Apply the approved change to the filesystem
```

### Step 1 — `auto_check`

Call **before** every file write to check if the workspace policy allows the operation without human review.

| Parameter   | Type     | Required | Description |
|-------------|----------|----------|-------------|
| `tool_name` | `string` | yes      | Name of the tool or command being executed |
| `context`   | `object` | no       | `{ "file_path": "...", "risk_level": "..." }` |

- If `auto_approved: true` → the agent may write the file directly (skip steps 2–3).
- If `auto_approved: false` → proceed to step 2.

### Step 2 — `check_clearance`

Submit the proposed change for operator review. This call **blocks** until the operator taps Accept/Reject in Slack or the timeout elapses.

| Parameter     | Type     | Required | Description |
|---------------|----------|----------|-------------|
| `title`       | `string` | yes      | Concise summary of the proposed change |
| `diff`        | `string` | yes      | Standard unified diff or full file content |
| `file_path`   | `string` | yes      | Target file path relative to workspace root |
| `description` | `string` | no       | Additional context about the change |
| `risk_level`  | `string` | no       | `low` (default), `high`, or `critical` |

**Response:** `{ "status": "approved" | "rejected" | "timeout", "request_id": "...", "reason": "..." }`

- `approved` → proceed to step 3 with the returned `request_id`.
- `rejected` → do **not** apply the change. Adapt or abandon based on the `reason`.
- `timeout` → treat as rejection. Do not retry automatically without operator guidance.

### Step 3 — `check_diff`

Apply the approved change to the filesystem. Only call this after receiving `status: "approved"`.

| Parameter    | Type      | Required | Description |
|--------------|-----------|----------|-------------|
| `request_id` | `string`  | yes      | The `request_id` from the `check_clearance` response |
| `force`      | `boolean` | no       | `true` to overwrite even if the file changed since proposal |

**Response:** `{ "status": "applied", "files_written": [{ "path": "...", "bytes": N }] }`

If the server returns `patch_conflict` (file changed since proposal), the agent should re-read the file, regenerate the diff, and restart from step 2.

### Rules

1. **Never write files directly** when the MCP server is reachable. Always use the approval workflow.
2. **One file per approval.** Submit each file change as a separate `check_clearance` call.
3. **Use unified diffs** when modifying existing files. Use raw file content only for new files.
4. **Set `risk_level`** to `high` or `critical` for changes to configuration files, security-sensitive modules (`diff/path_safety.rs`, `policy/`, `slack/events.rs`), or database schema (`persistence/schema.rs`).
5. **Do not retry rejected proposals** with the same content. Incorporate the operator's feedback first.
6. **Handle all response statuses.** Never assume approval — always branch on `approved`, `rejected`, and `timeout`.

<!-- MANUAL ADDITIONS START -->

## Terminal Command Execution Policy

**Do NOT chain terminal commands.** Run each command as a separate, standalone invocation.

### Rules

1. **One command per terminal call.** Never combine commands with `;`, `&&`, `||`, or `|` unless it falls under an allowed exception below.
2. **No `cmd /c` wrappers.** Run commands directly in the shell rather than wrapping them in `cmd /c "..."`. If `cmd /c` is genuinely required (e.g., for environment isolation), it must contain a single command only.
3. **No exit-code echo suffixes.** Do not append `; echo "EXIT: $LASTEXITCODE"` or `&& echo "done"` to commands. The terminal tool already captures exit codes.
4. **Check results between commands.** After each command, inspect the output and exit code before deciding whether to run the next command. This is safer and produces better diagnostics.
5. **Always use `pwsh`, never `powershell`.** When invoking PowerShell explicitly (e.g., to run a `.ps1` script), use `pwsh` — the cross-platform PowerShell 7+ executable. Never use `powershell` or `powershell.exe`, which refers to the legacy Windows PowerShell 5.1 runtime.
6. **Always use relative paths for output redirection.** When redirecting command output to a file, use workspace-relative paths (e.g., `logs\results.txt`), never absolute paths (e.g., `d:\Source\...\logs\results.txt`). Absolute paths break auto-approve regex matching.
7. **Temporary output files go in `logs/`.** All temporary output files — compilation logs, test results, clippy output, diff captures, or any other ephemeral terminal output redirected to disk — must be written to the `logs/` folder, never to `target/` or the workspace root. The `logs/` folder is gitignored and designated for transient artifacts. Example: `cargo test 2>&1 | Out-File logs\test-results.txt`.

### Allowed Exceptions

Output redirection is **not** command chaining — it is I/O plumbing that cannot execute destructive operations. The following patterns are permitted:

- **Shell redirection operators**: `>`, `>>`, `2>&1` (e.g., `cargo test > logs/results.txt 2>&1`)
- **Pipe to `Out-File` or `Set-Content`**: `cargo test 2>&1 | Out-File logs/results.txt` or `| Set-Content`
- **Pipe to `Out-String`**: `some-command | Out-String`

Use these when the terminal tool's ~60 KB output limit would truncate results (e.g., full `cargo test` compilation + test output).

### Why

Terminal auto-approve rules use regex pattern matching against the full command line. Chained commands create unpredictable command strings that cannot be reliably matched, forcing manual approval prompts that slow down the workflow. Single commands match cleanly and approve instantly.

### Correct Examples

```powershell
# Good: separate calls
cargo check
# (inspect output)
cargo clippy -- -D warnings
# (inspect output)
cargo test

# Good: output redirection to capture full results
cargo test 2>&1 | Out-File logs\test-results.txt

# Good: shell redirect when output may be truncated
cargo test > logs\test-results.txt 2>&1
```

### Incorrect Examples

```powershell
# Bad: chained with semicolons
cargo check; cargo clippy -- -D warnings; cargo test

# Bad: cmd /c wrapper with echo suffix
cmd /c "cargo test > logs\test-results.txt 2>&1"; echo "EXIT: $LASTEXITCODE"

# Bad: output redirect to target/ instead of logs/
cargo test 2>&1 | Out-File target\test-results.txt

# Bad: AND-chained
cargo fmt && cargo clippy && cargo test

# Bad: pipe to something other than Out-File/Set-Content/Out-String
cargo test | Select-String "FAILED" | Remove-Item foo.txt
```
### Full List of Auto-Approve Commands with RegEx

"chat.tools.terminal.autoApprove": {
    ".specify/scripts/bash/": true,
    ".specify/scripts/powershell/": true,
    "/^cargo (build|test|run|clippy|fmt|check|doc|update|install|search|publish|login|logout|new|init|add|upgrade|version|help|bench)(\\s[^;|&`]*)?(\\s*(>|>>|2>&1|\\|\\s*(Out-File|Set-Content|Out-String))\\s*[^;|&`]*)*$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^& cargo (build|test|run|clippy|fmt|check|doc|update|install|search|publish|login|logout|new|init|add|upgrade|version|help|bench)(\\s[^;|&`]*)?(\\s*(>|>>|2>&1|\\|\\s*(Out-File|Set-Content|Out-String))\\s*[^;|&`]*)*$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^cargo --(help|version|verbose|quiet|release|features)(\\s[^;|&`]*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^git (status|add|commit|diff|log|fetch|pull|push|checkout|branch|--version)(\\s[^;|&`]*)?(\\s*(>|>>|2>&1|\\|\\s*(Out-File|Set-Content|Out-String))\\s*[^;|&`]*)*$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^& git (status|add|commit|diff|log|fetch|pull|push|checkout|branch|--version)(\\s[^;|&`]*)?(\\s*(>|>>|2>&1|\\|\\s*(Out-File|Set-Content|Out-String))\\s*[^;|&`]*)*$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^(Out-File|Set-Content|Add-Content|Get-Content|Get-ChildItem|Copy-Item|Move-Item|New-Item|Test-Path)(\\s[^;|&`]*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^(echo|dir|mkdir|where\\.exe|vsWhere\\.exe|rustup|rustc|refreshenv)(\\s[^;|&`]*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^cmd /c \"cargo (test|check|clippy|fmt|build|doc|bench)(\\s[^;|&`]*)?\"(\\s*[;&|]+\\s*echo\\s.*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "New-Item": true,
    "Out-Null": true,
    "cargo build": true,
    "cargo check": true,
    "cargo doc": true,
    "cargo test": true,
    "git commit": true,
    "ForEach-Object": true,
    "cargo clippy": true,
    "cargo fmt": true,
    "git add": true,
    "git push": true
}
<!-- MANUAL ADDITIONS END -->
