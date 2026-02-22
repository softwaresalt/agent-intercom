# Setup Guide

Complete installation and configuration instructions for monocoque-agent-rc and its Slack integration.

## Prerequisites

- **Rust** stable toolchain (edition 2021) — install via [rustup](https://rustup.rs/)
- **Slack workspace** where you have permission to create apps
- **Windows 10/11** or **Linux/macOS** (named pipes on Windows, Unix domain sockets elsewhere)

## 1. Build the Server

Clone the repository and build both binaries:

```bash
git clone https://github.com/softwaresalt/monocoque-agent-rc.git
cd monocoque-agent-rc
cargo build --release
```

This produces two binaries in `target/release/`:

| Binary | Description |
|---|---|
| `monocoque-agent-rc` | The MCP remote agent server |
| `monocoque-ctl` | Local CLI companion for approvals and session control |

## 2. Create a Slack App

### 2.1 Create the App

1. Go to [api.slack.com/apps](https://api.slack.com/apps) and click **Create New App**.
2. Choose **From scratch**.
3. Name it (e.g., `Monocoque Agent RC`) and select your workspace.
4. Click **Create App**.

### 2.2 Enable Socket Mode

Socket Mode allows the server to receive events over an outbound WebSocket — no public URL or firewall ports needed.

1. In the app settings sidebar, go to **Socket Mode**.
2. Toggle **Enable Socket Mode** to On.
3. When prompted, give the app-level token a name (e.g., `monocoque-socket`) and add the scope `connections:write`.
4. Click **Generate**. Copy the token (`xapp-...`). This is your `SLACK_APP_TOKEN`.

### 2.3 Configure Bot Token Scopes

1. Go to **OAuth & Permissions** in the sidebar.
2. Under **Bot Token Scopes**, add these scopes:

| Scope | Purpose |
|---|---|
| `chat:write` | Post approval messages, status updates, and notifications |
| `chat:update` | Replace buttons after click (double-submission prevention) |
| `channels:history` | Read channel history (MCP resource `slack://channel/{id}/recent`) |
| `channels:read` | List and identify channels |
| `files:write` | Upload large diffs as file snippets |
| `commands` | Register the `/monocoque` slash command |

### 2.4 Create the Slash Command

1. Go to **Slash Commands** in the sidebar.
2. Click **Create New Command**.
3. Set:
   - **Command:** `/monocoque`
   - **Request URL:** Leave blank (Socket Mode handles routing)
   - **Short Description:** `Agent Remote Control commands`
   - **Usage Hint:** `help | sessions | session-start <prompt> | ...`
4. Click **Save**.

### 2.5 Enable Events (Interactivity)

1. Go to **Interactivity & Shortcuts** in the sidebar.
2. Toggle **Interactivity** to On.
3. The Request URL field is not required for Socket Mode, but if prompted, enter any placeholder URL.
4. Click **Save Changes**.

### 2.6 Install the App to Your Workspace

1. Go to **Install App** in the sidebar.
2. Click **Install to Workspace** and authorize.
3. Copy the **Bot User OAuth Token** (`xoxb-...`). This is your `SLACK_BOT_TOKEN`.

### 2.7 Invite the Bot to Your Channel

The bot must be a member of any channel it posts to.

**Option A — Slash command:**
Open the target Slack channel and type:

```
/invite @Monocoque Agent RC
```

**Option B — Channel settings:**
1. Click the channel name at the top of the channel.
2. Go to the **Integrations** tab.
3. Click **Add an App**.
4. Select your bot from the list.

### 2.8 Collect Required IDs

You need three identifiers:

| Value | Where to Find It | Format |
|---|---|---|
| **Channel ID** | Right-click the channel name → **View channel details** → scroll to the bottom | `C...` (e.g., `C01234ABCDE`) |
| **Team ID** | In your workspace URL (`https://app.slack.com/client/T.../...`) or via the Slack API `auth.test` | `T...` (e.g., `T01234ABCDE`) |
| **Your User ID** | Click your profile picture → **Profile** → **⋮** → **Copy member ID** | `U...` (e.g., `U0123456789`) |

## 3. Store Credentials

Credentials are loaded at runtime — never stored in `config.toml`.

### 3.1 Environment Variables (Recommended for Development)

Set these as **user-level** environment variables so they persist across terminal sessions:

**Windows (PowerShell):**

```powershell
[System.Environment]::SetEnvironmentVariable("SLACK_APP_TOKEN", "xapp-1-...", "User")
[System.Environment]::SetEnvironmentVariable("SLACK_BOT_TOKEN", "xoxb-...", "User")
[System.Environment]::SetEnvironmentVariable("SLACK_TEAM_ID", "T01234ABCDE", "User")
[System.Environment]::SetEnvironmentVariable("SLACK_MEMBER_IDS", "U0123456789", "User")
```

**Linux/macOS (bash/zsh):**

```bash
export SLACK_APP_TOKEN="xapp-1-..."
export SLACK_BOT_TOKEN="xoxb-..."
export SLACK_TEAM_ID="T01234ABCDE"
export SLACK_MEMBER_IDS="U0123456789"
```

Add these to your `.bashrc` / `.zshrc` for persistence.

`SLACK_MEMBER_IDS` is a comma-separated list of Slack user IDs authorized to approve requests and issue commands. Multiple users:

```
SLACK_MEMBER_IDS=U0123456789,U9876543210
```

### 3.2 OS Keychain (Recommended for Production)

Credentials stored in the OS keychain take priority over environment variables.

**Windows (Credential Manager):**

```powershell
# Using the keyring CLI or programmatically:
# Service: "monocoque-agent-rc"
# Keys: slack_app_token, slack_bot_token, slack_team_id
```

**macOS (Keychain Access):**

Store entries under service `monocoque-agent-rc` with keys `slack_app_token`, `slack_bot_token`, and `slack_team_id`.

**Resolution order per credential:** OS Keychain first, then environment variable fallback.

## 4. Configure the Server

Create or edit `config.toml` in the project root:

```toml
# Workspace root — the default directory agents operate in.
default_workspace_root = "D:/Source/GitHub/my-project"

# HTTP port for the SSE transport (agents connect here).
http_port = 3000

# IPC socket name (must match monocoque-ctl --ipc-name).
ipc_name = "monocoque-agent-rc"

# Maximum concurrent agent sessions.
max_concurrent_sessions = 3

# Host CLI binary the server spawns for new sessions.
host_cli = "D:/Tools/ghcpcli/copilot.exe"
host_cli_args = ["--stdio"]

# Days before terminated session data is purged.
retention_days = 30

[database]
# SQLite database path (parent directories auto-created).
path = "data/monocoque.db"

[slack]
# Default Slack channel ID for notifications.
# Per-workspace overrides are set via the SSE URL query parameter.
channel_id = "{your-slack-channel-id}"

[timeouts]
# Approval request timeout in seconds (default: 1 hour).
approval_seconds = 3600
# Continuation prompt timeout in seconds (default: 30 minutes).
prompt_seconds = 1800
# Wait-for-instruction timeout; 0 = indefinite.
wait_seconds = 0

[stall]
# Enable stall detection for idle agents.
enabled = true
# Seconds of inactivity before triggering a stall alert.
inactivity_threshold_seconds = 300
# Seconds between auto-nudge attempts.
escalation_threshold_seconds = 120
# Maximum consecutive auto-nudges before escalation.
max_retries = 3
# Default message sent to the agent on auto-nudge.
default_nudge_message = "Continue working on the current task. Pick up where you left off."

[commands]
# Custom shell commands exposed via /monocoque <alias> in Slack.
# Only commands listed here can be invoked — workspace policies cannot add more.
status = "git status"
```

## 5. Configure VS Code (MCP Client)

Add the server to your workspace's `.vscode/mcp.json`:

```jsonc
{
  "servers": {
    "monocoque-agent-rc": {
      "type": "sse",
      "url": "http://127.0.0.1:3000/sse?channel_id={your-slack-channel-id}"
    }
  }
}
```

The `channel_id` query parameter routes notifications to a specific Slack channel for this workspace. When omitted, the global `slack.channel_id` from `config.toml` is used.

**Multiple workspaces:** Each workspace can have its own `.vscode/mcp.json` targeting a different channel, all connecting to the same server instance.

## 6. Start the Server

### Development Mode

Use the included debug script, which loads credentials from user-level environment variables:

```powershell
# Build first
cargo build

# Run
pwsh ./run-debug.ps1
```

Or run directly:

```powershell
$env:SLACK_APP_TOKEN = [System.Environment]::GetEnvironmentVariable("SLACK_APP_TOKEN", "User")
$env:SLACK_BOT_TOKEN = [System.Environment]::GetEnvironmentVariable("SLACK_BOT_TOKEN", "User")
$env:SLACK_TEAM_ID   = [System.Environment]::GetEnvironmentVariable("SLACK_TEAM_ID", "User")
$env:SLACK_MEMBER_IDS = [System.Environment]::GetEnvironmentVariable("SLACK_MEMBER_IDS", "User")
$env:RUST_LOG = "info"

.\target\debug\monocoque-agent-rc.exe --config config.toml
```

### Release Mode

```bash
cargo build --release
RUST_LOG=info ./target/release/monocoque-agent-rc --config config.toml
```

### Verify Startup

The server logs its initialization sequence. Look for:

```
INFO starting HTTP/SSE MCP transport bind=127.0.0.1:3000
INFO MCP server ready
```

If Slack is configured, you should also see:

```
INFO slack service started
```

## 7. Verify the Integration

Run the included test script to confirm the full Slack approval flow:

```bash
python test_slack_approval.py
```

This script:

1. Seeds a test session in the SQLite database.
2. Opens an SSE connection with the workspace channel ID.
3. Completes the MCP handshake.
4. Calls the `ask_approval` tool.
5. Waits for the approval message to appear in Slack.

You should see an approval message with **Accept** and **Reject** buttons in your Slack channel. Click either button to complete the flow.

## 8. Workspace Auto-Approve Policy (Optional)

Create `.monocoque/settings.json` in your workspace root to define auto-approve rules:

```json
{
  "enabled": true,
  "commands": ["status"],
  "tools": ["heartbeat", "remote_log"],
  "file_patterns": {
    "write": ["**/*.md", "docs/**"],
    "read": ["**/*"]
  },
  "risk_level_threshold": "low",
  "log_auto_approved": false,
  "summary_interval_seconds": 300
}
```

This file is hot-reloaded — changes take effect without restarting the server.

| Field | Description |
|---|---|
| `enabled` | Master switch. When `false`, all operations require approval. |
| `commands` | Shell command aliases that bypass approval (must also exist in `config.toml [commands]`). |
| `tools` | MCP tool names that bypass approval. |
| `file_patterns.write` | Glob patterns for file writes that bypass approval. |
| `file_patterns.read` | Glob patterns for file reads that bypass approval. |
| `risk_level_threshold` | Maximum risk level for auto-approve (`low`, `high`). `critical` is never auto-approved. |
| `log_auto_approved` | Post a Slack notification when operations are auto-approved. |

## Troubleshooting

### Bot not posting to Slack

1. **Bot not in channel.** Invite the bot with `/invite @BotName` in the target channel.
2. **Wrong channel ID.** Verify the `channel_id` in your `.vscode/mcp.json` matches the channel where the bot is a member.
3. **Invalid tokens.** Check `SLACK_BOT_TOKEN` and `SLACK_APP_TOKEN` are set correctly.
4. **Missing scopes.** Ensure `chat:write` is in the bot token scopes.

### Server not starting

1. **Port in use.** Check if another process is using port 3000: `netstat -an | findstr :3000`
2. **Missing config.** Ensure `--config config.toml` points to a valid file.
3. **DB directory.** The server auto-creates the database directory, but check file permissions.

### monocoque-ctl not connecting

1. **IPC name mismatch.** Ensure `--ipc-name` matches the server's `ipc_name` in `config.toml`.
2. **Server not running.** The IPC socket only exists while the server is running.

### Unauthorized errors

1. **User ID not in SLACK_MEMBER_IDS.** Add your Slack user ID to the comma-separated list.
2. **Using wrong Slack account.** Verify you're signed into the correct workspace.
