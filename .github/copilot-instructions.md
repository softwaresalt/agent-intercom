# agent-intercom Development Guidelines

Last updated: 2026-04-14

agent-intercom is a MCP remote agent server enabling human operators to review and approve AI agent code changes via Slack, with workspace policy enforcement, stall detection, and session lifecycle management.

## Technology Stack

| Layer           | Technology                | Notes                                 |
|-----------------|---------------------------|---------------------------------------|
| Language        | Rust 2021 | (Rust 2021 edition, stable toolchain)          |
| Build           | cargo            | `cargo build`                   |
| Test            | cargo test           | `cargo test`                    |
| Lint            | clippy                | `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`                    |
| Format          | rustfmt             | `cargo fmt --all`                  |
| CI              | GitHub Actions           | GitHub Actions CI with lint, test-unit, test-integration, audit pipeline. Concurrency cancel-in-progress enabled.                          |
| `rmcp` | 0.13 | MCP SDK (ServerHandler, ToolRouter, ToolRoute) |
| `axum` | 0.8 | HTTP/SSE transport (StreamableHttpService on /mcp) |
| `slack-morphism` | 2.17 | Slack Socket Mode client |
| `sqlx` | 0.8 | SQLite async driver (file-based prod, in-memory tests) |
| `diffy` | 0.4 | Unified diff parsing and patch application |
| `interprocess` | 2.0 | IPC named pipes (Windows) / Unix domain sockets |
| `clap` | 4.5 | CLI argument parsing |
| `notify` | 6.1 | Filesystem watcher (policy hot-reload) |
| `tokio` | 1.37 | Async runtime (full feature set) |
| `tokio-util` | 0.7 | CancellationToken for graceful shutdown |
| `keyring` | 3 | OS keychain credential access |
| `reqwest` | 0.13 | HTTP client (rustls) |

## Project Structure

```text
src/
  config.rs               # GlobalConfig, credential loading, TOML parsing
  errors.rs               # AppError enum
  lib.rs                  # Crate root: re-exports GlobalConfig, AppError, Result
  main.rs                 # CLI bootstrap, tokio runtime, server startup
  audit/writer.rs         # AuditLogger trait and file-based implementation
  diff/                   # Unified diff parsing, patch application, atomic writes
  ipc/                    # IPC server for agent-intercom-ctl
  mcp/                    # MCP protocol layer
    handler.rs            # AppState, ToolRouter wiring
    sse.rs                # HTTP/SSE transport (axum)
    transport.rs          # Stdio transport
    tools/                # 10 MCP tool handlers
    resources/            # MCP resource providers
  models/                 # Domain models
  orchestrator/           # Session lifecycle management
  persistence/            # SQLite repository layer
  policy/                 # Workspace auto-approve rules
  slack/                  # Slack Socket Mode integration
ctl/main.rs               # agent-intercom-ctl companion CLI
tests/
  unit/                   # Isolated logic tests
  contract/               # MCP tool response contract verification
  integration/            # End-to-end flows with real SSE/DB
docs/
  adrs/                   # Architecture Decision Records
specs/                    # Feature specifications
config.toml               # Runtime configuration
```

## Commands

```bash
cargo build              # Build
cargo test               # Run all tests
cargo clippy --all-targets -- -D warnings -D clippy::pedantic               # Lint
cargo fmt --all             # Format check
cargo check --all-targets     # Fast compilation check (no codegen)
cargo audit                   # Security audit of dependencies
```

## Code Style and Conventions

### Error Handling

* All fallible operations return `Result<T, AppError>` (type alias in `src/errors.rs`)
* `AppError` variants: Config, Db, Slack, Mcp, Diff, Policy, Ipc, PathViolation, PatchConflict, NotFound, Unauthorized, AlreadyConsumed, Io
* Map external errors via `From` impls or `.map_err()` — never `unwrap()` or `expect()`
* Error messages are lowercase and do not end with a period

### Naming

* Module files: `src/{module}/mod.rs` pattern for directories
* Struct IDs: prefixed strings (`task:uuid`, `context:uuid`)
* Status values: `snake_case` (`todo`, `in_progress`, `done`, `blocked`)
* Default visibility: `pub(crate)` unless public API

### Documentation

* All public items require `///` doc comments
* Module-level `//!` doc comments on every `mod.rs` or standalone module file

### Testing

* TDD required: write tests first, verify they fail, then implement
* Test tiers in `tests/` directory:
Three tiers: `unit/` for isolated logic tests, `contract/` for MCP tool response contract verification, `integration/` for end-to-end flows with real SSE/DB. In-memory SQLite for all tests.

## Search Strategy

Use available workspace search tools before falling back to file-based search
(grep, glob, view). Indexed search returns precise results with minimal token
cost. File-based tools read raw content into the context window, consuming
tokens proportional to file size.

**Search tool preference order:**

1. When the `agent-engram` capability pack is enabled and reachable: `unified_search`, `query_memory`, `map_code`, `list_symbols`, `impact_analysis`, `query_graph`
2. Otherwise use workspace-indexed tools (if available): semantic search, symbol lookup, call graphs
3. File-based fallback: grep, glob, view — only when indexed results are insufficient

## Session Memory Requirements

* Working agent sessions MUST persist output to `docs/memory/` before the session ends — do NOT rely on built-in AI assistant memory features, which write to their own managed locations.
* Content to capture: task IDs completed, files modified, decisions and rationale, failed approaches, open questions, and next steps.
* File convention: `docs/memory/{YYYY-MM-DD}/{descriptive-slug}-memory.md`
* After writing memory, invoke the **compact-context** skill to consolidate stale checkpoints and finalize decided-plans. This is a mandatory workflow step, not advisory.
* If context has grown from loading multiple skill definitions mid-session, consider invoking **compact-context** proactively before hitting hard thresholds.

## Foundational Protocols

| Protocol | Location | When |
|---|---|---|
| **Circuit Breaker** | `.github/instructions/circuit-breaker.instructions.md` | All retry loops and failure handling |
| **Concurrency Control** | `.github/instructions/concurrency.instructions.md` | Multi-agent or human+agent concurrent edits |
| **Skill Discovery** | `scripts/search.ps1` / `scripts/search.sh` | Finding capabilities by keyword (Primitive 6) |

## Optional Capability Packs

### agent-intercom

When the workspace enabled the `agent-intercom` capability pack:

* verify the intercom server / tool surface is reachable before depending on remote approval or operator steering
* call heartbeat / ping at session start and keep it alive during long-running work
* broadcast major workflow transitions so the operator can observe planning, build, review, verification, and closure progress
* route destructive terminal commands and destructive file operations through the intercom approval workflow
* use transmit / standby flows when blocked on operator clarification or when intentionally pausing for instructions
* if the intercom service is unreachable, warn that remote visibility is degraded and avoid pretending approval or operator awareness exists

### agent-engram

When the workspace enabled the `agent-engram` capability pack:

* verify the engram daemon / MCP surface is reachable before depending on indexed lookup
* prefer engram tools for conceptual search, symbol discovery, call-graph lookup, impact analysis, and workspace-memory retrieval
* verify the workspace binding state before relying on results; if the daemon auto-binds the workspace, prefer status checks over repeated rebinding
* use `sync_workspace` or the equivalent freshness operation when code changed outside the expected indexing flow
* if semantic search is unavailable or degraded, fall back to `list_symbols` + `map_code` + `impact_analysis` before resorting to broad file scans
* treat `.engram/` generated artifacts as tool-managed state rather than files to hand-edit casually

### backlogit

When the workspace enabled the `backlogit` capability pack:

* verify the backlogit MCP / CLI surface is reachable before depending on queue, dependency, memory, or traceability operations
* prefer backlogit query operations for targeted state lookup instead of reading many backlog markdown files into context
* use backlogit queue and dependency operations when available rather than inferring execution order from prose alone
* write concise memory summaries and checkpoints through backlogit operations at task and session boundaries when supported
* append significant task comments and associate commits with task IDs for execution traceability when those operations are available
* if backlog content was edited outside the normal mutation flow, refresh the backlogit index before relying on query results

### browser-verification

When the workspace enabled the `browser-verification` capability pack:

* verify the target server or preview environment is reachable before launching browser work
* choose headed vs headless mode intentionally and record the reason
* derive browser routes from changed pages, components, or affected user journeys
* treat OAuth, email, SMS, payments, CAPTCHAs, or other external flows as explicit human checkpoints
* carry browser findings into runtime verification and operational closure rather than leaving them as informal notes

### continuous-learning

When the workspace enabled the `continuous-learning` capability pack:

* store observation state under `.autoharness/continuous-learning/`
* keep hook capture optional and environment-specific; manual capture is still valid
* use `observe` to capture recurring workflow signals, `learn` to infer instincts, and `evolve` to promote mature patterns into `learned-*` artifacts
* do not harden a rule into a learned instruction or skill until it has enough corroborating observations to justify the promotion
* treat learned artifacts as explicit repository knowledge rather than invisible prompt-only behavior

### strict-safety

When the workspace enabled the `strict-safety` capability pack:

* follow `.github/instructions/strict-safety.instructions.md`
* express risky work as `ProposedAction` entries with `ActionRisk` and `ActionResult`
* require explicit approval before destructive actions and prefer approval for high-blast-radius actions
* keep risky action records visible in plan hardening, review, runtime verification, and operational closure

### release-observability

When the workspace enabled the `release-observability` capability pack:

* follow `.github/instructions/release-observability.instructions.md`
* produce monitoring plans with SLIs, dashboards, baselines, and alert thresholds before merge
* complete pre-deploy audit checklists for runtime, migration, or rollout-risk changes
* define explicit post-deploy observation windows with owner and duration
* declare rollback triggers with named metrics and thresholds
* carry all release-observability artifacts into operational closure

### adversarial-review

When the workspace enabled the `adversarial-review` capability pack:

* follow `.github/instructions/adversarial-review.instructions.md`
* escalate from standard review when 3+ P0/P1 findings appear or the work is security-sensitive
* dispatch parallel reviewer instances across different model tiers for cross-model diversity
* assemble consensus-weighted findings (HIGH / MEDIUM / LOW confidence)
* treat HIGH-confidence P0/P1 findings as gate-blocking
* feed remediation queue entries into backlog

Generated by autoharness | Template: copilot-instructions.md.tmpl
