# Implementation Plan: Agent Intercom Release

**Branch**: `003-agent-intercom-release` | **Date**: 2026-02-23 | **Spec**: [spec.md](spec.md)  
**Input**: Feature specification from `specs/003-agent-intercom-release/spec.md`

## Summary

Rebrand the project from "monocoque-agent-rc" to "agent-intercom" across all source, configuration, documentation, and tooling. Fix 5 identified Slack notification gaps. Adopt intercom-themed MCP tool names. Create comprehensive product documentation. Build a release pipeline producing cross-platform binaries. Upgrade rmcp from 0.5 to 0.13.0 with full transport layer refactor.

## Technical Context

**Language/Version**: Rust stable, edition 2021  
**Primary Dependencies**: rmcp 0.5 → 0.13.0, axum 0.8, slack-morphism 2.17, sqlx 0.8, tokio 1.37  
**Storage**: SQLite via sqlx (bundled, file-based prod, in-memory tests)  
**Testing**: `cargo test` — unit (15 modules), contract (10 modules), integration (8 modules)  
**Target Platform**: Windows x64, Linux x64, macOS ARM64, macOS Intel  
**Project Type**: Single Rust workspace with two binaries  
**Performance Goals**: No new performance targets; maintain existing response characteristics  
**Constraints**: Zero clippy warnings (`pedantic`), no `unsafe`, no `unwrap`/`expect`, `max_width = 100`  
**Scale/Scope**: ~848 rename occurrences across ~110 files; 9 MCP tool handler refactors; 1 transport layer rewrite

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Safety-First Rust | **PASS** | All changes maintain `#![forbid(unsafe_code)]`, `Result`/`AppError` pattern. No new `unsafe` or `unwrap`/`expect`. |
| II. MCP Protocol Fidelity | **PASS** | All 9 tools remain visible. Tool rename changes only the `name` field, not visibility or behavior. rmcp upgrade preserves protocol compliance. |
| III. Test-First Development | **PASS** | TDD required for notification gap fixes. Rename is mechanical (tests updated alongside source). rmcp upgrade validated by existing contract + integration tests. |
| IV. Security Boundary Enforcement | **PASS** | Keychain service name change is a string constant update. Path security, credential handling, and session binding logic unchanged. |
| V. Structured Observability | **PASS** | No changes to tracing infrastructure. Log messages updated as part of rename. |
| VI. Single-Binary Simplicity | **PASS** | Still two binaries (`agent-intercom`, `agent-intercom-ctl`). No new dependencies except `git-cliff` (CI only, not runtime). |

**Post-Phase 1 re-check**: The rmcp 0.13 upgrade may pull in `schemars ^1.1.0` as a transitive dependency. This is acceptable — it's required by the MCP SDK, not speculative. The `transport-streamable-http-server` feature replaces `transport-sse-server` (not a net-new dependency).

## Project Structure

### Documentation (this feature)

```text
specs/003-agent-intercom-release/
├── spec.md              # Feature specification
├── plan.md              # This file
├── research.md          # Phase 0 research output
├── data-model.md        # Phase 1 data model (minimal — no new entities)
├── quickstart.md        # Phase 1 quickstart guide
├── SCENARIOS.md         # Behavioral matrix
├── tasks.md             # Phased task breakdown
├── checklists/
│   └── requirements.md  # Spec quality checklist
└── contracts/
    └── tool-name-mapping.md  # Old → new tool name reference
```

### Source Code (repository root)

```text
src/
├── config.rs            # Keychain name, env var prefix changes
├── errors.rs            # No changes expected
├── lib.rs               # Crate re-exports (rename)
├── main.rs              # Binary entry point, version embedding
├── diff/                # No changes expected
├── ipc/
│   ├── server.rs        # IPC pipe name update
│   └── socket.rs        # IPC pipe name constant
├── mcp/
│   ├── handler.rs       # Tool names, ServerInfo identity, capabilities
│   ├── sse.rs           # Complete rewrite for rmcp 0.13 Streamable HTTP
│   ├── transport.rs     # Verify stdio still works with rmcp 0.13
│   ├── tools/           # Tool handler file renames (cosmetic) + notification additions
│   └── resources/       # Verify resource types compile with rmcp 0.13
├── models/              # No changes expected
├── orchestrator/
│   └── spawner.rs       # MONOCOQUE_ env var prefix → INTERCOM_
├── persistence/         # No changes expected
├── policy/
│   ├── loader.rs        # .agentrc → .intercom constant
│   └── watcher.rs       # .agentrc → .intercom constant
└── slack/
    ├── commands.rs       # /monocoque → /intercom
    └── blocks.rs         # Verify notification Block Kit builders

ctl/
└── main.rs              # Binary name, IPC name, about text

tests/
├── unit/                # ~15 modules — rename imports
├── contract/            # ~10 modules — rename imports, update tool names
└── integration/         # ~8 modules — rename imports, SSE test rewrites

docs/
├── README.md            # Full rewrite
├── setup-guide.md       # Full update
├── user-guide.md        # Full update with new tool names
├── developer-guide.md   # New file
├── migration-guide.md   # New file
└── REFERENCE.md         # Full update

.github/
├── workflows/
│   └── release.yml      # New release pipeline
├── copilot-instructions.md  # Rename + tool name updates
└── agents/              # Tool name references updated
```

**Structure Decision**: Existing single-project structure maintained. No new directories. The release pipeline adds `.github/workflows/release.yml`. Documentation adds two new files (`developer-guide.md`, `migration-guide.md`).

## Complexity Tracking

| Concern | Impact | Mitigation |
|---|---|---|
| ~848 rename occurrences | High volume, low complexity per change | Phased approach: Cargo → source → tests → docs. `cargo check` after each phase. |
| rmcp 0.13 SSE removal | High complexity (transport rewrite) | Isolated as final phase. Study rmcp examples. Existing contract tests validate behavior. |
| Tool rename + notification fixes simultaneously | Medium risk of merge conflicts | Tool renames in Phase 2, notifications in Phase 3. Separate concerns. |
| Cross-platform release pipeline | Medium — untestable locally for all targets | GitHub Actions matrix build. Test on Windows locally, CI validates others. |

## Design Decisions

### D-001: Rename ordering — Cargo first

The Cargo.toml package name and binary names are changed first because `cargo check` will immediately surface any import path issues. Source files are updated next (identifiers), then tests (imports + assertions), then documentation (prose).

### D-002: Tool files not renamed on disk

The tool handler files (`src/mcp/tools/ask_approval.rs`, etc.) are NOT renamed on disk to match the new tool names. The file names are internal implementation details. Only the `Tool::name` field and the `ToolRouter` key change. This avoids git history disruption.

### D-003: Streamable HTTP replaces SSE with backward compat

The rmcp 0.13 upgrade replaces the `/sse` + `/message` two-endpoint model with a single `/mcp` POST endpoint. For backward compatibility, the axum router will also mount a redirect from `/sse` to `/mcp` with a deprecation header, allowing existing `mcp.json` configurations to work temporarily.

### D-004: No data model changes

This feature introduces no new database tables, no new domain entities, and no schema migrations. All changes are to naming, notifications, transport, and infrastructure. The `data-model.md` artifact documents this explicitly.

### D-005: Version embedding

The binary version is embedded at compile time using `env!("CARGO_PKG_VERSION")`. The `--version` flag in both binaries and the MCP `ServerInfo.version` field all read this value. No runtime version files.

## Phase Summary

| Phase | Focus | Key Deliverables |
|---|---|---|
| 1 | Cargo + core source rename | Compilable codebase under new name |
| 2 | Tool naming + MCP identity | 9 tools renamed, ServerInfo identity set |
| 3 | Slack notification gaps | 5 notification gaps fixed with tests |
| 4 | Test suite update | All tests passing under new names |
| 5 | Documentation | README, guides, migration doc |
| 6 | Release pipeline | GitHub Actions workflow, feature flags |
| 7 | rmcp 0.13 upgrade | Transport rewrite, handler updates |
