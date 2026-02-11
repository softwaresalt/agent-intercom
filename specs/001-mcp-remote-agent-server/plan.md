# Implementation Plan: MCP Remote Agent Server

**Branch**: `001-mcp-remote-agent-server` | **Date**: 2026-02-10 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/001-mcp-remote-agent-server/spec.md`

## Summary

Build an MCP server that provides remote I/O capabilities to local AI agents via Slack, enabling asynchronous code review, approval workflows, session orchestration, continuation prompt forwarding, and stall detection with auto-nudge — all operable from a mobile device. The server uses `rmcp` 0.5 for the MCP protocol, `slack-morphism` for Slack Socket Mode, `axum` 0.8 for HTTP/SSE transport, SurrealDB (embedded, RocksDB backend) for persistence, and `diffy` for diff application. Credentials are loaded from the OS keychain with environment variable fallback. The server supports multiple concurrent workspaces, each identified by its root path specified per-session.

## Technical Context

**Language/Version**: Rust (stable, edition 2021)
**Primary Dependencies**: `rmcp` 0.5, `slack-morphism` 2.17, `axum` 0.8, `tokio` 1.37, `serde`/`serde_json`, `diffy` 0.4, `notify` 6.1, `interprocess` 2.0, `clap` 4.5, `tracing`/`tracing-subscriber` 0.3
**Storage**: SurrealDB 1.5 embedded (RocksDB backend for production, in-memory for tests)
**Credential Storage**: OS keychain (Windows Credential Manager / macOS Keychain) as primary, environment variables as fallback (FR-036)
**Observability**: Structured tracing spans to stderr via `tracing-subscriber` covering tool calls, Slack interactions, stall events, session lifecycle (FR-037)
**Testing**: `cargo test` (unit), `cargo test --test integration` (integration), `cargo test --test contract` (contract)
**Target Platform**: Local workstation (Windows, macOS, Linux)
**Project Type**: Single Rust binary + CLI companion (`monocoque-ctl`)
**Performance Goals**: Approval response delivered to agent within 5s of operator tap (SC-001), file write within 2s (SC-002), server startup <10s (SC-010), stall detection within configured threshold (default 5min, SC-011)
**Constraints**: Single workstation deployment, up to 3 concurrent sessions (configurable), 24-hour unattended operation (SC-003), multi-workspace support (workspace root per-session)
**Scale/Scope**: 1 operator per session, multiple workspaces, up to 3 concurrent sessions, 30-day data retention with auto-purge (FR-035)
**Data Retention**: Time-based auto-purge — all persisted data purged 30 days after session termination, configurable (FR-035)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Validated against `constitution.md` v1.0.0 (6 principles, ratified 2026-02-10).

| Principle | Status | Evidence |
|-----------|--------|----------|
| I. Safety-First Rust | PASS | Rust stable edition 2021; `#![forbid(unsafe_code)]` enforced in `src/lib.rs` (T100); Clippy pedantic with `unwrap_used`/`expect_used` deny configured; all errors use `Result`/`AppError` pattern (T002) |
| II. MCP Protocol Fidelity | PASS | `rmcp` 0.5 SDK; all tools unconditionally visible (FR-032/T029); `monocoque/nudge` via standard notification mechanism (FR-027/T050) |
| III. Test-First Development | PASS | Test tasks precede implementation tasks in every phase; contract tests validate MCP tool schemas; integration tests validate cross-module flows; unit tests validate isolated logic |
| IV. Security Boundary Enforcement | PASS | Path validation via `validate_path` (T034/FR-006); command allowlist (FR-014/T078); OS keychain credentials (FR-036/T006); session owner binding (FR-013/T027, T067) |
| V. Structured Observability | PASS | Tracing spans in every phase via `tracing-subscriber` (FR-037); JSON + human-readable modes (T003); spans cover tool calls, Slack interactions, stall events, session lifecycle |
| VI. Single-Binary Simplicity | PASS | Two binaries (`monocoque-agent-rem` + `monocoque-ctl`); SurrealDB embedded sole persistence; workspace dependencies in `Cargo.toml`; all dependencies justified by concrete requirements |

**Gate status**: PASS — all 6 principles satisfied. No violations to document.

## Project Structure

### Documentation (this feature)

```text
specs/001-mcp-remote-agent-server/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── mcp-tools.json   # MCP tool definitions
│   └── mcp-resources.json # MCP resource/notification/slash command definitions
├── checklists/          # Test checklists
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
src/
├── main.rs              # Entry point, CLI args, server bootstrap
├── lib.rs               # Library root, re-exports
├── config.rs            # GlobalConfig parsing (TOML + keychain + env fallback)
├── models/              # Domain types (Session, ApprovalRequest, Checkpoint, etc.)
├── mcp/                 # MCP server handler, tool implementations
├── slack/               # Slack Socket Mode client, Block Kit rendering, interactions
│   └── handlers/        # Slack interaction handlers (approval, nudge, prompt)
├── persistence/         # SurrealDB layer (schema, queries, retention purge)
├── diff/                # Diff parsing, application, file safety
├── policy/              # Workspace policy loading, hot-reload, auto-approve evaluation
├── orchestrator/        # Session lifecycle, stall detection, process spawning
└── ipc/                 # Local IPC (named pipe / Unix socket) for monocoque-ctl

ctl/
└── main.rs              # monocoque-ctl local CLI binary

tests/
├── contract/            # Contract validation tests
├── integration/         # Integration tests
└── unit/                # Unit tests
```

**Structure Decision**: Single Rust project with two binary targets (`monocoque-agent-rem` and `monocoque-ctl`). Domain modules organized by capability area. This matches the existing codebase layout.

## Complexity Tracking

No constitution violations identified. All design decisions comply with the 6 constitution principles. If violations are discovered during implementation, they MUST be documented here with the specific principle violated, the justification, and the simpler alternative that was rejected.
