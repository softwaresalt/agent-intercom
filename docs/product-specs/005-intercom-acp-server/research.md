# Research: Intercom ACP Server

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## 1. Agent Client Protocol (ACP) Wire Format

### Decision: Line-delimited JSON (NDJSON) over stdio

### Rationale

The backlog specifies that the host CLI agent (GitHub Copilot CLI) operates in headless/stdio mode. The existing `host_cli` and `host_cli_args` configuration (`"copilot.exe"`, `["--stdio"]`) confirms stdio as the transport. Line-delimited JSON (NDJSON) is the standard framing for stdio-based agent communication — each JSON object is separated by a `\n` delimiter.

### Alternatives Considered

| Alternative | Evaluation |
|-------------|-----------|
| LSP Content-Length framing | Used by Language Server Protocol. More complex to implement. Would require a custom codec instead of `LinesCodec`. Not required unless the agent demands it — can be added as a future codec variant. |
| Raw TCP socket | The backlog discusses TCP connections, but the existing spawner infrastructure uses `tokio::process::Command` with stdio capture. TCP would require the agent to listen on a port, adding configuration complexity. |
| WebSocket | Overkill for local process communication. Adds dependency overhead. |

### Fallback Strategy

If the host CLI uses LSP Content-Length framing instead of NDJSON, the codec layer is designed as a trait/enum so an `LspCodec` variant can be swapped in without changing the reader/writer tasks. The codec selection can be configured in `config.toml` (e.g., `acp_framing = "ndjson"` or `acp_framing = "lsp"`).

---

## 2. AgentDriver Trait Design

### Decision: Async trait with `AgentEvent` channel for outbound events

### Rationale

The backlog's architecture section prescribes a trait-based abstraction (`AgentDriver`) with two communication directions:
- **Inbound (Slack → Agent)**: Method calls on the trait (`resolve_clearance`, `send_prompt`, `interrupt`)
- **Outbound (Agent → Slack)**: Events pushed into a `tokio::sync::mpsc` channel

This design decouples the Slack event loop from protocol specifics. The MCP driver wraps the existing `oneshot` channel pattern; the ACP driver wraps the stdio stream read/write tasks.

Since Rust 1.75, `async fn` in traits is stable — no need for the `async_trait` crate. However, since the driver is used as `Arc<dyn AgentDriver>`, trait object safety requires boxing the futures. Use `-> Pin<Box<dyn Future<...> + Send>>` return types or the `async_trait` attribute. Given the constitution's preference for minimal dependencies, use explicit `Pin<Box<...>>` patterns to avoid adding `async_trait` as a dependency.

### Alternatives Considered

| Alternative | Evaluation |
|-------------|-----------|
| Enum dispatch (no trait) | Simpler but requires `match` in every call site. Violates open/closed principle — adding a third protocol mode would touch every handler. |
| Channel-only (no trait methods) | Bidirectional channels add complexity. Method calls for inbound are more ergonomic and testable. |
| `async_trait` crate | Works but adds a dependency. Constitution Principle VI prefers stdlib solutions. Explicit `Pin<Box<...>>` is acceptable. |

---

## 3. Workspace-to-Channel Mapping Strategy

### Decision: TOML `[[workspace]]` array in `config.toml` with in-memory HashMap

### Rationale

The spec requires centralized workspace-to-channel mapping in `config.toml` with hot-reload support. Using a TOML table-array (`[[workspace]]`) with `id` and `channel_id` fields is idiomatic for the existing config structure. At startup, the array is parsed into a `HashMap<String, String>` (workspace_id → channel_id). The existing `notify`-based policy watcher pattern can be extended to watch `config.toml` for workspace mapping changes.

### Config Format

```toml
[[workspace]]
id = "agent-intercom"
channel_id = "C0123FRONTEND"

[[workspace]]
id = "my-backend"
channel_id = "C0456BACKEND"
```

### Alternatives Considered

| Alternative | Evaluation |
|-------------|-----------|
| SQLite table for workspace mappings | Overkill for a small mapping (5-10 entries typical). Adds DB migration complexity. Config file is the right abstraction level. |
| Separate workspace config file | Fragments configuration. Constitution Principle VI favors simplicity — single config file preferred. |
| Inline TOML table (`[workspace.agent-intercom]`) | Less readable when many workspaces exist. Array format scales better and is more explicit. |

---

## 4. Session Threading Implementation

### Decision: `thread_ts` column on `session` table, set on first Slack message

### Rationale

Slack threads are identified by the `ts` (timestamp) of the root message. The session model needs a `thread_ts` field that is `NULL` on creation and populated when the first Slack message for that session is posted. All subsequent `SlackService` calls for that session include `thread_ts` as the reply target.

The `SlackService::post_message` method signature gains an optional `thread_ts: Option<&str>` parameter. The session repo provides a `set_thread_ts(session_id, ts)` method called after the first message is posted.

### Migration Strategy

Since the schema uses idempotent DDL, adding columns to the `session` table requires `ALTER TABLE` statements alongside the existing `CREATE TABLE IF NOT EXISTS`. The `bootstrap_schema` function already runs on every startup — add idempotent `ALTER TABLE session ADD COLUMN IF NOT EXISTS` statements for the new fields.

Note: SQLite's `ALTER TABLE ADD COLUMN` does not support `IF NOT EXISTS` directly. Use the pattern: check `PRAGMA table_info(session)` for column existence, then conditionally `ALTER TABLE`.

---

## 5. Multi-Session Channel Routing

### Decision: Channel + thread_ts composite lookup in session_repo

### Rationale

The spec identifies the RI-04 deferred issue: `store_from_slack` in `src/slack/handlers/steer.rs` picks the first active session regardless of channel. The fix is to add `channel_id` to the `session` table and scope all session lookups by channel. When multiple sessions exist in the same channel (different workspaces), `thread_ts` disambiguates.

New query methods:
- `find_active_by_channel(channel_id)` — returns all active sessions in a channel
- `find_by_channel_and_thread(channel_id, thread_ts)` — returns the specific session for a threaded interaction

### Slack Interaction Routing

Button actions and modal submissions include both `channel_id` and `message_ts` in their payloads. The router extracts these and finds the matching session. Slash commands include `channel_id` but not `thread_ts` — they default to the most recently active session in that channel.

---

## 6. ACP Stall Detection Adaptation

### Decision: Monitor stream read activity instead of tool call timestamps

### Rationale

The existing stall detector monitors the `updated_at` field on sessions, which is bumped by tool calls (particularly `ping`). In ACP mode, the agent may not call tools — instead, it sends messages on the stream. The stall detector needs a second activity signal: the ACP reader task bumps a shared `last_stream_activity` timestamp on every successful message read.

The `StallDetector` gains an `activity_source` enum:
- `ToolCall` — existing behavior, monitors `session.updated_at`
- `StreamActivity` — monitors the shared timestamp from the ACP reader

### Nudge Delivery

In MCP mode, nudges are delivered via MCP custom notification (`intercom/nudge`). In ACP mode, nudges are delivered by writing a prompt message to the ACP stream via the `AgentDriver::send_prompt` method. The stall detector calls `driver.send_prompt(session_id, nudge_message)` regardless of mode.

---

## 7. Backward Compatibility for `channel_id` Query Parameter

### Decision: Accept both `workspace_id` and `channel_id`, prefer `workspace_id`

### Rationale

Existing MCP clients (VS Code workspaces) have `?channel_id=C...` in their `mcp.json`. Breaking this would require updating every workspace configuration simultaneously. The transition path:

1. Accept both `workspace_id` and `channel_id` query parameters
2. If `workspace_id` is present, look up channel from config mapping
3. If only `channel_id` is present, use it directly (backward compatible)
4. If both are present, `workspace_id` wins; log deprecation warning
5. Document the migration path in config docs

### Timeline

The `channel_id` query parameter is deprecated but not removed in this feature. A future feature (006+) can remove it after all workspaces have migrated.

---

## 8. Process Spawning and Management

### Decision: Extend existing `spawner.rs` pattern with ACP-specific stdio capture

### Rationale

The existing `src/orchestrator/spawner.rs` already spawns agent processes via `tokio::process::Command` with `kill_on_drop(true)`. For ACP mode, the spawner additionally captures stdin/stdout handles and passes them to the ACP codec/reader/writer tasks.

Key differences from MCP spawning:
- MCP spawning sets up stdio for the agent to connect back via MCP
- ACP spawning captures stdin/stdout handles for direct bidirectional streaming
- Both use `kill_on_drop(true)` for crash safety

The spawner returns an `AcpConnection` struct containing the `ChildStdin` (write handle), `ChildStdout` (read handle), and `Child` (process handle for monitoring).
