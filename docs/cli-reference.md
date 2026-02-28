# CLI Reference — agent-intercom-ctl

`agent-intercom-ctl` is the local CLI companion for agent-intercom. It communicates with a running server over IPC (named pipes on Windows, Unix domain sockets on Linux/macOS) and provides fast, keyboard-driven approvals and session control when you're at your desk.

## Prerequisites

The server must be running. The IPC socket is created on server startup and removed on shutdown.

## Global Options

| Flag | Default | Description |
|---|---|---|
| `--ipc-name <name>` | `agent-intercom` | IPC socket name. Must match the server's `ipc_name` in `config.toml`. |

## Subcommands

### `list`

List all active agent sessions.

```bash
agent-intercom-ctl list
```

**Output fields per session:**

| Field | Description |
|---|---|
| `session_id` | Session UUID |
| `status` | `active`, `paused`, `completed`, `interrupted` |
| `workspace` | Resolved workspace root path |
| `last_tool` | Most recently called MCP tool |
| `last_activity` | Timestamp of most recent activity |

---

### `approve`

Approve a pending `check_clearance` or `transmit` request.

```bash
agent-intercom-ctl approve <request_id>
```

**Arguments:**

| Argument | Required | Description |
|---|---|---|
| `<request_id>` | **Yes** | UUID of the pending request (from `list` output or Slack notification) |

**Effect:** Resolves the oneshot channel in the server, unblocking the agent with `status: "approved"`.

---

### `reject`

Reject a pending `check_clearance` request with a reason.

```bash
agent-intercom-ctl reject <request_id> --reason "needs error handling"
```

**Arguments:**

| Argument | Required | Description |
|---|---|---|
| `<request_id>` | **Yes** | UUID of the pending `check_clearance` request |

**Options:**

| Flag | Required | Description |
|---|---|---|
| `--reason <text>` | **Yes** | Human-readable explanation for the rejection |

**Effect:** Resolves the oneshot channel with `status: "rejected"` and the provided reason.

---

### `resume`

Resume an agent that is waiting via `standby` (wait_for_instruction).

```bash
# Resume with no specific instructions
agent-intercom-ctl resume

# Resume with instructions
agent-intercom-ctl resume "focus on writing tests next"
```

**Arguments:**

| Argument | Required | Description |
|---|---|---|
| `[instruction]` | No | Optional text instructions to pass to the agent |

**Effect:** Resolves the `standby` oneshot with `decision: "resume"` and the optional instruction text.

> **Important:** This command resumes the `standby` MCP tool (wakes a waiting agent). It does not resume a _paused_ session. Session pause/resume are managed via Slack slash commands (`/intercom session-pause`, `/intercom session-resume`) and only change the database status flag — they do not affect the agent's transport connection or process. See the [User Guide — Session Concepts](user-guide.md#session-concepts) for details.

---

### `mode`

Switch the server's operational mode.

```bash
agent-intercom-ctl mode <mode>
```

**Arguments:**

| Argument | Required | Values | Description |
|---|---|---|---|
| `<mode>` | **Yes** | `remote`, `local`, `hybrid` | Target operational mode |

**Mode descriptions:**

| Mode | Slack | IPC | Use Case |
|---|---|---|---|
| `remote` | Active | Inactive | Monitor from anywhere via Slack |
| `local` | Inactive | Active | Fast local approvals at your desk |
| `hybrid` | Active | Active | Both channels active; first response wins |

---

## Examples

```bash
# See what's running
agent-intercom-ctl list

# Approve a pending code change
agent-intercom-ctl approve a1b2c3d4-...

# Reject with a reason
agent-intercom-ctl reject a1b2c3d4-... --reason "use error handling instead of unwrap"

# Wake a waiting agent with instructions
agent-intercom-ctl resume "switch to the authentication module"

# Switch to local mode for desk work
agent-intercom-ctl mode local

# Switch back to remote when leaving
agent-intercom-ctl mode remote
```

## IPC Protocol

The CLI uses a JSON-line protocol over named pipes (Windows) or Unix domain sockets. Each request and response is a single JSON object terminated by a newline character. The protocol is internal and may change between releases.
