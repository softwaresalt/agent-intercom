# Migration Guide

Transition steps for users upgrading from an earlier installation (previously distributed as `monocoque-agent-rc`).

## Overview of Changes

| Component | Old Name | New Name |
|---|---|---|
| Server binary | `monocoque-agent-rc` | `agent-intercom` |
| CLI binary | `monocoque-ctl` | `agent-intercom-ctl` |
| Slack slash command | `/monocoque` | `/intercom` |
| IPC socket name | `monocoque-agent-rc` | `agent-intercom` |
| OS keychain service | `monocoque-agent-rc` | `agent-intercom` |
| Policy directory | `.agentrc/settings.json` | `.intercom/settings.json` |
| `mcp.json` server key | `monocoque-agent-rc` | `agent-intercom` |
| Database path (example) | `data/agent-rc.db` | `data/agent-intercom.db` |

### MCP Tool Name Changes

| Old Tool Name | New Tool Name |
|---|---|
| `ask_approval` | `check_clearance` |
| `accept_diff` | `check_diff` |
| `check_auto_approve` | `auto_check` |
| `forward_prompt` | `transmit` |
| `wait_for_instruction` | `standby` |
| `heartbeat` | `ping` |
| `remote_log` | `broadcast` |
| `recover_state` | `reboot` |
| `set_operational_mode` | `switch_freq` |

---

## Step-by-Step Migration

### 1. Update the Slack App

The slash command must be renamed from `/monocoque` to `/intercom`.

1. Go to [api.slack.com/apps](https://api.slack.com/apps) and select your app.
2. Go to **Slash Commands** in the sidebar.
3. Click **Edit** on the `/monocoque` command.
4. Change the **Command** field to `/intercom`.
5. Update the **Short Description** to `Agent Intercom commands` (optional but recommended).
6. Click **Save**.

No reinstall is required — the change takes effect immediately.

---

### 2. Rename Credentials in the OS Keychain

If you stored credentials in the OS keychain under the old service name:

**Windows (Credential Manager via PowerShell):**

```powershell
# Read old credentials
$old = Get-StoredCredential -Target "monocoque-agent-rc"

# Write under new service name (repeat for each credential)
New-StoredCredential -Target "agent-intercom" -Credential $old
```

Alternatively, use the Windows Credential Manager UI (search **Credential Manager** in Start), find entries under `monocoque-agent-rc`, and re-create them under `agent-intercom`.

**macOS (Keychain Access):**

Open **Keychain Access**, search for `monocoque-agent-rc`, and duplicate each entry with the service name changed to `agent-intercom`. Remove the old entries after verifying the server starts correctly.

**Environment variables** do not need to change — `SLACK_APP_TOKEN`, `SLACK_BOT_TOKEN`, `SLACK_TEAM_ID`, and `SLACK_MEMBER_IDS` remain the same.

---

### 3. Update `config.toml`

Change the `ipc_name` field:

```toml
# Before
ipc_name = "monocoque-agent-rc"

# After
ipc_name = "agent-intercom"
```

Update the database path if you use the example value:

```toml
# Before
[database]
path = "data/agent-rc.db"

# After
[database]
path = "data/agent-intercom.db"
```

> **Note:** If you keep the old `data/agent-rc.db` path, the existing database file is still used. Rename the file or update the path, but not both.

---

### 4. Update `.vscode/mcp.json`

Change the server key name to `agent-intercom`:

```jsonc
// Before
{
  "servers": {
    "monocoque-agent-rc": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp?channel_id=..."
    }
  }
}

// After
{
  "servers": {
    "agent-intercom": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp?channel_id=..."
    }
  }
}
```

After updating this file, reload the VS Code window (`Ctrl+Shift+P` → **Developer: Reload Window**) so the IDE reconnects to the server under the new key.

---

### 5. Update Workspace Policy Directory

If you use the workspace auto-approve policy, rename the directory:

```bash
# Linux/macOS
mv .agentrc .intercom

# Windows (PowerShell)
Rename-Item .agentrc .intercom
```

The settings file structure inside is unchanged — only the directory name changes.

Also update any references to `"heartbeat"` or `"remote_log"` in the `tools` list of `.intercom/settings.json`:

```json
// Before
{
  "tools": ["heartbeat", "remote_log", "check_auto_approve"]
}

// After
{
  "tools": ["ping", "broadcast", "auto_check"]
}
```

---

### 6. Replace Binaries

Remove the old binaries and install the new ones:

```bash
# Remove old binaries (adjust path to where you extracted/installed them)
rm monocoque-agent-rc monocoque-ctl           # Linux/macOS
del monocoque-agent-rc.exe monocoque-ctl.exe  # Windows

# Extract the new release archive and place agent-intercom and agent-intercom-ctl
# in the same directory, or rebuild from source:
cargo build --release
```

---

### 7. Update Agent `mcp.json` Tool References

If your MCP client configuration or agent custom instructions reference old tool names, update them to the new names per the mapping in the [Overview](#overview-of-changes).

---

### 8. Verify the Migration

1. Start the server: `agent-intercom --config config.toml`
2. Verify the startup logs show no errors.
3. In Slack, type `/intercom help` — you should see the command response.
4. In VS Code, confirm the MCP tools panel shows the 9 tools with new names.
5. Run a test approval workflow:
   - Ask the agent to make a small change.
   - Confirm `check_clearance` posts to Slack.
   - Accept the change and verify `check_diff` applies it.

---

## `channel_id` to `workspace_id` Migration

Earlier installations used a raw Slack `channel_id` directly in the MCP URL query parameter. The preferred approach is now to define named `[[workspace]]` entries in `config.toml` and use `workspace_id` in the URL. The old `channel_id` parameter continues to work, so this migration is optional but recommended for new setups and multi-workspace environments.

### Why Migrate?

| | `?channel_id=` (old) | `?workspace_id=` (new) |
|---|---|---|
| Channel change requires | Editing every workspace's `mcp.json` | Editing only `config.toml` |
| Hot-reload | No | Yes — new sessions pick up immediately |
| Human-readable URL | No | Yes |
| Backward compatible | N/A | Yes — `channel_id` still works as fallback |

### Before

`.vscode/mcp.json` in each workspace contains the raw channel ID:

```jsonc
{
  "servers": {
    "agent-intercom": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp?channel_id=C0123456789"
    }
  }
}
```

### After

**Step 1** — Add a `[[workspace]]` entry to `config.toml` for each workspace:

```toml
[[workspace]]
workspace_id = "my-repo"
channel_id   = "C0123456789"
label        = "My Repository"
```

**Step 2** — Update `.vscode/mcp.json` to use `workspace_id`:

```jsonc
{
  "servers": {
    "agent-intercom": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp?workspace_id=my-repo"
    }
  }
}
```

**Step 3** — Reload the VS Code window (`Ctrl+Shift+P` → **Developer: Reload Window**) so the IDE reconnects with the updated URL.

No server restart is required — the `config.toml` watcher picks up new `[[workspace]]` entries for new sessions automatically.

### Resolution Fallback

If an agent connects with `?workspace_id=unknown-id` and no matching mapping exists, the server falls back to the `channel_id` query parameter (if provided). Connections that yield no channel at all are rejected with a descriptive error.

If you need to revert:

1. Restore the old binaries.
2. Change the Slack slash command back to `/monocoque`.
3. Restore `ipc_name = "monocoque-agent-rc"` in `config.toml`.
4. Restore the `mcp.json` server key to `monocoque-agent-rc`.
5. Restore the `.agentrc/` directory if renamed.
6. The database file is compatible — no changes needed there.
