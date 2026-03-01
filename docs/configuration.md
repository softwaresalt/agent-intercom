# Configuration Reference

All runtime settings live in `config.toml`. Copy `config.toml.example` to `config.toml` and adjust the values for your environment.

```bash
cp config.toml.example config.toml
```

Pass `--config <path>` to use a different location (default: `config.toml` in the working directory).

---

## Top-Level Settings

| Key | Type | Default | Description |
|---|---|---|---|
| `default_workspace_root` | string | *(required)* | Absolute path to the primary Git workspace root. The MCP agent operates within this directory. |
| `http_port` | integer | `3000` | Port for the HTTP/SSE MCP transport endpoint. Must match the port in every connected workspace's `.vscode/mcp.json`. |
| `ipc_name` | string | `"agent-intercom"` | Named pipe (Windows) or Unix domain socket name for `agent-intercom-ctl`. Change only when running multiple server instances. |
| `max_concurrent_sessions` | integer | `3` | Maximum concurrent agent sessions. Additional connection attempts are rejected until a session terminates. |
| `host_cli` | string | *(required)* | Path or command name for the AI coding agent CLI. Examples: `"copilot"`, `"claude"`, `"/usr/local/bin/gh"`. |
| `host_cli_args` | array of strings | `[]` | Default arguments passed to `host_cli` when spawning sessions. Typical: `["--stdio"]` for stdio transport or `["--sse"]` for SSE transport. |
| `retention_days` | integer | `30` | Days to keep terminated session data before automatic purge. Applies to sessions, approvals, prompts, checkpoints, steering messages, and inbox items. |

> **Retention and recovery:** Terminated and interrupted sessions remain in the database for `retention_days`. This means an agent can use `reboot` (recover_state) with a specific `session_id` to reload checkpoints from a prior session — but only within the retention window. After purge, all session data is permanently deleted.
| `slack_detail_level` | string | `"standard"` | Controls Slack message verbosity. One of `"minimal"`, `"standard"`, or `"verbose"`. See [Detail Levels](#detail-levels). |

### Detail Levels

| Level | Behavior |
|---|---|
| `minimal` | Only errors and warnings are posted to Slack. Tool calls and status updates are suppressed. |
| `standard` | Normal operational messages — approvals, prompts, session lifecycle, and errors. |
| `verbose` | All events including auto-approved actions, broadcast messages, and heartbeat activity. |

Approvals and critical errors are always posted regardless of the detail level.

---

## `[database]`

| Key | Type | Default | Description |
|---|---|---|---|
| `path` | string | `"data/agent-rc.db"` | Relative or absolute path to the SQLite database file. Parent directories are created automatically on first run. |

---

## `[slack]`

Slack credentials are **not** stored in `config.toml`. They are loaded at runtime from the OS keychain or environment variables.

### Environment Variables

| Variable | Description |
|---|---|
| `SLACK_BOT_TOKEN` | Bot user OAuth token (`xoxb-...`). Required scopes: `chat:write`, `channels:history`, `channels:read`, `files:write`, `commands`. |
| `SLACK_APP_TOKEN` | App-level token for Socket Mode (`xapp-...`). |
| `SLACK_TEAM_ID` | Slack workspace team ID (`T...`). |
| `SLACK_MEMBER_IDS` | Comma-separated Slack user IDs of authorized operators (e.g., `U0123456789,U9876543210`). Only these users can approve requests and issue commands. |

### OS Keychain (Alternative)

Store tokens under the keychain service `agent-intercom` with keys:

- `slack_bot_token`
- `slack_app_token`
- `slack_team_id`

The keychain is checked first; environment variables are used as fallback.

### Per-Workspace Channel

Each VS Code workspace specifies its Slack channel by appending `?channel_id=` to the MCP URL in `.vscode/mcp.json`:

```jsonc
{
  "servers": {
    "agent-intercom": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp?channel_id=C0123FRONTEND"
    }
  }
}
```

There is no global channel fallback. Every workspace must supply either `channel_id` (direct) or `workspace_id` (mapped). See [`[[workspace]]`](#workspace) below for the recommended approach.

---

## `[timeouts]`

| Key | Type | Default | Description |
|---|---|---|---|
| `approval_seconds` | integer | `3600` | Seconds to wait for operator approval before the request times out. |
| `prompt_seconds` | integer | `1800` | Seconds to wait for a continuation prompt response. |
| `wait_seconds` | integer | `0` | Seconds to wait for a standby instruction. `0` means wait indefinitely. |

---

## `[stall]`

Stall detection monitors agent activity and escalates when an agent goes idle for too long.

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | boolean | `true` | Enable or disable automatic stall detection. |
| `inactivity_threshold_seconds` | integer | `300` | Seconds of inactivity before the agent is considered stalled. |
| `escalation_threshold_seconds` | integer | `120` | Seconds after stall detection before auto-nudge or escalation. |
| `max_retries` | integer | `3` | Maximum consecutive auto-nudge attempts before marking the session as blocked. |
| `default_nudge_message` | string | `"Continue working on the current task. Pick up where you left off."` | Message delivered to the agent when a stall is detected. |

---

## `[commands]`

A key-value map of short aliases for the `/intercom run <alias>` slash command. Each key is an alias name, and the value is the shell command to execute.

```toml
[commands]
status = "git status -s"
test = "cargo test"
lint = "cargo clippy -- -D warnings"
```

These aliases are for operator convenience only. They do not affect MCP auto-approve policy.

---

## `[[workspace]]`

Workspace-to-channel mapping entries route agent connections to the correct Slack channel by a short, human-readable identifier instead of a raw Slack channel ID. Each `[[workspace]]` block adds one mapping.

| Key | Type | Required | Description |
|---|---|---|---|
| `workspace_id` | string | Yes | Short identifier used in the `?workspace_id=` query parameter. Must be unique and non-empty. |
| `channel_id` | string | Yes | Slack channel ID that messages for this workspace are routed to. |
| `label` | string | No | Human-readable label shown in logs and Slack messages. |

```toml
[[workspace]]
workspace_id = "my-repo"
channel_id   = "C0123456789"
label        = "My Repository"

[[workspace]]
workspace_id = "api-service"
channel_id   = "C9876543210"
label        = "API Service"
```

Agent `.vscode/mcp.json` using a workspace mapping:

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

### Resolution Order

When an agent connects, the server resolves the target channel as follows:

1. If `workspace_id` is present, look up the matching `[[workspace]]` entry and use its `channel_id`.
2. If no match is found (or `workspace_id` was not provided), fall back to the `channel_id` query parameter.
3. If neither yields a channel, the connection is rejected.

`[[workspace]]` entries are hot-reloaded — changes take effect for new sessions without restarting the server.

> **Migration note:** The `?channel_id=` query parameter is the original approach and remains supported for backwards compatibility. The `?workspace_id=` approach is preferred for new installations because channel reassignments only require updating `config.toml`, not every workspace's `mcp.json`. See [Migration Guide](migration-guide.md#channel_id-to-workspace_id-migration) for transition steps.

---

## `[acp]`

ACP (Agent Communication Protocol) mode spawns the AI agent CLI as a subprocess and communicates via a newline-delimited JSON (NDJSON) stream. This mode is an alternative to the MCP HTTP/SSE and stdio transports.

Start the server in ACP mode:

```bash
agent-intercom --mode acp
```

In ACP mode, credentials are loaded from mode-prefixed environment variables (`SLACK_BOT_TOKEN_ACP`, `SLACK_APP_TOKEN_ACP`, `SLACK_MEMBER_IDS_ACP`) before falling back to the shared variables. The OS keychain service name is `agent-intercom-acp`.

| Key | Type | Default | Description |
|---|---|---|---|
| `max_sessions` | integer | `5` | Maximum concurrent ACP sessions. Requests beyond this limit are rejected with a descriptive error. |
| `startup_timeout_seconds` | integer | `30` | Seconds to wait for the agent subprocess to emit its ready signal on stdout. If no ready line arrives the spawner kills the process and returns an error. |

```toml
[acp]
max_sessions = 5
startup_timeout_seconds = 30
```

The `[acp]` section may be omitted entirely; all fields default to the values in the table above.

> **Note:** `host_cli` (top-level) must be set when using ACP mode. The server validates this at startup and returns a descriptive error if it is missing.

---

Per-workspace auto-approve rules live in `.intercom/settings.json` inside each workspace root (not in `config.toml`). The policy file is hot-reloaded — changes take effect immediately without restarting the server.

See the [User Guide](user-guide.md) for auto-approve policy syntax and examples.

---

## Complete Example

```toml
# Absolute path to the primary workspace root.
default_workspace_root = "/home/dev/my-project"

# HTTP/SSE transport port.
http_port = 3000

# IPC socket name for agent-intercom-ctl.
ipc_name = "agent-intercom"

# Maximum concurrent agent sessions.
max_concurrent_sessions = 3

# AI coding agent CLI command and default arguments.
host_cli = "copilot"
host_cli_args = ["--sse"]

# Days to retain terminated session data.
retention_days = 30

# Slack message verbosity: minimal, standard, or verbose.
slack_detail_level = "standard"

[database]
path = "data/agent-intercom.db"

[slack]
# Credentials come from environment variables or OS keychain.
# See the Environment Variables section above.

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

[commands]
status = "git status -s"

# Workspace-to-channel mappings (preferred over ?channel_id= query parameter).
[[workspace]]
workspace_id = "my-repo"
channel_id   = "C0123456789"
label        = "My Repository"

[[workspace]]
workspace_id = "api-service"
channel_id   = "C9876543210"
label        = "API Service"

# ACP mode settings (omit this section to accept all defaults).
[acp]
max_sessions = 5
startup_timeout_seconds = 30
```
