# Implementation Plan: Intercom ACP Server

**Branch**: `005-intercom-acp-server` | **Date**: 2026-02-28 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/005-intercom-acp-server/spec.md`

## Summary

Add Agent Client Protocol (ACP) server mode to agent-intercom so the server can actively connect to and control headless agent processes, alongside the existing passive MCP server mode. The implementation introduces an `AgentDriver` trait abstracting protocol differences, a `--mode` CLI flag for mode selection, workspace-to-channel mapping in `config.toml` (replacing per-workspace `channel_id` query parameters), per-session Slack threading, and multi-session channel routing with `thread_ts` disambiguation.

## Technical Context

**Language/Version**: Rust stable, edition 2021
**Primary Dependencies**: rmcp 0.13, axum 0.8, slack-morphism 2.17, sqlx 0.8, tokio 1.37, tokio-util 0.7 (LinesCodec for ACP stream framing), notify 6.1, interprocess 2.0, clap 4.5, serde/serde_json 1.0, chrono 0.4, uuid 1.7
**Storage**: SQLite via sqlx 0.8 (file-based prod, in-memory tests). Schema additions: `protocol_mode`, `channel_id`, `thread_ts`, `connectivity_status`, `last_activity_at`, `restart_of` columns on `session` table. Workspace mappings loaded from `config.toml` into in-memory HashMap (not persisted to SQLite).
**Testing**: cargo test (unit/, contract/, integration/ tiers). TDD required.
**Target Platform**: Windows (primary), Linux/macOS (secondary)
**Project Type**: Single workspace, two binaries (agent-intercom, agent-intercom-ctl)
**Performance Goals**: ACP session start < 10s from Slack command to first status update. Stream parsing handles sustained throughput without backpressure. Workspace channel resolution O(1) via HashMap.
**Constraints**: No new external dependencies beyond tokio-util codec features (already a dependency). Single-binary simplicity. All paths validated within workspace root.
**Scale/Scope**: Single operator, 1-5 concurrent agent sessions typical. Multiple workspaces per server instance.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Safety-First Rust | Pass | All new code in Rust stable. No unsafe. Result/AppError pattern for all fallible ops. Clippy pedantic. AgentDriver trait uses async-safe patterns (no `unwrap`/`expect`). |
| II. MCP Protocol Fidelity | Pass | MCP mode unchanged — all 9 tools remain unconditionally visible. ACP mode uses a separate code path; MCP protocol compliance unaffected. |
| III. Test-First Development | Pass | TDD required. New unit tests for driver trait, stream codec, workspace mapping. Contract tests for extended session model. Integration tests for ACP lifecycle. |
| IV. Security Boundary Enforcement | Pass | ACP spawned processes inherit workspace root constraint. Workspace mapping only resolves channels — no path expansion. Session owner binding enforced for ACP sessions same as MCP. |
| V. Structured Observability | Pass | ACP stream events emit tracing spans. Session lifecycle transitions logged. Workspace mapping resolution traced. |
| VI. Single-Binary Simplicity | Pass | No new binaries. tokio-util codec features already available. workspace_mapping stored in config.toml (parsed into in-memory HashMap). No new databases or caches. |
| VII. CLI Workspace Containment | Pass | ACP spawned processes use `kill_on_drop(true)` and workspace root constraint. No file operations outside cwd. |

No violations. All features align with constitution principles.

## Project Structure

### Documentation (this feature)

```text
specs/005-intercom-acp-server/
├── plan.md              # This file
├── research.md          # Phase 0: research findings
├── data-model.md        # Phase 1: entity models
├── quickstart.md        # Phase 1: implementation quickstart
├── contracts/           # Phase 1: driver trait and stream contracts
│   ├── agent-driver.md
│   ├── acp-stream.md
│   └── workspace-mapping.md
└── tasks.md             # Phase 2: task breakdown (via /speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── config.rs               # + WorkspaceMappingConfig, workspace_mappings field
├── errors.rs               # + AppError::Acp variant for stream errors
├── main.rs                 # + --mode flag (Mode::Mcp / Mode::Acp), ACP startup path
├── driver/                 # NEW module: protocol-agnostic agent driver
│   ├── mod.rs              # AgentDriver trait definition + AgentEvent enum
│   ├── mcp_driver.rs       # MCP implementation (wraps existing oneshot pattern)
│   └── acp_driver.rs       # ACP implementation (session-indexed stream writers)
├── acp/                    # NEW module: ACP stream handling
│   ├── mod.rs              # ACP module root
│   ├── codec.rs            # LinesCodec-based stream framing
│   ├── reader.rs           # Read task: parse incoming agent messages → AgentEvent
│   ├── writer.rs           # Write task: serialize outbound responses
│   └── spawner.rs          # Process spawning, stdio capture, env isolation
├── mcp/
│   ├── handler.rs          # + AgentDriver in AppState, thread_ts routing
│   ├── sse.rs              # + workspace_id query param, deprecation warning for channel_id
│   └── tools/
│       ├── heartbeat.rs    # unchanged (steering delivery already in 004)
│       └── recover_state.rs # unchanged (inbox delivery already in 004)
├── models/
│   └── session.rs          # + thread_ts, channel_id, protocol_mode, connectivity_status, last_activity_at, restart_of fields
├── orchestrator/
│   ├── stall_detector.rs   # + ACP stream activity monitoring variant
│   └── session_manager.rs  # + ACP session start/stop lifecycle
├── persistence/
│   ├── schema.rs           # + ALTER session table (6 new columns + indexes)
│   └── session_repo.rs     # + find_by_channel, find_by_channel_and_thread queries
├── slack/
│   ├── blocks.rs           # + session thread root message builder
│   ├── client.rs           # + thread_ts parameter on all session message sends
│   ├── commands.rs         # + workspace_id to channel resolution in session-start
│   ├── events.rs           # + thread_ts extraction for routing + owner verification
│   └── handlers/
│       └── steer.rs        # + channel_id/thread_ts scoped session lookup (RI-04 fix)
└── ipc/
    └── server.rs           # unchanged (ACP ctl subcommands deferred to future feature)

ctl/
└── main.rs                 # unchanged (ACP ctl subcommands deferred to future feature)

tests/
├── unit/
│   ├── driver_trait_tests.rs       # NEW: AgentDriver trait behavior
│   ├── acp_codec_tests.rs          # NEW: stream framing / parsing
│   ├── workspace_mapping_tests.rs  # NEW: config resolution
│   └── session_routing_tests.rs    # NEW: channel + thread_ts routing
├── contract/
│   ├── session_contract_tests.rs   # UPDATED: protocol_mode, thread_ts fields
│   └── driver_contract_tests.rs    # NEW: driver response contracts
└── integration/
    ├── acp_lifecycle_tests.rs      # NEW: end-to-end ACP session
    ├── workspace_routing_tests.rs  # NEW: multi-workspace channel routing
    └── thread_routing_tests.rs     # NEW: per-session Slack threading
```

**Structure Decision**: Adds two new top-level source modules (`src/driver/`, `src/acp/`) following established patterns. The driver module owns the protocol abstraction; the acp module owns stream mechanics. Existing modules are extended minimally — session model gains 3 fields, config gains workspace mappings, SSE gains workspace_id resolution.

## Complexity Tracking

No constitution violations to justify.
