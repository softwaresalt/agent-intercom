<!--
Sync Impact Report
==================
Version change: 0.0.0 → 1.0.0
Bump rationale: Initial adoption — new constitution (MAJOR)

Added principles:
  - I. Safety-First Rust
  - II. MCP Protocol Fidelity
  - III. Test-First Development (NON-NEGOTIABLE)
  - IV. Security Boundary Enforcement
  - V. Structured Observability
  - VI. Single-Binary Simplicity

Added sections:
  - Technical Constraints
  - Development Workflow

Removed sections: (none — initial version)

Templates requiring updates:
  - .specify/templates/plan-template.md ✅ aligned (Constitution Check
    section already references constitution gates)
  - .specify/templates/spec-template.md ✅ aligned (no constitution-
    specific references required)
  - .specify/templates/tasks-template.md ✅ aligned (phase structure
    and checkpoint pattern compatible)
  - .specify/templates/checklist-template.md ✅ aligned (generic
    template, no constitution-specific tokens)
  - .specify/templates/agent-file-template.md ✅ aligned (no
    constitution-specific tokens)

Follow-up TODOs: (none)
-->

# monocoque-agent-rem Constitution

## Core Principles

### I. Safety-First Rust

All production code MUST be written in Rust (stable toolchain,
edition 2021). `unsafe` code is forbidden at the workspace level
(`#![forbid(unsafe_code)]`). Clippy pedantic lints MUST pass with
zero warnings. `unwrap()` and `expect()` are denied; all fallible
operations MUST use the `Result`/`AppError` pattern defined in
`src/lib.rs`.

**Rationale**: The server manages file-system writes, credential
access, and long-lived network connections on behalf of autonomous
agents. Memory safety and explicit error handling are non-negotiable
to prevent data loss, security breaches, or silent failures in
unattended operation.

### II. MCP Protocol Fidelity

The server MUST implement the Model Context Protocol via the `rmcp`
0.5 SDK. All MCP tools MUST be unconditionally visible to every
connected agent regardless of configuration. Tools called in
inapplicable contexts MUST return a descriptive error rather than
being hidden. Custom notifications (e.g., `monocoque/nudge`) MUST
use the standard MCP notification mechanism.

**Rationale**: Consistent tool surface ensures agents can discover
capabilities without conditional logic. Protocol compliance
guarantees interoperability with any MCP-compatible client (Claude
Code, GitHub Copilot CLI, Cursor, VS Code).

### III. Test-First Development (NON-NEGOTIABLE)

Every feature MUST have tests written before implementation code.
The test directory structure (`tests/contract/`, `tests/integration/`,
`tests/unit/`) MUST be maintained. Contract tests validate MCP tool
input/output schemas. Integration tests validate cross-module
interactions. Unit tests validate isolated logic. All tests MUST
pass via `cargo test` before any code is merged.

**Rationale**: The server operates unattended for extended periods.
Regressions in approval flows, diff application, or stall detection
can silently corrupt agent sessions. Test-first discipline catches
failures before they reach production.

### IV. Security Boundary Enforcement

All file-system operations MUST resolve within the configured
workspace root. Path traversal attempts MUST be rejected with
`AppError::PathViolation`. Remote command execution MUST be
restricted to the explicit allowlist in the global configuration.
Sensitive credentials (Slack tokens) MUST be loaded from the OS
keychain with environment-variable fallback; credentials MUST NOT
be stored in plaintext configuration files. Each agent session MUST
be bound to exactly one Slack user (owner) at creation time — only
the session owner may interact with that session.

**Rationale**: The server writes files and executes commands on
behalf of remote operators via Slack. Without strict boundaries, a
compromised or misbehaving agent could access arbitrary files,
execute arbitrary commands, or allow unauthorized users to
manipulate sessions.

### V. Structured Observability

All significant operations MUST emit structured tracing spans to
stderr via `tracing-subscriber`. Span coverage MUST include: MCP
tool call execution, Slack API interactions, stall detection events,
and session lifecycle transitions. Log output MUST support both
human-readable and JSON formats via `tracing-subscriber` features.
No external metrics endpoint or telemetry collector is required for
v1.

**Rationale**: The server runs as a background service for hours or
days. When something goes wrong during unattended operation,
structured traces are the primary diagnostic tool. Without them,
debugging stall detection, approval timeouts, or Slack connectivity
issues would require reproducing the exact scenario.

### VI. Single-Binary Simplicity

The project MUST produce a single workspace with two binaries
(`monocoque-agent-rem` and `monocoque-ctl`). Dependencies MUST be
managed via `Cargo.toml` workspace dependencies. New dependencies
MUST be justified by a concrete requirement — do not add libraries
speculatively. Prefer the standard library over external crates
when the standard library solution is adequate. SurrealDB in
embedded mode is the sole persistence layer; do not introduce
additional databases or caches.

**Rationale**: Operational simplicity is critical for a tool that
developers install on personal workstations. Every additional
dependency increases build time, attack surface, and maintenance
burden. The single-binary model ensures deployment is a single
file copy.

## Technical Constraints

- **Language**: Rust stable, edition 2021
- **Async runtime**: Tokio (full features)
- **MCP SDK**: `rmcp` 0.5 with `server`, `transport-sse-server`,
  and `transport-io` features
- **HTTP/SSE**: Axum 0.8
- **Slack**: `slack-morphism` with Socket Mode
- **Persistence**: SurrealDB embedded (RocksDB for production,
  in-memory for tests)
- **Diff/Patch**: `diffy` 0.4
- **File watching**: `notify` 6.x
- **Formatting**: `rustfmt.toml` with `max_width = 100`,
  edition 2021
- **Linting**: `cargo clippy` with pedantic deny,
  `unwrap_used` deny, `expect_used` deny
- **Build verification**: `cargo test && cargo clippy` MUST pass
  before merge
- **License**: Apache 2.0

## Development Workflow

1. **Feature specs first**: Every feature MUST have a specification
   in `specs/###-feature-name/spec.md` before implementation begins.
2. **Plan before code**: Implementation plans MUST be generated via
   the speckit workflow (`spec → plan → tasks`) and stored alongside
   the spec.
3. **Branch per feature**: Each feature MUST be developed on a
   dedicated branch matching the spec directory name
   (e.g., `001-mcp-remote-agent-server`).
4. **Contract-first design**: MCP tool schemas and data models MUST
   be defined in contract documents before implementation. Changes
   to contracts require updating corresponding contract tests.
5. **Commit discipline**: Each commit MUST represent a coherent,
   buildable change. Commit messages MUST follow conventional
   commits format (e.g., `feat:`, `fix:`, `docs:`, `test:`).
6. **No dead code**: Placeholder modules (e.g., `//! placeholder`)
   MUST be replaced with real implementations or removed before a
   feature is considered complete.

## Governance

This constitution supersedes all other development practices for
the monocoque-agent-rem project. All code reviews and automated
checks MUST verify compliance with these principles.

- **Amendments**: Any change to this constitution MUST be documented
  with a version bump, rationale, and sync impact report. Principle
  removals or redefinitions require a MAJOR version bump. New
  principles or material expansions require MINOR. Clarifications
  and wording fixes require PATCH.
- **Compliance review**: Every implementation plan MUST include a
  "Constitution Check" section (per the plan template) that maps
  the proposed work against these principles and documents any
  justified violations in the Complexity Tracking table.
- **Conflict resolution**: When a principle conflicts with a
  practical implementation need, the conflict MUST be documented
  in the plan's Complexity Tracking table with the specific
  principle violated, the justification, and the simpler
  alternative that was rejected.

**Version**: 1.0.0 | **Ratified**: 2026-02-10 | **Last Amended**: 2026-02-10
