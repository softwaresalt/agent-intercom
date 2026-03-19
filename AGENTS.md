# agent-intercom — Agent Instructions

This file is read automatically by `copilot` CLI and other agent tools that
support `AGENTS.md`. It defines the authoritative rules for working in this
repository. All agents operating here must follow these instructions regardless
of flags such as `--allow-all`, `--yolo`, or `--autopilot`.

Last updated: 2026-03-07 | Constitution version: 2.2.0

---

## Core Principles

### I. Safety-First Rust (NON-NEGOTIABLE)

All production code MUST be written in Rust (stable toolchain, edition 2021).
`unsafe` code is forbidden at the workspace level (`#![forbid(unsafe_code)]`).
Clippy pedantic lints MUST pass with zero warnings. `unwrap()` and `expect()`
are denied; all fallible operations MUST use the `Result`/`AppError` pattern
defined in `src/errors.rs`.

### II. MCP Protocol Fidelity

The server MUST implement the Model Context Protocol via the `rmcp` 0.13 SDK.
All MCP tools MUST be unconditionally visible to every connected agent. Tools
called in inapplicable contexts MUST return a descriptive error rather than
being hidden. Custom notifications (e.g., `intercom/nudge`) MUST use the
standard MCP notification mechanism.

### III. Test-First Development (NON-NEGOTIABLE)

Every feature MUST have tests written before implementation code. The test
directory structure (`tests/contract/`, `tests/integration/`, `tests/unit/`)
MUST be maintained. All tests MUST pass via `cargo test` before any code is
merged. Steps: write test → confirm it fails (red) → implement → confirm it
passes (green). Never write production code before the corresponding test
exists and has been observed to fail.

### IV. Security Boundary Enforcement

All file-system operations MUST resolve within the configured workspace root.
Path traversal attempts MUST be rejected with `AppError::PathViolation`.
Remote command execution MUST be restricted to the explicit allowlist in the
global configuration. Sensitive credentials (Slack tokens) MUST be loaded from
the OS keychain with environment-variable fallback; credentials MUST NOT be
stored in plaintext configuration files. Each agent session MUST be bound to
exactly one Slack user (owner) at creation time — only the session owner may
interact with that session.

### V. Structured Observability

All significant operations MUST emit structured tracing spans to stderr via
`tracing-subscriber`. Span coverage MUST include: MCP tool call execution,
Slack API interactions, stall detection events, and session lifecycle
transitions.

### VI. Single-Binary Simplicity

The project MUST produce a single workspace with two binaries
(`agent-intercom` and `agent-intercom-ctl`). New dependencies MUST be
justified by a concrete requirement — do not add libraries speculatively.
Prefer the standard library over external crates when adequate. SQLite via
sqlx (bundled) is the sole persistence layer.

### VII. CLI Workspace Containment (NON-NEGOTIABLE)

When an agent operates in CLI mode, it MUST NOT create, modify, or delete any
file or directory outside the current working directory tree. This applies to
all file operations. Paths that resolve above or outside the cwd — whether via
absolute paths, `..` traversal, symlinks, or environment variable expansion —
MUST be refused. The only exception is reading files explicitly provided by
the user as context.

### VIII. Destructive Terminal Command Approval (NON-NEGOTIABLE)

All destructive terminal commands MUST go through agent-intercom operator
approval before execution, regardless of `--allow-all`, `--yolo`, or any
other permissive mode. A terminal command is destructive if it:

- Deletes files or directories (`rm`, `Remove-Item`, `del`, `rmdir`)
- Overwrites files without backup (`mv` to existing target, `Move-Item -Force`)
- Modifies system configuration (`reg`, `Set-ExecutionPolicy`, `chmod`, `chown`)
- Alters version control history (`git reset --hard`, `git push --force`, `git clean -fd`)
- Drops or truncates database content (`DROP TABLE`, `TRUNCATE`, `DELETE FROM` without `WHERE`)
- Installs or removes system-level packages (`npm install -g`, `cargo install`, `apt remove`)
- Executes arbitrary code from untrusted sources (`curl | sh`, `iex (irm ...)`)

Required workflow: `auto_check` → `check_clearance` → execute only after
`status: "approved"`. Permissive flags do NOT bypass this gate.

---

## Technical Constraints

| Concern | Constraint |
|---|---|
| Language | Rust stable, edition 2021 |
| Async runtime | Tokio (full features) |
| MCP SDK | `rmcp` 0.13 — `ServerHandler`, `ToolRouter`, `ToolRoute::new_dyn()` |
| HTTP transport | Axum 0.8 — `StreamableHttpService` at `/mcp` |
| Slack | `slack-morphism` 2.17 — Socket Mode |
| Persistence | SQLite via sqlx 0.8 (bundled; in-memory for tests) |
| Diff/Patch | `diffy` 0.4 |
| File watching | `notify` 6.x |
| Formatting | `rustfmt.toml`: `max_width = 100`, edition 2021 |
| Linting | `cargo clippy` pedantic deny, `unwrap_used` deny, `expect_used` deny |
| Build check | `cargo test && cargo clippy` MUST pass before merge |

---

## Active Dependencies

| Crate | Version | Purpose |
|---|---|---|
| `rmcp` | 0.13 | MCP SDK |
| `axum` | 0.8 | HTTP/SSE transport |
| `slack-morphism` | 2.17 | Slack Socket Mode client |
| `tokio` | 1.37 | Async runtime |
| `sqlx` | 0.8 | SQLite async driver |
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
| `regex` | 1.12 | Regular expression matching (policy evaluation) |

---

## Project Structure

```text
src/
  config.rs               # GlobalConfig, credential loading, TOML parsing
  errors.rs               # AppError enum
  lib.rs                  # Crate root — re-exports GlobalConfig, AppError, Result
  main.rs                 # CLI bootstrap, tokio runtime, server startup
  audit/writer.rs         # AuditLogger trait and file-based implementation
  diff/                   # Unified diff parsing, patch application, atomic writes
    applicator.rs, patcher.rs, path_safety.rs, writer.rs
  ipc/                    # IPC server for agent-intercom-ctl
    server.rs, socket.rs
  mcp/                    # MCP protocol layer
    handler.rs            # AppState, ToolRouter wiring
    sse.rs                # HTTP/SSE transport (axum)
    transport.rs          # Stdio transport
    tools/                # MCP tool handlers
    resources/slack_channel
  models/                 # Domain models
    approval, checkpoint, inbox, policy, progress, prompt, session, stall, steering
  orchestrator/           # Session lifecycle management
    session_manager, checkpoint_manager, child_monitor, spawner,
    stall_consumer, stall_detector
  persistence/            # SQLite repository layer
    db.rs, schema.rs
    approval_repo, checkpoint_repo, inbox_repo, prompt_repo,
    session_repo, stall_repo, steering_repo, retention
  policy/                 # Workspace auto-approve rules
    evaluator, loader, watcher
  slack/                  # Slack Socket Mode integration
    blocks.rs             # Block Kit message builders
    client.rs             # SlackService (rate-limited message queue)
    commands.rs           # Slash command handlers
    events.rs             # Event dispatcher with authorization guard
    handlers/             # Per-event-type handlers
ctl/main.rs               # agent-intercom-ctl companion CLI
tests/
  unit/                   # Isolated logic tests
  contract/               # MCP tool response contract verification
  integration/            # End-to-end flows with real SSE/DB
docs/adrs/                # Architecture Decision Records (0001–0013)
specs/                    # Feature specifications (001–005)
config.toml               # Runtime configuration
rustfmt.toml              # max_width = 100, edition = 2021
```

### Binaries

| Binary | Path | Description |
|---|---|---|
| `agent-intercom` | `src/main.rs` | MCP remote agent server |
| `agent-intercom-ctl` | `ctl/main.rs` | Local CLI companion (IPC client) |

---

## Quality Gates

Run in order. Do not skip any gate.

```powershell
# Gate 1 — Compilation
cargo check

# Gate 2 — Lint (zero warnings required)
cargo clippy -- -D warnings

# Gate 3 — Formatting
cargo fmt --all -- --check
# If violations: cargo fmt --all

# Gate 4 — Tests (all must pass)
cargo test
# If output truncated:
cargo test 2>&1 | Out-File logs\test-results.txt
```

---

## Code Style and Conventions

### Error Handling

- All fallible operations return `Result<T, AppError>` (type alias in `src/errors.rs`)
- `AppError` variants: `Config`, `Db`, `Slack`, `Mcp`, `Diff`, `Policy`, `Ipc`,
  `PathViolation`, `PatchConflict`, `NotFound`, `Unauthorized`, `AlreadyConsumed`, `Io`
- Map external errors via `From` impls or `.map_err()` — never `unwrap()` or `expect()`
- Error messages are lowercase and do not end with a period

### Naming

- Module files: `src/{module}/mod.rs` pattern for directories
- Struct IDs: prefixed strings (`task:uuid`, `context:uuid`, `spec:uuid`)
- Status values: `snake_case` (`todo`, `in_progress`, `done`, `blocked`)
- Default visibility: `pub(crate)` unless the item needs to be public API

### Documentation

- All public items require `///` doc comments
- Module-level `//!` doc comments on every `mod.rs` or standalone module file

### Database (SQLite)

- All DB access goes through `persistence/` repository modules — no raw queries elsewhere
- File-based SQLite for production, in-memory SQLite for tests
- Schema uses idempotent DDL (`CREATE TABLE IF NOT EXISTS`) in `persistence/schema.rs`

### MCP (rmcp 0.13)

- Implements `ServerHandler` trait on `IntercomServer` in `mcp/handler.rs`
- Tools registered via `ToolRouter` / `ToolRoute::new_dyn()` — no `#[tool]` proc macros
- All tools always registered and visible; inapplicable calls return descriptive errors
- Blocking tools (`check_clearance`, `transmit`, `standby`) use `tokio::sync::oneshot` channels

### Slack (slack-morphism)

- Socket Mode — outbound-only WebSocket (no inbound firewall ports)
- All message posting routes through the rate-limited in-memory queue with exponential backoff
- Respect `Retry-After` headers from the Slack API
- Use Block Kit builders from `slack/blocks.rs` for all messages
- Centralized authorization guard in `slack/events.rs` — unauthorized users silently ignored
- Double-submission prevention via `chat.update` replacing buttons before handler dispatch

### Async (tokio)

- `tokio::task::spawn_blocking` for CPU-bound or blocking I/O
- Drop `MutexGuard`/`RwLockGuard` before `.await` points
- `tokio_util::sync::CancellationToken` for graceful shutdown coordination
- `tokio::process::Command` with `kill_on_drop(true)` for agent session processes

### Path Security

- All file paths canonicalized and validated via `starts_with(workspace_root)` in `diff/path_safety.rs`
- Reject paths outside the workspace root — return `AppError::PathViolation`

### Testing

- TDD required: write tests first, verify they fail, then implement
- Three test tiers in `tests/` directory (not inline)
- Test DB: always use in-memory SQLite (`":memory:"`)
- Use `serial_test` crate for tests requiring sequential execution

---

## Remote Approval Workflow for Destructive File Operations

File creation and modification proceed directly — no approval needed.
The approval workflow applies to **destructive operations only** (deletion,
directory removal, permanent content removal).

### Required Call Sequence

```
1. auto_check      → Is this auto-approved by workspace policy?
2. check_clearance → Submit proposal; blocks until operator responds via Slack
3. check_diff      → Execute only after status: "approved"
```

### Rules

1. File creation and modification: write directly, then broadcast the change.
2. After every non-destructive file write, call `broadcast` at `info` level with
   `[FILE] {created|modified}: {file_path}` and include the diff or full content.
3. Destructive operations: always route through `auto_check` → `check_clearance` → `check_diff`.
4. One destructive operation per approval — never batch deletions.
5. Set `risk_level: "high"` or `"critical"` for config files, security modules
   (`diff/path_safety.rs`, `policy/`, `slack/events.rs`), or DB schema (`persistence/schema.rs`).
6. Do not retry rejected proposals with the same content.
7. Always branch on `approved`, `rejected`, and `timeout` — never assume approval.

---

## Terminal Command Execution Policy

**Do NOT chain terminal commands.** Run each command as a separate, standalone
invocation and inspect output before proceeding.

### Rules

1. **One command per call.** Never combine with `;`, `&&`, `||`, or `|` except
   for permitted output-redirection exceptions below.
2. **No `cmd /c` wrappers** unless strictly necessary; even then, single command only.
3. **No exit-code echo suffixes.** Don't append `; echo "EXIT: $LASTEXITCODE"`.
4. **Check results between commands.** Inspect output and exit code before continuing.
5. **Always use `pwsh`, never `powershell`.** Use the PowerShell 7+ executable.
6. **Use relative paths for output redirection.** Never absolute paths — they break
   auto-approve regex matching.
7. **Temporary output files go in `logs/`.** Never write to `target/` or the root.

### Permitted Exceptions (output redirection only)

```powershell
cargo test 2>&1 | Out-File logs\test-results.txt
cargo test > logs\test-results.txt 2>&1
some-command | Out-String
```

### Correct

```powershell
cargo check
cargo clippy -- -D warnings
cargo test 2>&1 | Out-File logs\test-results.txt
```

### Incorrect

```powershell
cargo check; cargo clippy; cargo test        # chained — forbidden
cargo fmt && cargo clippy && cargo test      # AND-chained — forbidden
cargo test 2>&1 | Out-File target\out.txt   # wrong output dir — forbidden
```

---

## Development Workflow

1. **Feature specs first**: Every feature MUST have a specification in
   `specs/###-feature-name/spec.md` before implementation begins.
2. **Plan before code**: Implementation plans stored alongside the spec.
3. **Branch per feature**: Branch name matches the spec directory name
   (e.g., `001-mcp-remote-agent-server`).
4. **Contract-first design**: MCP tool schemas defined before implementation.
   Changes to contracts require updating corresponding contract tests.
5. **Commit discipline**: Each commit MUST be coherent and buildable.
   Commit messages follow conventional commits format
   (`feat:`, `fix:`, `docs:`, `test:`).
6. **No dead code**: Placeholder modules MUST be replaced or removed before
   a feature is considered complete.

<!-- BEGIN BEADS INTEGRATION v:1 profile:full hash:d4f96305 -->
## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Dolt-powered version control with native sync
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**

```bash
bd ready --json
```

**Create new issues:**

```bash
bd create "Issue title" --description="Detailed context" -t bug|feature|task -p 0-4 --json
bd create "Issue title" --description="What this issue is about" -p 1 --deps discovered-from:bd-123 --json
```

**Claim and update:**

```bash
bd update <id> --claim --json
bd update bd-42 --priority 1 --json
```

**Complete work:**

```bash
bd close bd-42 --reason "Completed" --json
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task atomically**: `bd update <id> --claim`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" --description="Details about what was found" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`

### Auto-Sync

bd automatically syncs via Dolt:

- Each write auto-commits to Dolt history
- Use `bd dolt push`/`bd dolt pull` for remote sync
- No manual export/import needed!

### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems

For more details, see README.md and docs/QUICKSTART.md.

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

<!-- END BEADS INTEGRATION -->
