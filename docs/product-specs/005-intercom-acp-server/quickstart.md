# Quickstart: Intercom ACP Server

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## What This Feature Does

Adds an Agent Client Protocol (ACP) mode to agent-intercom where the server actively connects to and controls headless agent processes, alongside the existing passive MCP server mode. Also introduces workspace-to-channel mapping, per-session Slack threading, and multi-session channel routing.

## Key Architectural Decisions

1. **AgentDriver trait** — protocol-agnostic abstraction; Slack handlers call trait methods regardless of MCP/ACP
2. **NDJSON over stdio** — ACP uses line-delimited JSON for stream communication with agent processes
3. **Workspace mappings in config.toml** — centralized workspace-to-channel mapping replaces per-workspace `channel_id` query parameters
4. **Session threading** — each session owns a Slack thread via `thread_ts` on the session model
5. **Channel + thread routing** — session lookup scoped by `channel_id` and `thread_ts` to prevent cross-session misrouting

## Implementation Order

### Phase 1: Foundation
- `AgentDriver` trait and `AgentEvent` enum in `src/driver/`
- `McpDriver` wrapping existing oneshot pattern
- Session model additions (`protocol_mode`, `channel_id`, `thread_ts`)
- Schema migration for new columns
- Session repo: `find_by_channel`, `find_by_channel_and_thread` queries

### Phase 2: Workspace Mapping
- `WorkspaceMapping` config parsing in `config.rs`
- `workspace_id` query parameter in SSE middleware
- Backward compatibility for `channel_id` query parameter
- Hot-reload of workspace mappings via `notify` watcher

### Phase 3: Slack Threading
- `thread_ts` propagation through `SlackService`
- Session thread root message on first Slack post
- Thread-scoped button and modal interactions
- Multi-session routing fix (RI-04)

### Phase 4: ACP Stream
- ACP codec (`LinesCodec` wrapper) in `src/acp/codec.rs`
- Stream reader task: parse inbound → `AgentEvent`
- Stream writer task: serialize outbound responses
- `AcpDriver` implementation of `AgentDriver` trait

### Phase 5: ACP Session Lifecycle
- `--mode` CLI flag in `src/main.rs`
- ACP spawner: process launch + stdio capture
- ACP session start from Slack (`/intercom session-start`)
- Process exit monitoring → session termination
- ACP stall detection adaptation

### Phase 6: Integration & Polish
- Offline message queuing (extend 004 inbox)
- End-to-end integration tests
- Config documentation updates
- Migration guide for `channel_id` → `workspace_id`

## Files to Create

| File | Purpose |
|------|---------|
| `src/driver/mod.rs` | `AgentDriver` trait, `AgentEvent` enum |
| `src/driver/mcp_driver.rs` | MCP implementation of `AgentDriver` |
| `src/driver/acp_driver.rs` | ACP implementation of `AgentDriver` |
| `src/acp/mod.rs` | ACP module root |
| `src/acp/codec.rs` | NDJSON codec for stream framing |
| `src/acp/reader.rs` | Inbound stream parser |
| `src/acp/writer.rs` | Outbound stream serializer |
| `src/acp/spawner.rs` | Agent process spawning and stdio capture |

## Files to Modify

| File | Changes |
|------|---------|
| `src/main.rs` | Add `--mode` CLI flag, ACP startup branch |
| `src/config.rs` | Add `WorkspaceMapping` config, `[[workspace]]` parsing |
| `src/errors.rs` | Add `AppError::Acp(String)` variant |
| `src/models/session.rs` | Add `protocol_mode`, `channel_id`, `thread_ts` fields |
| `src/persistence/schema.rs` | ALTER TABLE for new session columns + indexes |
| `src/persistence/session_repo.rs` | New query methods for channel/thread routing |
| `src/mcp/handler.rs` | Wire `AgentDriver` into `AppState` |
| `src/mcp/sse.rs` | Parse `workspace_id` query param, deprecation warning |
| `src/slack/client.rs` | Add `thread_ts` parameter to message posting |
| `src/slack/events.rs` | Extract `thread_ts` for routing |
| `src/slack/handlers/steer.rs` | Channel-scoped session lookup (RI-04 fix) |
| `src/orchestrator/stall_detector.rs` | Stream activity monitoring for ACP |

## Testing Strategy

- **Unit tests**: Driver trait behavior, codec parsing, workspace resolution, session routing
- **Contract tests**: Session model with new fields, driver response shapes, stream message format
- **Integration tests**: Full ACP lifecycle, multi-workspace routing, Slack threading
- **Regression**: All existing MCP tests must pass unchanged

## Dependencies on Feature 004

- Steering queue (`steering_message` table, `/intercom steer` command) — used for offline message queuing
- Task inbox (`task_inbox` table) — extended for ACP session cold-start
- Policy hot-reload (`PolicyWatcher`) — pattern reused for workspace mapping hot-reload
- Audit logging (`AuditLogger`) — ACP events emitted to audit log
