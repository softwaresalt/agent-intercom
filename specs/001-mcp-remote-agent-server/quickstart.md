# Quickstart: MCP Remote Agent Server

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)
**Date**: 2026-02-09

## Prerequisites

- Rust toolchain (stable, edition 2021) — install via [rustup](https://rustup.rs/)
- A Slack workspace with a bot application configured for Socket Mode
  - App-Level Token (`xapp-...`) with `connections:write` scope
  - Bot User OAuth Token (`xoxb-...`) with `chat:write`, `files:write`, `commands`, `reactions:write` scopes
  - Slash command `/monocoque` configured pointing to the bot
  - Interactivity enabled (for button actions and modal submissions)
- An MCP-compatible AI agent (Claude Code, GitHub Copilot CLI, Cursor, VS Code) installed on the local workstation
- The host CLI binary for session spawning (e.g., `claude`, `gh`) on the system `PATH`

## Setup

### 1. Clone and build

```bash
git clone https://github.com/softwaresalt/monocoque-agent-rem.git
cd monocoque-agent-rem
cargo build --release
```

The build produces two binaries:

- `target/release/monocoque-agent-rem` — the MCP server
- `target/release/monocoque-ctl` — the local CLI override tool

### 2. Create the global configuration

Create `config.toml` in the project root (or `~/.config/monocoque/config.toml`):

```toml
workspace_root = "/path/to/your/project"
http_port = 3000
ipc_name = "monocoque-agent-rem"

[slack]
app_token = "xapp-1-..."
bot_token = "xoxb-..."
channel_id = "C0123456789"
authorized_user_ids = ["U0123456789"]

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = true
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "Continue working on the current task. Pick up where you left off."

[host]
cli = "claude"
cli_args = []

max_concurrent_sessions = 3

[commands]
status = "git status"
diff = "git diff"
log = "git log --oneline -20"
test = "cargo test"
clippy = "cargo clippy"
```

### 3. (Optional) Create workspace auto-approve policy

Create `.monocoque/settings.json` in your workspace root:

```json
{
  "autoApprove": {
    "enabled": true,
    "commands": ["git status", "git diff", "cargo test *"],
    "tools": ["remote_log", "check_auto_approve", "heartbeat"],
    "filePatterns": {
      "write": ["tests/**", "*.test.rs"],
      "read": ["**"]
    },
    "riskLevelThreshold": "low"
  },
  "notifications": {
    "logAutoApproved": true,
    "summaryIntervalSeconds": 300
  }
}
```

### 4. Connect your AI agent

Add the MCP server to your agent's configuration.

**Claude Code** (`~/.claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "monocoque": {
      "command": "/path/to/monocoque-agent-rem",
      "args": ["--config", "/path/to/config.toml"]
    }
  }
}
```

**VS Code / Copilot** (`.vscode/mcp.json`):

```json
{
  "servers": {
    "monocoque": {
      "command": "/path/to/monocoque-agent-rem",
      "args": ["--config", "/path/to/config.toml"]
    }
  }
}
```

### 5. Verify the connection

Once the agent starts and connects:

1. The server connects to Slack via Socket Mode (outbound WebSocket — no firewall changes needed).
2. A startup message appears in the configured Slack channel: "Monocoque Agent Remote connected."
3. The agent can now call any of the 9 MCP tools: `ask_approval`, `accept_diff`, `check_auto_approve`, `forward_prompt`, `remote_log`, `recover_state`, `set_operational_mode`, `wait_for_instruction`, `heartbeat`.

## Basic workflow

1. **Agent generates a code change** → calls `ask_approval` with the diff.
2. **Server posts the diff to Slack** with Accept/Reject buttons.
3. **Operator reviews on mobile** → taps Accept.
4. **Server returns "approved"** to the agent with a `request_id`.
5. **Agent calls `accept_diff`** with the `request_id`.
6. **Server writes the file to disk** and confirms to both the agent and Slack.

## Running tests

```bash
# Unit tests (uses in-memory SurrealDB)
cargo test

# Integration tests
cargo test --test integration

# Contract validation
cargo test --test contract
```

## Local override (when at the desk)

Use `monocoque-ctl` to approve/reject from a local terminal:

```bash
# List pending requests
monocoque-ctl list

# Approve a specific request
monocoque-ctl approve <request_id>

# Reject with reason
monocoque-ctl reject <request_id> --reason "needs more tests"
```
