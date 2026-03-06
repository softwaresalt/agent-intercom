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

---

## Adopting ACP Mode

ACP (Agent Client Protocol) mode lets the server spawn and manage AI agent subprocesses directly, communicating via NDJSON streams on stdin/stdout. This is an alternative to MCP mode where agents connect inbound via HTTP.

### When to Use ACP vs MCP

| | MCP Mode (default) | ACP Mode |
|---|---|---|
| Agent connection | Agent connects to server via HTTP | Server spawns agent as subprocess |
| Transport | Streamable HTTP on `/mcp` | NDJSON on stdin/stdout |
| Lifecycle | Agent manages its own process | Server owns spawn/monitor/restart |
| IDE integration | VS Code, Cursor, etc. via `mcp.json` | Not needed — Slack-driven |
| Slash command | `/acom` | `/arc` |
| Use when | IDE-driven development with MCP clients | Slack-first operation, agent CLI with NDJSON support |

### Step 1: Create a Second Slack App

ACP mode requires its own Slack app (per [ADR-0015](../docs/adrs/0015-separate-slack-apps-for-mcp-and-acp.md)). The two apps coexist in the same Slack workspace with independent credentials and rate limits.

1. Go to [api.slack.com/apps](https://api.slack.com/apps) and click **Create New App**.
2. Name it something distinguishable (e.g., `Agent RC`).
3. Enable **Socket Mode** under **Settings → Socket Mode**.
4. Add a **Slash Command**: `/arc` (pointing to your server).
5. Under **OAuth & Permissions**, add the same bot scopes as your MCP app:
   - `chat:write`, `commands`, `app_mentions:read`, `channels:history`, `groups:history`, `files:write`
6. Install the app to your workspace and note the **Bot Token** and **App-Level Token**.

### Step 2: Configure ACP Credentials

ACP mode uses `_ACP`-suffixed environment variables (or separate keychain entries) so both apps can run on the same machine:

**Environment variables:**

```bash
# ACP-specific credentials (required)
export SLACK_BOT_TOKEN_ACP="xoxb-your-acp-bot-token"
export SLACK_APP_TOKEN_ACP="xapp-your-acp-app-token"
export SLACK_MEMBER_IDS_ACP="U0123456789,U9876543210"

# Optional — falls back to shared SLACK_TEAM_ID if not set
export SLACK_TEAM_ID_ACP="T0123456789"
```

**Or use the OS keychain** under service name `agent-intercom-acp`:

```powershell
# Windows (PowerShell)
cmdkey /generic:agent-intercom-acp:slack_bot_token /user:agent-intercom /pass:"xoxb-..."
cmdkey /generic:agent-intercom-acp:slack_app_token /user:agent-intercom /pass:"xapp-..."
```

The credential resolution order is: ACP keychain → ACP env var → shared keychain → shared env var. This means you can share `SLACK_TEAM_ID` between modes if desired.

### Step 3: Update `config.toml`

Add the `[acp]` section and ensure your `[[workspace]]` entries include the `path` field:

```toml
# Agent CLI binary that supports NDJSON streaming
host_cli = "copilot"
host_cli_args = ["--acp"]

[acp]
max_sessions = 5                  # Maximum concurrent agent sessions
startup_timeout_seconds = 30      # Handshake timeout per session
http_port = 3001                  # ACP HTTP port (MCP uses 3000)

# Each workspace needs a `path` — ACP uses it as the agent's working directory
[[workspace]]
workspace_id = "my-repo"
channel_id   = "C0123456789"
label        = "My Repository"
path         = "D:\\Source\\my-repo"
```

> **Important:** The `path` field determines the agent subprocess's working directory. Without it, ACP falls back to `default_workspace_root`. For MCP mode the `path` field is optional.

### Step 4: Start in ACP Mode

```bash
agent-intercom --mode acp --config config.toml
```

The `--mode` flag accepts `mcp` (default) or `acp`. In ACP mode:

- MCP HTTP transport on port 3000 is **disabled**
- ACP HTTP transport starts on port 3001 (configurable via `[acp].http_port`)
- IPC pipe name is auto-suffixed (`agent-intercom-acp`)
- Slash commands use the `/arc` prefix

### Step 5: Use ACP Slash Commands

ACP mode introduces session management commands (unavailable in MCP mode):

| Command | Description |
|---|---|
| `/arc session-start <prompt>` | Start a new agent session with the given prompt |
| `/arc session-stop [session_id]` | Gracefully stop a session (sends interrupt, then terminates) |
| `/arc session-restart [session_id]` | Restart a session with its original prompt |
| `/arc sessions` | List all tracked sessions |
| `/arc help` | Show available commands |

Operator interaction (approvals, steering, prompts) works the same as MCP mode — messages appear in the configured Slack channel, threaded per session.

### Step 6: Verify

1. Start the server with `--mode acp`.
2. In Slack, type `/arc help` — you should see the ACP command list.
3. Start a session: `/arc session-start Hello, please list the files in this directory`.
4. Confirm the agent subprocess spawns and posts output to the Slack channel.
5. Stop the session: `/arc session-stop`.

### Running Both Modes Simultaneously

You can run MCP and ACP servers side-by-side on the same machine. They use separate:

- Slack apps and credentials (`_ACP` suffix)
- HTTP ports (3000 vs 3001)
- IPC pipe names (`agent-intercom` vs `agent-intercom-acp`)
- Database files (configure separate `[database].path` values)

Start them in separate terminals:

```bash
# Terminal 1 — MCP mode (default)
agent-intercom --config config-mcp.toml

# Terminal 2 — ACP mode
agent-intercom --mode acp --config config-acp.toml
```
