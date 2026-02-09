# Implementation Plan: MCP Remote Agent Server

**Branch**: `001-mcp-remote-agent-server` | **Date**: 2026-02-09 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-mcp-remote-agent-server/spec.md`

## Summary

Build a standalone MCP server in Rust that provides remote I/O capabilities to local AI agents via Slack. The server bridges agentic IDEs (Claude Code, GitHub Copilot CLI, Cursor, VS Code) with a remote operator's Slack mobile app, enabling asynchronous code review/approval workflows, diff application, continuation prompt forwarding, stall detection with auto-nudge, session orchestration, and workspace auto-approve policies — all without requiring the operator to be physically present at the workstation.

## Technical Context

**Language/Version**: Rust (stable, edition 2021)
**Primary Dependencies**: `rmcp` 0.5 (official MCP SDK), `slack-morphism` (Slack Socket Mode), `axum` 0.8 (HTTP/SSE transport), `tokio` (async runtime), `serde`/`serde_json`, `diffy` 0.4 (diff/patch), `notify` (fs watcher), `tracing`/`tracing-subscriber`
**Storage**: SurrealDB embedded (`kv-rocksdb` for persistence, `kv-mem` for tests) — session state, approval requests, checkpoints
**Testing**: `cargo test` — unit tests with in-memory SurrealDB, integration tests with stdio/SSE transport, contract tests against MCP tool schemas
**Target Platform**: Cross-platform (Linux, macOS, Windows) — local workstation daemon
**Project Type**: Single project — one Rust binary (`monocoque-agent-rem`) plus one lightweight CLI binary (`monocoque-ctl`)
**Performance Goals**: Server startup < 10s, approval response relay < 5s, stall detection within configured threshold (default 5 min), concurrent sessions ≤ 3
**Constraints**: No inbound firewall ports (Slack Socket Mode is outbound-only), all file ops scoped to workspace root, < 200 MB memory at steady state
**Scale/Scope**: Single operator, 1–3 concurrent agent sessions, 1 Slack workspace

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

The project constitution (`.specify/memory/constitution.md`) is in template state with no concrete principles defined. No gates to enforce. Plan proceeds without constitution violations.

> **Status**: PASS (no active constitution constraints)

## Project Structure

### Documentation (this feature)

```text
specs/001-mcp-remote-agent-server/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   ├── mcp-tools.json   # MCP tool schemas (JSON-RPC)
│   └── mcp-resources.json # MCP resource schemas
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── main.rs              # Server entry point, transport setup, signal handling
├── config.rs            # config.toml parsing, global settings
├── models/              # Domain entities (ApprovalRequest, Session, Checkpoint, etc.)
│   ├── mod.rs
│   ├── approval.rs
│   ├── session.rs
│   ├── checkpoint.rs
│   ├── prompt.rs
│   ├── stall.rs
│   └── policy.rs
├── mcp/                 # MCP server layer (tool handlers, resource providers)
│   ├── mod.rs
│   ├── server.rs        # ServerHandler impl, tool router
│   ├── tools/           # Individual tool implementations
│   │   ├── mod.rs
│   │   ├── ask_approval.rs
│   │   ├── accept_diff.rs
│   │   ├── check_auto_approve.rs
│   │   ├── forward_prompt.rs
│   │   ├── remote_log.rs
│   │   ├── recover_state.rs
│   │   ├── set_operational_mode.rs
│   │   ├── wait_for_instruction.rs
│   │   └── heartbeat.rs
│   └── resources/       # MCP resource implementations
│       ├── mod.rs
│       └── slack_channel.rs
├── slack/               # Slack Bridge Layer (Socket Mode, Block Kit)
│   ├── mod.rs
│   ├── client.rs        # WebSocket lifecycle, reconnection
│   ├── events.rs        # Interaction event handlers (button presses, modals)
│   ├── blocks.rs        # Block Kit message builders
│   └── commands.rs      # Slash command router and dispatcher
├── persistence/         # SurrealDB embedded storage
│   ├── mod.rs
│   ├── db.rs            # Connection, schema init
│   ├── approval_repo.rs
│   ├── session_repo.rs
│   └── checkpoint_repo.rs
├── orchestrator/        # Session lifecycle management
│   ├── mod.rs
│   ├── session_manager.rs
│   ├── stall_detector.rs
│   └── spawner.rs       # Host CLI process spawning
├── policy/              # Auto-approve policy evaluator
│   ├── mod.rs
│   ├── evaluator.rs
│   └── watcher.rs       # notify-based hot-reload
├── diff/                # Diff applicator module
│   ├── mod.rs
│   └── applicator.rs
├── ipc/                 # Local IPC control layer
│   ├── mod.rs
│   └── socket.rs        # Named pipe / Unix domain socket
└── lib.rs               # Shared types, re-exports

ctl/
└── main.rs              # monocoque-ctl CLI binary

tests/
├── contract/            # MCP tool schema validation
├── integration/         # End-to-end with mock Slack
└── unit/                # Module-level unit tests
```

**Structure Decision**: Single Rust workspace with two binary targets (`monocoque-agent-rem` server and `monocoque-ctl` CLI). The `src/` tree follows a modular architecture matching the six core modules defined in the technical spec (MCP Server, Slack Bridge, Session Manager, Local Control, Registry Dispatcher, Diff Applicator) plus stall detection and policy evaluation. Tests are split by scope.

## Complexity Tracking

> No constitution violations to justify — constitution is in template state.
