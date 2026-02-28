# Contract: Workspace-to-Channel Mapping

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## Overview

Defines how workspace namespaces map to Slack channel IDs, replacing the per-workspace `channel_id` query parameter with centralized configuration.

## Configuration Format

```toml
# config.toml

[[workspace]]
id = "agent-intercom"
channel_id = "C0123FRONTEND"

[[workspace]]
id = "my-backend"
channel_id = "C0456BACKEND"

[[workspace]]
id = "shared-libs"
channel_id = "C0123FRONTEND"   # Multiple workspaces can share a channel
```

## Resolution Logic

### Query Parameter Handling

The SSE/MCP endpoint accepts these query parameters:

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace_id` | String | Workspace namespace to resolve via config mapping (new) |
| `channel_id` | String | Direct Slack channel ID (legacy, deprecated) |
| `session_id` | String | Pre-created session ID for spawned agents (existing) |

### Resolution Priority

```
1. If workspace_id is present:
   a. Look up in workspace_mappings HashMap
   b. If found → use mapped channel_id
   c. If not found → log warning, session operates without Slack channel
2. If only channel_id is present:
   a. Log deprecation warning
   b. Use channel_id directly
3. If both workspace_id and channel_id are present:
   a. workspace_id takes precedence
   b. Log deprecation warning for channel_id
4. If neither is present:
   a. Session operates without Slack channel (local-only mode)
```

### MCP.json Migration

**Before (deprecated)**:
```json
{
  "servers": {
    "intercom": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp?channel_id=C0123FRONTEND"
    }
  }
}
```

**After (preferred)**:
```json
{
  "servers": {
    "intercom": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp?workspace_id=agent-intercom"
    }
  }
}
```

## In-Memory Representation

```rust
/// Workspace-to-channel mapping loaded from config.toml.
pub struct WorkspaceMappings {
    /// Maps workspace_id → channel_id
    mappings: HashMap<String, String>,
}

impl WorkspaceMappings {
    /// Resolve a workspace_id to its configured Slack channel.
    pub fn resolve(&self, workspace_id: &str) -> Option<&str>;

    /// Check if a workspace_id has a configured mapping.
    pub fn contains(&self, workspace_id: &str) -> bool;
}
```

## Hot-Reload Behavior

- The `notify` file watcher (existing `PolicyWatcher` pattern) watches `config.toml`
- On file change, the workspace mappings section is re-parsed
- The new mappings replace the old `Arc<RwLock<WorkspaceMappings>>`
- Active sessions are **not** affected — they retain their channel_id from connection time
- Only new connections use the updated mappings

## Validation Rules

| Rule | Error |
|------|-------|
| Workspace `id` is empty | `AppError::Config("workspace id must not be empty")` |
| Workspace `id` contains invalid characters | `AppError::Config("workspace id must be alphanumeric, hyphens, or underscores")` |
| Duplicate workspace `id` | `AppError::Config("duplicate workspace id: {id}")` |
| `channel_id` is empty for a workspace entry | `AppError::Config("channel_id must not be empty for workspace: {id}")` |

## Test Contract

1. **resolve known workspace** — `workspace_id=agent-intercom` → channel `C0123FRONTEND`
2. **resolve unknown workspace** — `workspace_id=unknown` → `None`
3. **backward compat channel_id** — only `channel_id=C123` → channel `C123`
4. **precedence** — both `workspace_id` and `channel_id` → workspace mapping wins
5. **duplicate detection** — two entries with same `id` → config parse error
6. **hot-reload** — change mapping, reload → new sessions use updated mapping
7. **empty workspace section** — no `[[workspace]]` entries → all sessions local-only unless `channel_id` provided
