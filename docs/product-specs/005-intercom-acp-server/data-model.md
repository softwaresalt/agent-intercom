# Data Model: Intercom ACP Server

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## Entity Changes

### Session (Modified)

The existing `Session` entity gains three new fields to support protocol tracking, Slack threading, and channel routing.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String (UUID) | Yes | Unique record identifier (existing) |
| `owner_user_id` | String | Yes | Owning Slack user ID (existing) |
| `workspace_root` | String | Yes | Absolute path to workspace directory (existing) |
| `status` | Enum | Yes | Lifecycle status: created, active, paused, terminated, interrupted (existing) |
| `prompt` | String | No | Initial prompt/instruction (existing) |
| `mode` | Enum | Yes | Operational routing mode: remote, local, hybrid (existing) |
| `created_at` | DateTime | Yes | Creation timestamp (existing) |
| `updated_at` | DateTime | Yes | Last activity timestamp (existing) |
| `terminated_at` | DateTime | No | Termination timestamp (existing) |
| `last_tool` | String | No | Most recent tool called (existing) |
| `nudge_count` | Integer | Yes | Consecutive nudge attempts (existing) |
| `stall_paused` | Boolean | Yes | Whether stall detection is paused (existing) |
| `progress_snapshot` | JSON | No | Last-reported progress items (existing) |
| **`protocol_mode`** | **Enum** | **Yes** | **Agent communication protocol: `mcp` or `acp`. Recorded at session creation. Default: `mcp`.** |
| **`channel_id`** | **String** | **No** | **Slack channel ID where this session's messages are posted. For MCP: resolved from workspace mapping or query parameter at connection time. For ACP: derived from the Slack channel where `/intercom session-start` was issued.** |
| **`thread_ts`** | **String** | **No** | **Slack thread timestamp of the session's root message. NULL until the first message is posted. All subsequent messages use this as `thread_ts`.** |
| **`connectivity_status`** | **Enum** | **Yes** | **Agent connectivity state: `online`, `offline`, or `stalled`. Separate from lifecycle `status`. Default: `online`. Updated by stream activity monitoring and stall detector.** |
| **`last_activity_at`** | **DateTime** | **No** | **Timestamp of last agent activity (stream message, tool call, heartbeat). Used by stall detector and persisted for recovery across server restarts.** |
| **`restart_of`** | **String** | **No** | **Session ID of the predecessor session if this session was created via a restart. NULL for original sessions. Enables session lineage tracking.** |

#### State Transitions

No changes to existing state transitions. The `protocol_mode` is immutable after creation.

```
Created → Active → Paused → Active (resume)
                 → Terminated
                 → Interrupted
Paused → Terminated
       → Interrupted
Interrupted → Active (recovery)
```

**Session Restart**: When an operator restarts a stalled/interrupted session, a new session record is created with a fresh UUID. The new session inherits `thread_ts` and `channel_id` from the original. The original session remains in `terminated` state. The new session's `restart_of` field links to the original session ID.

#### Validation Rules

- `protocol_mode` must be `mcp` or `acp`
- `channel_id` is set at session creation for ACP sessions (derived from the Slack channel where `/intercom session-start` was issued) or at first tool call for MCP sessions (derived from workspace mapping or query parameter)
- `thread_ts` is immutable once set — the session's Slack thread cannot change
- `connectivity_status` must be `online`, `offline`, or `stalled`
- `restart_of` must reference an existing session ID if set

---

### WorkspaceMapping (New — Config-Derived, Not Persisted)

Workspace-to-channel mapping loaded from `config.toml` at startup and held in memory. Not persisted to SQLite.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String | Yes | Workspace namespace identifier (e.g., `agent-intercom`, `my-backend`). Must be unique across all mappings. |
| `channel_id` | String | Yes | Slack channel ID to route messages for this workspace (e.g., `C0123FRONTEND`). |

#### Validation Rules

- `id` must be non-empty and contain only alphanumeric characters, hyphens, and underscores
- `channel_id` must match Slack channel ID format (starts with `C` or `G`, followed by alphanumeric characters)
- Duplicate `id` values are rejected at config load time
- Multiple workspaces may map to the same `channel_id` (sessions disambiguated by `thread_ts`)

---

### AgentEvent (New — Runtime Only, Not Persisted)

Events emitted by the ACP driver (or MCP driver) to the shared application core via `tokio::sync::mpsc` channel.

| Variant | Fields | Description |
|---------|--------|-------------|
| `ClearanceRequested` | `request_id: String`, `session_id: String`, `title: String`, `description: Option<String>`, `diff: Option<String>`, `file_path: String`, `risk_level: String` | Agent requests operator approval for a file operation |
| `StatusUpdated` | `session_id: String`, `message: String` | Agent sends a status update or log message |
| `PromptForwarded` | `session_id: String`, `prompt_id: String`, `prompt_text: String`, `prompt_type: String` | Agent forwards a continuation prompt for operator decision |
| `HeartbeatReceived` | `session_id: String`, `progress: Option<Vec<ProgressItem>>` | Agent sends a heartbeat/ping signal |
| `SessionTerminated` | `session_id: String`, `exit_code: Option<i32>`, `reason: String` | Agent process has exited or stream has closed |

#### Notes

- `AgentEvent` is the unified event type for both MCP and ACP drivers
- The MCP driver generates these events from tool call handlers
- The ACP driver generates these events from parsed stream messages
- The core event loop consumes these events identically regardless of source

---

### AcpMessage (New — Wire Format, Not Persisted)

JSON messages exchanged over the ACP stdio stream. Two directions: agent → server (inbound) and server → agent (outbound).

#### Inbound (Agent → Server)

| Method | Fields | Maps To |
|--------|--------|---------|
| `clearance/request` | `id: String`, `title: String`, `description: String`, `diff: Option<String>`, `file_path: String`, `risk_level: String` | `AgentEvent::ClearanceRequested` |
| `status/update` | `message: String` | `AgentEvent::StatusUpdated` |
| `prompt/forward` | `id: String`, `text: String`, `type: String` | `AgentEvent::PromptForwarded` |
| `heartbeat` | `progress: Option<Vec<ProgressItem>>` | `AgentEvent::HeartbeatReceived` |

#### Outbound (Server → Agent)

| Method | Fields | Description |
|--------|--------|-------------|
| `clearance/response` | `id: String` (envelope), `status: String`, `reason: Option<String>` | Approval decision from operator. Correlation via envelope `id` matching the original `clearance/request` id. |
| `prompt/send` | `text: String` | New prompt or instruction to the agent |
| `prompt/response` | `id: String`, `decision: String`, `instruction: Option<String>` | Decision on a forwarded continuation prompt |
| `session/interrupt` | `reason: String` | Request agent to stop current work |
| `nudge` | `message: String` | Stall recovery nudge message |

---

## Schema Migration

### DDL Additions

Add to `persistence/schema.rs` `bootstrap_schema()` function:

```sql
-- New columns on session table (idempotent via PRAGMA check)
-- protocol_mode: 'mcp' (default) or 'acp'
-- channel_id: Slack channel for this session
-- thread_ts: Slack thread timestamp for session threading
-- connectivity_status: 'online' (default), 'offline', or 'stalled'
-- last_activity_at: timestamp of last agent activity (for stall recovery across restarts)
-- restart_of: predecessor session ID for restarted sessions

ALTER TABLE session ADD COLUMN protocol_mode TEXT NOT NULL DEFAULT 'mcp';
ALTER TABLE session ADD COLUMN channel_id TEXT;
ALTER TABLE session ADD COLUMN thread_ts TEXT;
ALTER TABLE session ADD COLUMN connectivity_status TEXT NOT NULL DEFAULT 'online';
ALTER TABLE session ADD COLUMN last_activity_at TEXT;
ALTER TABLE session ADD COLUMN restart_of TEXT;
```

Since SQLite does not support `ALTER TABLE ADD COLUMN IF NOT EXISTS`, the migration must check `PRAGMA table_info(session)` before each `ALTER TABLE` statement.

### New Indexes

```sql
CREATE INDEX IF NOT EXISTS idx_session_channel ON session(channel_id, status);
CREATE INDEX IF NOT EXISTS idx_session_channel_thread ON session(channel_id, thread_ts);
```

## Relationship Diagram

```
┌──────────────────┐    config.toml     ┌─────────────────────┐
│ WorkspaceMapping │◄───────────────────│    GlobalConfig      │
│ (in-memory)      │                    │ + workspace_mappings │
└──────────────────┘                    └─────────────────────┘
        │ resolves channel_id                     │
        ▼                                         │
┌──────────────────────┐                ┌─────────────────────┐
│    Session           │◄───────────────│     AppState        │
│ + protocol_mode      │                │ + agent_driver      │
│ + channel_id         │                └─────────────────────┘
│ + thread_ts          │                          │
│ + connectivity_status│                ┌─────────────────────┐
│ + last_activity_at   │                │   AgentDriver       │
│ + restart_of         │                │   (trait object)    │
└──────────────────────┘                ├─────────────────────┤
        ▲                               │ McpDriver │AcpDriver│
        │ session_id                    │           │(per-sess)│
┌──────────────────┐                    └─────────────────────┘
│   AgentEvent     │◄───────────────────        ▲
│ (mpsc channel)   │                    session_id → Sender map
└──────────────────┘
```
