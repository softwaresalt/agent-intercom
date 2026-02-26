# agent-intercom — Comprehensive Reference

> Agent Intercom MCP Server for Agentic Development IDEs.

---

## Table of Contents

1. [MCP Tools](#1-mcp-tools)
2. [MCP Resources](#2-mcp-resources)
3. [Slack Commands](#3-slack-commands)
4. [Slack Interactive Actions](#4-slack-interactive-actions)
5. [IPC Commands (agent-intercom-ctl)](#5-ipc-commands-agent-intercom-ctl)
6. [Configuration](#6-configuration)
7. [Database Schema](#7-database-schema)
8. [Domain Models](#8-domain-models)
9. [Transport Layer](#9-transport-layer)
10. [Policy System](#10-policy-system)
11. [Orchestrator](#11-orchestrator)
12. [Error Types](#12-error-types)
13. [CLI Arguments](#13-cli-arguments)
14. [Server Lifecycle](#14-server-lifecycle)
15. [Diff & Path Safety](#15-diff--path-safety)
16. [Architecture Decision Records](#16-architecture-decision-records)
17. [Dependencies](#17-dependencies)

---

## 1. MCP Tools

Nine tools are registered via `ToolRouter` / `ToolRoute::new_dyn()`. All nine tools are always registered and visible; inapplicable calls return descriptive errors. The stall detection timer is reset before and after every tool call.

### 1.1 `check_clearance`

**Purpose:** Submit a code proposal for remote operator approval via Slack. **Blocks** the agent until the operator responds (Accept/Reject) or the configured timeout elapses.

**Input Parameters:**

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `title` | `string` | **Yes** | — | Concise summary of the proposal |
| `description` | `string` | No | `null` | Contextual details about the proposed change |
| `diff` | `string` | **Yes** | — | Standard unified diff or raw file content |
| `file_path` | `string` | **Yes** | — | Target file path relative to `workspace_root` |
| `risk_level` | `string` | No | `"low"` | Risk classification. Enum: `"low"`, `"high"`, `"critical"` |

**Response:**

```json
{
  "status": "approved" | "rejected" | "timeout",
  "request_id": "<uuid>",
  "reason": "<string, present only on rejection>"
}
```

**Behavior:**

1. Resolves the active session and its `workspace_root`.
2. Validates `file_path` against the workspace root (path safety).
3. Computes SHA-256 hash of the current file (or `"new_file"` if it doesn't exist).
4. Creates an `ApprovalRequest` record in the database with status `Pending`.
5. Posts to Slack with Block Kit message containing title, description, file path, risk level badge, and diff excerpt.
6. If the diff exceeds 20 lines (`INLINE_DIFF_THRESHOLD`), uploads it as a Slack file snippet.
7. Registers a `tokio::sync::oneshot` channel and blocks until a response arrives or timeout.
8. Timeout: `config.timeouts.approval_seconds` (default 3600s / 1 hour). On timeout, marks the request as `Expired` and posts a warning to Slack.
9. Cleans up the pending map and updates `session.last_tool`.

**Risk Level Emoji Mapping:**
- `low` → 🟢
- `high` → 🟡
- `critical` → 🔴

---

### 1.2 `check_diff`

**Purpose:** Apply previously approved code changes to the local file system. **Non-blocking.**

**Input Parameters:**

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `request_id` | `string` | **Yes** | — | Unique identifier of the approved proposal |
| `force` | `boolean` | No | `false` | When `true`, overwrite even if local content has diverged since proposal |

**Response (success):**

```json
{
  "status": "applied",
  "files_written": [
    { "path": "<relative file path>", "bytes": <integer> }
  ]
}
```

**Response (error):**

```json
{
  "status": "error",
  "error_code": "<code>",
  "error_message": "<description>"
}
```

**Error Codes:**

| Code | Meaning |
|---|---|
| `request_not_found` | No approval request found with the given ID |
| `already_consumed` | Approved diff has already been applied |
| `not_approved` | Approval request is not in `Approved` status |
| `path_violation` | File path escapes workspace root |
| `patch_conflict` | File content has changed since proposal was created |

**Behavior:**

1. Looks up `ApprovalRequest` by `request_id`.
2. Validates status is `Approved` (returns domain error codes for other statuses).
3. Resolves the owning session's `workspace_root`.
4. Validates file path against workspace root.
5. Computes current SHA-256 hash and compares to `original_hash`.
   - If diverged and `force=false`: returns `patch_conflict`.
   - If diverged and `force=true`: warns via Slack and proceeds.
6. Determines write mode:
   - If content starts with `"--- "` or `"diff "` → applies as unified diff patch via `diffy`.
   - Otherwise → writes as full file content.
7. Marks the approval as `Consumed` in the database.
8. Posts confirmation to Slack with bytes written.

---

### 1.3 `auto_check`

**Purpose:** Query the workspace auto-approve policy to determine whether an operation can bypass the remote approval gate. **Non-blocking.**

**Input Parameters:**

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `tool_name` | `string` | **Yes** | — | Name of the tool or command to check |
| `context` | `object` | No | `null` | Additional metadata for fine-grained evaluation |

**Context Object:**

| Field | Type | Description |
|---|---|---|
| `file_path` | `string` | Target file path (relative to workspace root) |
| `risk_level` | `string` | Risk level of the operation: `"low"`, `"high"`, `"critical"` |

**Response:**

```json
{
  "auto_approved": true | false,
  "matched_rule": "<rule key>" | null
}
```

**Matched Rule Format:** `"command:<name>"`, `"tool:<name>"`, or `"file_pattern:<write|read>:<glob>"`.

**Behavior:**

1. Resolves the active session's `workspace_root`.
2. Loads the workspace policy from `.intercom/settings.json`.
3. Evaluates the policy (see [Policy System](#10-policy-system) for evaluation order).
4. Returns immediately with the result.

---

### 1.4 `transmit`

**Purpose:** Forward an agent-generated continuation prompt to the remote operator via Slack with Continue/Refine/Stop buttons. **Blocks** the agent until the operator responds or the configured timeout elapses.

**Input Parameters:**

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `prompt_text` | `string` | **Yes** | — | Raw text of the continuation prompt |
| `prompt_type` | `string` | No | `"continuation"` | Category. Enum: `"continuation"`, `"clarification"`, `"error_recovery"`, `"resource_warning"` |
| `elapsed_seconds` | `integer` | No | `null` | Seconds since last user interaction |
| `actions_taken` | `integer` | No | `null` | Count of actions performed in this iteration |

**Response:**

```json
{
  "decision": "continue" | "refine" | "stop",
  "instruction": "<string, present only when decision is 'refine'>"
}
```

**Behavior:**

1. Resolves the active session.
2. Creates a `ContinuationPrompt` record in the database.
3. Posts to Slack with prompt type icon, text, context line (elapsed time / actions), and Continue/Refine/Stop buttons.
4. Registers a `tokio::sync::oneshot` channel and blocks.
5. Timeout: `config.timeouts.prompt_seconds` (default 1800s / 30 minutes). On timeout, **auto-continues** (decision = `"continue"`) per FR-008, and posts a warning to Slack.
6. If the sender is dropped (server shutdown), also defaults to `"continue"`.

**Prompt Type Icons:**
- `continuation` → 🔄
- `clarification` → ❓
- `error_recovery` → ⚠️
- `resource_warning` → 📊

---

### 1.5 `broadcast`

**Purpose:** Send a non-blocking status log message to the Slack channel with severity-based formatting. Returns immediately.

**Input Parameters:**

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `message` | `string` | **Yes** | — | Log message to post |
| `level` | `string` | No | `"info"` | Severity level. Enum: `"info"`, `"success"`, `"warning"`, `"error"` |
| `thread_ts` | `string` | No | `null` | Slack thread timestamp to post as a reply |

**Response:**

```json
{
  "posted": true | false,
  "ts": "<slack message timestamp>"
}
```

**Behavior:**

1. Validates that `level` is one of the four valid values.
2. Resolves active session for `last_tool` update.
3. Posts directly to Slack (uses `post_message_direct`, not the queue) with severity formatting.
4. Returns `posted: false` if Slack is not configured.

**Severity Formatting (Block Kit):**
- `info` → ℹ️
- `success` → ✅
- `warning` → ⚠️
- `error` → ❌

---

### 1.6 `reboot`

**Purpose:** Retrieve the last known state from persistent storage on startup. Called by the agent to check for interrupted sessions or pending requests. **Non-blocking.**

**Input Parameters:**

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session_id` | `string` | No | `null` | Specific session to recover. When omitted, auto-finds the most recently interrupted session. |

**Response (clean):**

```json
{ "status": "clean" }
```

**Response (recovered):**

```json
{
  "status": "recovered",
  "session_id": "<uuid>",
  "pending_requests": [
    {
      "request_id": "<uuid>",
      "type": "approval" | "prompt",
      "title": "<string>",
      "created_at": "<ISO 8601>"
    }
  ],
  "last_checkpoint": {
    "checkpoint_id": "<uuid>",
    "label": "<string or null>",
    "created_at": "<ISO 8601>"
  },
  "progress_snapshot": [
    { "label": "<string>", "status": "done" | "in_progress" | "pending" }
  ]
}
```

Fields `pending_requests`, `last_checkpoint`, and `progress_snapshot` are omitted when empty/absent.

---

### 1.7 `switch_freq`

**Purpose:** Switch between remote, local, and hybrid operational modes at runtime. Persists the mode to the database. **Non-blocking.**

**Input Parameters:**

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `mode` | `string` | **Yes** | — | Target mode. Enum: `"remote"`, `"local"`, `"hybrid"` |

**Response:**

```json
{
  "previous_mode": "remote" | "local" | "hybrid",
  "current_mode": "remote" | "local" | "hybrid"
}
```

**Behavior:**

1. Resolves the active session and records the previous mode.
2. Updates the session's mode in the database.
3. If the new mode includes Slack (`Remote` or `Hybrid`), posts a notification to Slack.

**Mode Descriptions:**

| Mode | Slack | IPC |
|---|---|---|
| `remote` | Active | Inactive |
| `local` | Inactive | Active |
| `hybrid` | Active | Active (first response wins) |

---

### 1.8 `standby`

**Purpose:** Place the agent in standby, posting a waiting status to Slack with Resume/Stop buttons. **Blocks** until the operator responds or timeout elapses.

**Input Parameters:**

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `message` | `string` | No | `"Agent is idle and awaiting instructions."` | Status message displayed in Slack while waiting |
| `timeout_seconds` | `integer` | No | `0` | Maximum wait time in seconds. `0` = use config value; config `0` = indefinite. |

**Response:**

```json
{
  "status": "resumed" | "timeout",
  "instruction": "<string, present when operator provides instruction>"
}
```

**Behavior:**

1. Resolves the active session.
2. Posts waiting status to Slack with: pause icon, message, optional timeout indicator, and Resume/Resume with Instructions/Stop buttons.
3. Registers a `tokio::sync::oneshot` channel.
4. Effective timeout resolution:
   - If `timeout_seconds` = 0 → use `config.timeouts.wait_seconds`.
   - If config also = 0 → truly indefinite wait (no timeout).
5. On timeout, posts warning to Slack.

---

### 1.9 `ping`

**Purpose:** Lightweight liveness signal. Resets the stall detection timer and optionally stores a structured progress snapshot. **Non-blocking.**

**Input Parameters:**

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `status_message` | `string` | No | `null` | Optional status update logged to the operator via Slack |
| `progress_snapshot` | `array` | No | `null` | Optional structured progress snapshot (replaces previous when present) |

**Progress Snapshot Item:**

| Field | Type | Required | Description |
|---|---|---|---|
| `label` | `string` | **Yes** | Human-readable task description (must not be empty) |
| `status` | `string` | **Yes** | Current status. Enum: `"done"`, `"in_progress"`, `"pending"` |

**Response:**

```json
{
  "acknowledged": true,
  "session_id": "<uuid>",
  "stall_detection_enabled": true | false
}
```

**Behavior:**

1. Resolves the active session. **Requires exactly one active session** — returns an error if zero or multiple active sessions exist.
2. Validates the progress snapshot (all labels must be non-empty).
3. If a snapshot is provided, persists it on the session record.
4. Updates `session.last_tool` and `session.updated_at`.
5. Resets the stall detector timer for the session.
6. If `status_message` is provided, posts it to Slack with ℹ️ severity formatting.

---

## 2. MCP Resources

### 2.1 `slack://channel/{id}/recent`

**Purpose:** Expose recent Slack channel history as an MCP resource so agents can read operator instructions posted directly in the channel.

**Resource Template URI:** `slack://channel/{id}/recent`

**MIME Type:** `application/json`

**Parameters:**

| Parameter | Location | Type | Default | Description |
|---|---|---|---|---|
| `{id}` | URI path | `string` | — | Slack channel ID (must match configured/effective channel) |
| `limit` | URI query | `integer` | `20` | Number of messages to return. Clamped to range `[1, 100]`. |

**Response:**

```json
{
  "messages": [
    {
      "ts": "<slack timestamp>",
      "user": "<slack user ID>",
      "text": "<message text>",
      "thread_ts": "<slack timestamp, present only for threaded replies>"
    }
  ],
  "has_more": true | false
}
```

**Behavior:**

1. Parses the channel ID from the URI (`slack://channel/{id}/recent`).
2. Validates the requested channel matches the configured/effective channel (returns error if mismatch).
3. Fetches history via `conversations.history` Slack API.
4. Returns messages in the contract-defined JSON format.

---

## 3. Slack Commands

All commands are invoked via `/intercom <command>`. Every command enforces authorization — checks the calling user against `config.authorized_user_ids` (loaded from `SLACK_MEMBER_IDS` env var). Unauthorized users are silently ignored.

### 3.1 `help [category]`

**Description:** Display available commands and usage instructions.

**Categories:**

| Category | Description |
|---|---|
| (none) | Full help with all categories |
| `session` | Session management commands |
| `checkpoint` | Checkpoint commands |
| `files` | File browsing commands |

---

### 3.2 `sessions`

**Description:** List all active sessions with their ID, status, workspace, last tool, and last activity timestamp.

---

### 3.3 `session-start <prompt>`

**Description:** Start a new agent session by spawning the host CLI process.

**Parameters:**

| Parameter | Required | Description |
|---|---|---|
| `<prompt>` | **Yes** | Initial task prompt/instruction for the agent |

**Behavior:**

1. Enforces `max_concurrent_sessions` limit.
2. Creates a `Session` record.
3. Spawns the host CLI process with environment variables:
   - `INTERCOM_WORKSPACE_ROOT` — resolved workspace path
   - `INTERCOM_MCP_URL` — `/mcp?session_id=<id>` URL for the spawned agent
   - `INTERCOM_SESSION_ID` — session UUID
4. Activates the session upon successful process start.
5. Posts confirmation to Slack with session ID and workspace.

---

### 3.4 `session-pause [session_id]`

**Description:** Pause a running session. Defaults to the caller's most recently active session if `session_id` is omitted.

**Authorization:** Must be the session owner.

---

### 3.5 `session-resume [session_id]`

**Description:** Resume a paused session. Defaults to the caller's most recently active session if `session_id` is omitted.

**Authorization:** Must be the session owner.

---

### 3.6 `session-clear [session_id]`

**Description:** Terminate and clean up a session. Kills the child process with a 5-second grace period, then force-kills if needed. Defaults to the caller's most recently active session if `session_id` is omitted.

**Authorization:** Must be the session owner.

---

### 3.7 `session-checkpoint [session_id] [label]`

**Description:** Create a checkpoint of the session's current state.

**Parameters:**

| Parameter | Required | Description |
|---|---|---|
| `[session_id]` | No | Target session (defaults to caller's active session) |
| `[label]` | No | Human-readable checkpoint label (e.g., `"before-refactor"`) |

**Behavior:**

1. Computes SHA-256 hashes for all regular files (non-recursive) in the session's workspace root.
2. Serializes the session state as JSON.
3. Stores the checkpoint with file hashes, session state, and optional label.

---

### 3.8 `session-restore <checkpoint_id>`

**Description:** Restore a previously created checkpoint. Warns about any files that have diverged since the checkpoint was created.

**Parameters:**

| Parameter | Required | Description |
|---|---|---|
| `<checkpoint_id>` | **Yes** | ID of the checkpoint to restore |

**Divergence Detection:**

| Kind | Description |
|---|---|
| `Modified` | File content has changed since checkpoint |
| `Deleted` | File existed at checkpoint time but is now missing |
| `Added` | File was added after the checkpoint |

---

### 3.9 `session-checkpoints [session_id]`

**Description:** List all checkpoints for a session. Defaults to the caller's active session.

---

### 3.10 `list-files [path] [--depth N]`

**Description:** List the workspace directory tree.

**Parameters:**

| Parameter | Required | Default | Description |
|---|---|---|---|
| `[path]` | No | `.` (workspace root) | Directory to list |
| `--depth N` | No | `3` | Maximum recursion depth |

**Behavior:** Skips hidden directories, `node_modules`, and `target` by default. Formats output as a text tree.

---

### 3.11 `show-file <path> [--lines START:END]`

**Description:** Display file contents with syntax highlighting.

**Parameters:**

| Parameter | Required | Description |
|---|---|---|
| `<path>` | **Yes** | File path relative to workspace root |
| `--lines START:END` | No | Line range to display |

**Behavior:** If the output exceeds 3500 characters, uploads the content as a Slack file snippet instead of posting inline.

---

### 3.12 Custom Commands

**Description:** Any command alias registered in `config.commands` can be invoked as a Slack command.

**Configuration:** Defined in `config.toml` under `[commands]`:

```toml
[commands]
status = "git status"
```

**Behavior:**

1. Validates the command exists in the global `config.commands` map.
2. Executes the shell command in the workspace root.
3. Pauses the stall detector timer during execution.
4. Posts the output to Slack.

**Security:** Only commands explicitly listed in `config.commands` can be invoked as Slack aliases (FR-014). MCP auto-approve policy is governed separately by `.intercom/settings.json` (ADR-0012).

---

## 4. Slack Interactive Actions

### 4.1 Central Dispatch (events.rs)

All interactive actions flow through a centralized dispatcher that provides:

1. **Authorization guard**: Checks all interacting users against `authorized_user_ids`. Unauthorized users are silently ignored.
2. **Double-submission prevention**: Replaces interactive buttons with "Processing…" text via `chat.update` before dispatching to the appropriate handler.
3. **Routing**: Routes by `action_id` prefix to the correct handler.

### 4.2 Approval Actions

| Action ID | Effect | Resolves To |
|---|---|---|
| `approve_accept` | Sets status to `Approved`, resolves oneshot channel | `check_clearance` returns `status: "approved"` |
| `approve_reject` | Sets status to `Rejected` with reason `"rejected by operator"`, resolves oneshot | `check_clearance` returns `status: "rejected"` |

### 4.3 Prompt Actions

| Action ID | Effect | Resolves To |
|---|---|---|
| `prompt_continue` | Resolves with `Continue` decision | `transmit` returns `decision: "continue"` |
| `prompt_refine` | Resolves with `Refine` decision and placeholder instruction | `forward_prompt` returns `decision: "refine"` |
| `prompt_stop` | Resolves with `Stop` decision | `forward_prompt` returns `decision: "stop"` |

### 4.4 Stall/Nudge Actions

| Action ID | Effect |
|---|---|
| `stall_nudge` | Increments nudge count, resets stall detector timer |
| `stall_nudge_instruct` | Increments nudge count (modal support planned) |
| `stall_stop` | Dismisses the alert, terminates the session, removes stall detector |

### 4.5 Wait Actions

| Action ID | Effect | Resolves To |
|---|---|---|
| `wait_resume` | Resolves with `status: "resumed"` | `wait_for_instruction` returns with no instruction |
| `wait_resume_instruct` | Resolves with `status: "resumed"` and placeholder instruction | `wait_for_instruction` returns with instruction |
| `wait_stop` | Resolves with `status: "resumed"` and instruction `"stop"` | `wait_for_instruction` returns instruction to stop |

---

## 5. IPC Commands (agent-intercom-ctl)

`agent-intercom-ctl` is a local CLI companion binary that communicates with the server via named pipes (Windows) / Unix domain sockets.

### 5.1 CLI Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `--ipc-name` | `string` | `"agent-intercom"` | IPC socket name (must match server's `ipc_name` config) |

### 5.2 Commands

#### `list`

List active sessions.

**Response:**

```json
{
  "sessions": [
    {
      "session_id": "<uuid>",
      "status": "<lowercase status>",
      "mode": "<lowercase mode>",
      "workspace_root": "<path>",
      "last_tool": "<tool name or null>",
      "updated_at": "<ISO 8601>"
    }
  ]
}
```

#### `approve <id>`

Approve a pending approval request.

**Parameters:**
- `<id>` — Approval request ID (required)

**Response:** `{ "request_id": "<id>", "status": "approved" }`

#### `reject <id> [--reason TEXT]`

Reject a pending approval request.

**Parameters:**
- `<id>` — Approval request ID (required)
- `--reason TEXT` — Rejection reason (default: `"rejected via local CLI"`)

**Response:** `{ "request_id": "<id>", "status": "rejected" }`

#### `resume [instruction]`

Resume a waiting agent with optional instruction text.

**Parameters:**
- `[instruction]` — Optional instruction text

**Behavior:**
- If an explicit session ID is provided as `id`, resumes that session.
- Otherwise, resumes the first pending wait (for single-session scenarios).

**Response:** `{ "session_id": "<id>", "status": "resumed" }`

#### `mode <mode>`

Switch operational mode for the active session.

**Parameters:**
- `<mode>` — Target mode: `remote`, `local`, or `hybrid`

**Response:** `{ "previous_mode": "<mode>", "current_mode": "<mode>" }`

### 5.3 IPC Protocol

| Aspect | Detail |
|---|---|
| Transport | Named pipes (Windows) / Unix domain sockets |
| Framing | JSON-line protocol (one JSON object per line, newline-delimited) |
| Library | `interprocess` crate 2.0 with tokio feature |
| Authentication | Optional shared-secret `auth_token` field. Generated per server instance (random UUID). |

**Request format:**

```json
{
  "command": "<verb>",
  "id": "<entity ID, optional>",
  "reason": "<text, optional>",
  "instruction": "<text, optional>",
  "mode": "<mode, optional>",
  "auth_token": "<shared secret, optional>"
}
```

**Response format:**

```json
{
  "ok": true | false,
  "data": { ... },
  "error": "<message, present only on failure>"
}
```

---

## 6. Configuration

### 6.1 config.toml

The main configuration file is parsed into `GlobalConfig`. Path: specified via `--config` CLI argument.

#### Top-Level Fields

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `default_workspace_root` | `PathBuf` | **Yes** | — | Default workspace root used for the primary stdio agent. Canonicalized on load. |
| `max_concurrent_sessions` | `u32` | No | `3` | Maximum number of concurrent agent sessions. Must be > 0. |
| `host_cli` | `string` | **Yes** | — | Path to the host CLI binary (e.g., `"claude"`, `"gh"`, `"copilot.exe"`) |
| `host_cli_args` | `Vec<string>` | No | `[]` | Default arguments passed to the host CLI on spawn |
| `http_port` | `u16` | No | `3000` | HTTP port for the SSE transport (binds to `127.0.0.1`) |
| `ipc_name` | `string` | No | `"agent-intercom"` | Named pipe / Unix socket identifier |
| `retention_days` | `u32` | No | `30` | Days after session termination before data is purged |

#### `[database]`

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `path` | `PathBuf` | No | `"data/agent-rc.db"` | Path to the SQLite database file. Parent directories auto-created. |

#### `[slack]`

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `channel_id` | `string` | **Yes** | — | Default Slack channel ID where notifications are posted |

**Note:** Slack tokens (`app_token`, `bot_token`, `team_id`) are **not** in config.toml. They are loaded at runtime (see Credentials below).

#### `[timeouts]`

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `approval_seconds` | `u64` | No | `3600` | Approval request timeout (seconds) |
| `prompt_seconds` | `u64` | No | `1800` | Continuation prompt timeout (seconds) |
| `wait_seconds` | `u64` | No | `0` | Wait-for-instruction timeout; `0` = no timeout (indefinite) |

#### `[stall]`

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `enabled` | `bool` | No | `true` | Whether stall detection is active |
| `inactivity_threshold_seconds` | `u64` | No | `300` | Idle time (seconds) before triggering a stall alert |
| `escalation_threshold_seconds` | `u64` | No | `120` | Delay (seconds) before auto-nudge when unattended |
| `max_retries` | `u32` | No | `3` | Maximum consecutive auto-nudges before escalation |
| `default_nudge_message` | `string` | No | `"Continue working on the current task. Pick up where you left off."` | Default message delivered to the agent on auto-nudge |

#### `[commands]`

A `HashMap<String, String>` mapping command aliases to shell commands. These define the global allowlist — workspace policies cannot introduce commands outside this list.

```toml
[commands]
status = "git status"
```

### 6.2 Credentials

Credentials are loaded at runtime via `GlobalConfig::load_credentials()`. **Never stored in config.toml.**

**Resolution Order (per credential):**

1. **OS Keychain**: Service `"agent-intercom"`, key name matching the credential.
2. **Environment Variable**: Fallback if keychain lookup fails or returns empty.

Keychain access uses `tokio::task::spawn_blocking` since the `keyring` crate is synchronous I/O.

| Credential | Keychain Key | Env Variable | Required | Description |
|---|---|---|---|---|
| Slack App Token | `slack_app_token` | `SLACK_APP_TOKEN` | **Yes** | App-level token for Socket Mode (`xapp-...`) |
| Slack Bot Token | `slack_bot_token` | `SLACK_BOT_TOKEN` | **Yes** | Bot user OAuth token (`xoxb-...`) |
| Slack Team ID | `slack_team_id` | `SLACK_TEAM_ID` | No | Workspace team ID (`T...`). Empty default if absent. |
| Authorized Users | — | `SLACK_MEMBER_IDS` | **Yes** | Comma-separated Slack user IDs (e.g., `U0123456789,U9876543210`). Whitespace around entries is trimmed. |

**Note:** `SLACK_MEMBER_IDS` is always loaded from the environment variable — there is no keychain fallback for this credential.

### 6.3 Per-Workspace Channel Override

Each VS Code workspace can target a different Slack channel by appending a `channel_id` query parameter to the MCP URL:

```json
{
  "servers": {
    "agent-intercom": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp?channel_id=C0123FRONTEND"
    }
  }
}
```

When omitted, the global `slack.channel_id` is used.

### 6.4 Example config.toml

```toml
default_workspace_root = "D:/Source/GitHub/my-project"
http_port = 3000
ipc_name = "agent-intercom"
max_concurrent_sessions = 3
host_cli = "D:/Tools/ghcpcli/copilot.exe"
host_cli_args = ["--stdio"]
retention_days = 30

[database]
path = "data/agent-intercom.db"

[slack]
channel_id = "C0AFXFQP1TJ"

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
status = "git status"
```

---

## 7. Database Schema

SQLite via `sqlx` 0.8. WAL journal mode. Single-writer connection pool (max_connections = 1). Schema uses idempotent DDL (`CREATE TABLE IF NOT EXISTS`), safe to re-run on every startup.

### 7.1 `session`

| Column | Type | Constraints | Description |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY NOT NULL | UUID |
| `owner_user_id` | TEXT | NOT NULL | Slack user ID of the session owner |
| `workspace_root` | TEXT | NOT NULL | Absolute path to workspace directory |
| `status` | TEXT | NOT NULL, CHECK IN (`'created'`, `'active'`, `'paused'`, `'terminated'`, `'interrupted'`) | Lifecycle status |
| `prompt` | TEXT | nullable | Initial task prompt |
| `mode` | TEXT | NOT NULL, CHECK IN (`'remote'`, `'local'`, `'hybrid'`) | Operational routing mode |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp |
| `updated_at` | TEXT | NOT NULL | ISO 8601 timestamp |
| `terminated_at` | TEXT | nullable | ISO 8601 timestamp |
| `last_tool` | TEXT | nullable | Name of last MCP tool called |
| `nudge_count` | INTEGER | NOT NULL DEFAULT 0 | Consecutive nudge attempts |
| `stall_paused` | INTEGER | NOT NULL DEFAULT 0 | Whether stall detection is paused (0/1) |
| `progress_snapshot` | TEXT | nullable | JSON-serialized `Vec<ProgressItem>` |

**Valid Status Transitions:**

| From | Allowed To |
|---|---|
| `created` | `active` |
| `active` | `paused`, `terminated`, `interrupted` |
| `paused` | `active`, `terminated`, `interrupted` |
| `interrupted` | `active` |

### 7.2 `approval_request`

| Column | Type | Constraints | Description |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY NOT NULL | UUID |
| `session_id` | TEXT | NOT NULL | FK to session |
| `title` | TEXT | NOT NULL | Proposal summary |
| `description` | TEXT | nullable | Contextual details |
| `diff_content` | TEXT | NOT NULL | Unified diff or raw content |
| `file_path` | TEXT | NOT NULL | Target file relative to workspace root |
| `risk_level` | TEXT | NOT NULL, CHECK IN (`'low'`, `'high'`, `'critical'`) | Risk classification |
| `status` | TEXT | NOT NULL, CHECK IN (`'pending'`, `'approved'`, `'rejected'`, `'expired'`, `'consumed'`, `'interrupted'`) | Lifecycle status |
| `original_hash` | TEXT | NOT NULL | SHA-256 hash of file at proposal time |
| `slack_ts` | TEXT | nullable | Slack message timestamp |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp |
| `consumed_at` | TEXT | nullable | ISO 8601 timestamp when diff was applied |

### 7.3 `checkpoint`

| Column | Type | Constraints | Description |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY NOT NULL | UUID |
| `session_id` | TEXT | NOT NULL | FK to session |
| `label` | TEXT | nullable | Human-readable label |
| `session_state` | TEXT | NOT NULL | JSON-serialized session state |
| `file_hashes` | TEXT | NOT NULL | JSON-serialized `HashMap<String, String>` (path → SHA-256) |
| `workspace_root` | TEXT | NOT NULL | Workspace root at checkpoint time |
| `progress_snapshot` | TEXT | nullable | JSON-serialized progress items |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp |

### 7.4 `continuation_prompt`

| Column | Type | Constraints | Description |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY NOT NULL | UUID |
| `session_id` | TEXT | NOT NULL | FK to session |
| `prompt_text` | TEXT | NOT NULL | Raw prompt text |
| `prompt_type` | TEXT | NOT NULL, CHECK IN (`'continuation'`, `'clarification'`, `'error_recovery'`, `'resource_warning'`) | Category |
| `elapsed_seconds` | INTEGER | nullable | Seconds since last interaction |
| `actions_taken` | INTEGER | nullable | Count of actions performed |
| `decision` | TEXT | nullable | Operator's decision |
| `instruction` | TEXT | nullable | Revised instruction text |
| `slack_ts` | TEXT | nullable | Slack message timestamp |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp |

### 7.5 `stall_alert`

| Column | Type | Constraints | Description |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY NOT NULL | UUID |
| `session_id` | TEXT | NOT NULL | FK to session |
| `last_tool` | TEXT | nullable | Name of last tool before stall |
| `last_activity_at` | TEXT | NOT NULL | ISO 8601 timestamp of last MCP activity |
| `idle_seconds` | INTEGER | NOT NULL | Elapsed idle time when alert was created |
| `nudge_count` | INTEGER | NOT NULL DEFAULT 0 | Number of nudge attempts |
| `status` | TEXT | NOT NULL, CHECK IN (`'pending'`, `'nudged'`, `'self_recovered'`, `'escalated'`, `'dismissed'`) | Alert status |
| `nudge_message` | TEXT | nullable | Custom nudge message |
| `progress_snapshot` | TEXT | nullable | JSON-serialized progress items |
| `slack_ts` | TEXT | nullable | Slack message timestamp |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp |

### 7.6 Indexes

| Index | Table | Column |
|---|---|---|
| `idx_approval_session` | `approval_request` | `session_id` |
| `idx_checkpoint_session` | `checkpoint` | `session_id` |
| `idx_prompt_session` | `continuation_prompt` | `session_id` |
| `idx_stall_session` | `stall_alert` | `session_id` |

### 7.7 Data Retention

Background hourly task purges data older than `retention_days` (default 30). Runs after the first hour, then repeats at 1-hour intervals.

**Deletion Order (children before parent):**

1. `stall_alert` (for terminated sessions older than cutoff)
2. `checkpoint`
3. `continuation_prompt`
4. `approval_request`
5. `session`

**Cutoff:** Sessions where `terminated_at IS NOT NULL AND terminated_at < (now - retention_days)`.

---

## 8. Domain Models

### 8.1 `Session`

| Field | Type | Description |
|---|---|---|
| `id` | `String` | UUID primary key |
| `owner_user_id` | `String` | Slack user ID (immutable after creation) |
| `workspace_root` | `String` | Absolute path to workspace directory |
| `status` | `SessionStatus` | Lifecycle status |
| `prompt` | `Option<String>` | Initial task prompt |
| `mode` | `SessionMode` | Operational routing mode |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last activity timestamp |
| `last_tool` | `Option<String>` | Most recently called MCP tool |
| `nudge_count` | `i64` | Consecutive nudge attempts for current stall |
| `stall_paused` | `bool` | Whether stall detection is paused |
| `terminated_at` | `Option<DateTime<Utc>>` | Termination timestamp |
| `progress_snapshot` | `Option<Vec<ProgressItem>>` | Last-reported progress |

**`SessionStatus` enum:** `Created`, `Active`, `Paused`, `Terminated`, `Interrupted`

**`SessionMode` enum:** `Remote`, `Local`, `Hybrid`

### 8.2 `ApprovalRequest`

| Field | Type | Description |
|---|---|---|
| `id` | `String` | UUID primary key |
| `session_id` | `String` | FK to session |
| `title` | `String` | Proposal summary |
| `description` | `Option<String>` | Contextual details |
| `diff_content` | `String` | Unified diff or raw content |
| `file_path` | `String` | Target file relative to workspace root |
| `risk_level` | `RiskLevel` | Risk classification |
| `status` | `ApprovalStatus` | Lifecycle status |
| `original_hash` | `String` | SHA-256 hash at proposal time |
| `slack_ts` | `Option<String>` | Slack message timestamp |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `consumed_at` | `Option<DateTime<Utc>>` | Application timestamp |

**`RiskLevel` enum:** `Low`, `High`, `Critical`

**`ApprovalStatus` enum:** `Pending`, `Approved`, `Rejected`, `Expired`, `Consumed`, `Interrupted`

### 8.3 `ContinuationPrompt`

| Field | Type | Description |
|---|---|---|
| `id` | `String` | UUID primary key |
| `session_id` | `String` | FK to session |
| `prompt_text` | `String` | Raw prompt text |
| `prompt_type` | `PromptType` | Category |
| `elapsed_seconds` | `Option<i64>` | Seconds since last interaction |
| `actions_taken` | `Option<i64>` | Count of actions performed |
| `decision` | `Option<PromptDecision>` | Operator's response |
| `instruction` | `Option<String>` | Revised instruction text |
| `slack_ts` | `Option<String>` | Slack message timestamp |
| `created_at` | `DateTime<Utc>` | Creation timestamp |

**`PromptType` enum:** `Continuation`, `Clarification`, `ErrorRecovery`, `ResourceWarning`

**`PromptDecision` enum:** `Continue`, `Refine`, `Stop`

### 8.4 `Checkpoint`

| Field | Type | Description |
|---|---|---|
| `id` | `String` | UUID primary key |
| `session_id` | `String` | FK to session |
| `label` | `Option<String>` | Human-readable label |
| `session_state` | `serde_json::Value` | Serialized session state |
| `file_hashes` | `HashMap<String, String>` | Map of file path → SHA-256 hash |
| `workspace_root` | `String` | Workspace root at checkpoint time |
| `progress_snapshot` | `Option<Vec<ProgressItem>>` | Progress at checkpoint time |
| `created_at` | `DateTime<Utc>` | Creation timestamp |

### 8.5 `StallAlert`

| Field | Type | Description |
|---|---|---|
| `id` | `String` | UUID primary key |
| `session_id` | `String` | FK to session |
| `last_tool` | `Option<String>` | Last tool before stall |
| `last_activity_at` | `DateTime<Utc>` | Last MCP activity timestamp |
| `idle_seconds` | `i64` | Elapsed idle time at alert creation |
| `nudge_count` | `i64` | Number of nudge attempts |
| `status` | `StallAlertStatus` | Alert lifecycle status |
| `nudge_message` | `Option<String>` | Custom nudge message from operator |
| `progress_snapshot` | `Option<Vec<ProgressItem>>` | Progress at alert time |
| `slack_ts` | `Option<String>` | Slack message timestamp |
| `created_at` | `DateTime<Utc>` | Creation timestamp |

**`StallAlertStatus` enum:** `Pending`, `Nudged`, `SelfRecovered`, `Escalated`, `Dismissed`

### 8.6 `ProgressItem`

| Field | Type | Description |
|---|---|---|
| `label` | `String` | Human-readable task description (must not be empty) |
| `status` | `ProgressStatus` | Current status |

**`ProgressStatus` enum:** `Done`, `InProgress`, `Pending`

### 8.7 `WorkspacePolicy`

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `false` | Master switch for auto-approve |
| `commands` | `Vec<String>` | `[]` | Shell commands that bypass approval (glob wildcards) |
| `tools` | `Vec<String>` | `[]` | MCP tool names that bypass approval |
| `file_patterns` | `FilePatterns` | `{}` | File pattern rules |
| `risk_level_threshold` | `RiskLevel` | `Low` | Maximum risk level for auto-approve |
| `log_auto_approved` | `bool` | `false` | Whether to post auto-approved actions to Slack |
| `summary_interval_seconds` | `u64` | `300` | Interval for summary notifications |

**`FilePatterns` struct:**

| Field | Type | Default | Description |
|---|---|---|---|
| `write` | `Vec<String>` | `[]` | Glob patterns for auto-approved file writes |
| `read` | `Vec<String>` | `[]` | Glob patterns for auto-approved file reads |

---

## 9. Transport Layer

### 9.1 Stdio Transport

**Purpose:** Serves MCP over stdin/stdout for direct invocation by agentic IDEs (Claude Code, GitHub Copilot CLI, Cursor, VS Code).

**Implementation:** Uses `rmcp::transport::io::stdio()`. Creates a single `AgentRcServer` instance.

**Connection:** The primary agent connects directly — no HTTP involved.

### 9.2 HTTP/SSE Transport

**Purpose:** Enables multiple concurrent agent connections via HTTP with Server-Sent Events.

**Endpoints:**

| Path | Method | Description |
|---|---|---|
| `/sse` | GET | SSE connection endpoint. Accepts optional `channel_id` query parameter. |
| `/message` | POST | MCP message endpoint for SSE-connected clients |

**Binding:** `127.0.0.1:{http_port}` (default port 3000).

**Per-Session Channel Override:** Each SSE connection extracts `channel_id` from the query string (`/sse?channel_id=C_WORKSPACE`). A connection semaphore serializes SSE establishment to prevent race conditions on the channel_id inbox.

**Concurrency:** Each inbound SSE connection creates a fresh `AgentRcServer` instance sharing the same `AppState`.

### 9.3 Shared Application State (`AppState`)

| Field | Type | Description |
|---|---|---|
| `config` | `Arc<GlobalConfig>` | Global configuration |
| `db` | `Arc<SqlitePool>` | SQLite connection pool |
| `slack` | `Option<Arc<SlackService>>` | Slack client (absent in local-only mode) |
| `pending_approvals` | `Arc<Mutex<HashMap<String, oneshot::Sender<ApprovalResponse>>>>` | Pending approval oneshots keyed by `request_id` |
| `pending_prompts` | `Arc<Mutex<HashMap<String, oneshot::Sender<PromptResponse>>>>` | Pending prompt oneshots keyed by `prompt_id` |
| `pending_waits` | `Arc<Mutex<HashMap<String, oneshot::Sender<WaitResponse>>>>` | Pending wait oneshots keyed by `session_id` |
| `stall_detectors` | `Option<Arc<Mutex<HashMap<String, StallDetectorHandle>>>>` | Per-session stall detectors keyed by `session_id` |
| `ipc_auth_token` | `Option<String>` | Shared secret for IPC authentication (random UUID per instance) |

### 9.4 Oneshot Response Types

**`ApprovalResponse`**: `{ status: String, reason: Option<String> }`

**`PromptResponse`**: `{ decision: String, instruction: Option<String> }`

**`WaitResponse`**: `{ status: String, instruction: Option<String> }`

---

## 10. Policy System

### 10.1 Policy File

**Location:** `{workspace_root}/.intercom/settings.json`

**Example:**

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

### 10.2 Policy Loader

**File:** `.intercom/settings.json` relative to workspace root.

| Condition | Behavior |
|---|---|
| Missing file/directory | Returns `WorkspacePolicy::default()` (deny-all) |
| Empty file | Returns deny-all + logs warning |
| Malformed JSON | Returns deny-all + logs warning |
| Valid JSON | Parses into `WorkspacePolicy` as-is; workspace policy is self-contained (ADR-0012) |

### 10.3 Policy Evaluator

**Evaluation Order:**

1. **Disabled** → deny all
2. **Risk level threshold** → deny if requested risk exceeds threshold. `critical` risk is **never** auto-approved regardless of threshold.
3. **Command matching** → approve if `tool_name` matches any regex in workspace `auto_approve_commands` (ADR-0012: global allowlist gate removed)
4. **Tool matching** → approve if `tool_name` is in workspace `tools` list
5. **File pattern matching** → approve if `context.file_path` matches any write/read glob pattern
6. **No match** → deny

**Risk Ordinal:** `Low` (0) < `High` (1) < `Critical` (2). Request risk must be ≤ threshold.

**Matched Rule Format:**
- `"command:<name>"` — matched via command rule
- `"tool:<name>"` — matched via tool rule
- `"file_pattern:<write|read>:<glob>"` — matched via file glob pattern

### 10.4 Policy Hot-Reload (Watcher)

**Library:** `notify` crate (recommended watcher).

**Behavior:**
- Watches the `.intercom/` directory per workspace for file create/modify/remove events on `settings.json`.
- On change, reloads the policy via `PolicyLoader` and updates the in-memory cache.
- Cache type: `PolicyCache = Arc<RwLock<HashMap<PathBuf, WorkspacePolicy>>>`.
- Watchers are registered when sessions start and unregistered when sessions terminate.
- If `.intercom/` directory doesn't exist yet, the watcher is stored but deferred.

---

## 11. Orchestrator

### 11.1 Session Manager

**Functions:**

| Function | Description |
|---|---|
| `pause_session(id, repo)` | Sets session status to `Paused` |
| `resume_session(id, repo)` | Sets session status to `Active` |
| `terminate_session(id, repo, child)` | Terminates session: 5-second grace period for child process, then force-kill. Sets status to `Terminated`. |
| `resolve_session(id?, user, repo)` | Resolves session by ID or most recently active for user. Validates ownership. |

### 11.2 Spawner

**Function:** `spawn_session(prompt, workspace_root, owner_user_id, config, session_repo, http_port)`

**Behavior:**

1. Canonicalizes `workspace_root` to an absolute path.
2. Enforces `max_concurrent_sessions` limit.
3. Verifies user authorization via `config.ensure_authorized()`.
4. Creates a `Session` record with status `Created` and mode `Remote`.
5. Builds SSE URL: `http://localhost:{http_port}/mcp`.
6. Spawns the host CLI process with:
   - Arguments: `host_cli_args` + `prompt`
   - Environment variables:
     - `INTERCOM_WORKSPACE_ROOT` — resolved workspace path
     - `INTERCOM_MCP_URL` — `/mcp?session_id=<id>` URL for the spawned agent
     - `INTERCOM_SESSION_ID` — session UUID
   - Working directory: workspace path
   - stdin: null, stdout: piped, stderr: piped
   - `kill_on_drop(true)`
7. Activates the session after successful spawn.

**Function:** `verify_session_owner(session, user_id)` — Returns `Unauthorized` error if user is not the owner.

### 11.3 Stall Detector

**Per-session timer** with configurable thresholds on a background tokio task.

**Events (`StallEvent` enum):**

| Event | Fields | Description |
|---|---|---|
| `Stalled` | `session_id`, `idle_seconds` | Agent has been idle past the inactivity threshold |
| `AutoNudge` | `session_id`, `nudge_count` | Auto-nudge triggered after escalation interval |
| `Escalated` | `session_id`, `nudge_count` | Max retries exceeded — escalated alert |
| `SelfRecovered` | `session_id` | Agent resumed activity while stall alert was active |

**Configuration (per-detector):**

| Setting | From Config | Description |
|---|---|---|
| `inactivity_threshold` | `stall.inactivity_threshold_seconds` | Idle time before first stall event |
| `escalation_interval` | `stall.escalation_threshold_seconds` | Time between auto-nudges |
| `max_retries` | `stall.max_retries` | Max nudges before escalation |

**`StallDetectorHandle` methods:**

| Method | Description |
|---|---|
| `reset()` | Reset the inactivity timer (called on every tool activity or heartbeat) |
| `pause()` | Pause stall detection (e.g., during long-running server operations) |
| `resume()` | Resume stall detection after pause; resets the timer |
| `is_stalled()` | Check whether the detector currently considers the session stalled |
| `session_id()` | Get the session ID this handle controls |

**Timer Implementation:**
- Uses `tokio::sync::Notify` for reset coordination.
- Uses `AtomicBool` for pause/stalled state flags.
- When paused, polls at 50ms intervals until unpaused.
- Escalation loop: after stall detected, waits `escalation_interval`, then auto-nudges up to `max_retries`. After max retries, emits `Escalated` and waits for manual intervention or reset.

### 11.4 Checkpoint Manager

**Functions:**

| Function | Description |
|---|---|
| `create_checkpoint(session_id, label, session_repo, checkpoint_repo)` | Snapshot session state and workspace file hashes (SHA-256, non-recursive). |
| `restore_checkpoint(checkpoint_id, checkpoint_repo)` | Load checkpoint and detect workspace divergences. Returns `(Checkpoint, Vec<DivergenceEntry>)`. |
| `hash_workspace_files(root)` | Compute SHA-256 hashes for all regular files (non-recursive) in a directory. |

**`DivergenceKind` enum:**

| Variant | Description |
|---|---|
| `Modified` | File content has changed since checkpoint |
| `Deleted` | File existed at checkpoint time but is now missing |
| `Added` | File was added after the checkpoint |

---

## 12. Error Types

**Type alias:** `Result<T> = std::result::Result<T, AppError>`

**`AppError` enum:**

| Variant | Display Prefix | Description |
|---|---|---|
| `Config(String)` | `config:` | Configuration parsing or validation failure |
| `Db(String)` | `db:` | Persistence failure with SQLite |
| `Slack(String)` | `slack:` | Slack API or Socket Mode failure |
| `Mcp(String)` | `mcp:` | MCP protocol or tool dispatch failure |
| `Diff(String)` | `diff:` | Diff parsing or file-write failure |
| `Policy(String)` | `policy:` | Policy evaluation or loading failure |
| `Ipc(String)` | `ipc:` | IPC communication failure |
| `PathViolation(String)` | `path violation:` | File path escapes workspace root |
| `PatchConflict(String)` | `patch conflict:` | Patch application failed due to content divergence |
| `NotFound(String)` | `not found:` | Requested entity does not exist |
| `Unauthorized(String)` | `unauthorized:` | Caller is not authorized |
| `AlreadyConsumed(String)` | `already consumed:` | Approval or prompt has already been consumed |

**From implementations:** `toml::de::Error` → `Config`, `sqlx::Error` → `Db`.

**Convention:** Error messages are lowercase and do not end with a period.

---

## 13. CLI Arguments

### 13.1 `agent-intercom`

| Argument | Type | Required | Default | Description |
|---|---|---|---|---|
| `--config` | `PathBuf` | **Yes** | — | Path to the TOML configuration file |
| `--log-format` | `text` \| `json` | No | `text` | Log output format |
| `--workspace` | `PathBuf` | No | — | Override the default workspace root |

### 13.2 `agent-intercom-ctl`

| Argument | Type | Required | Default | Description |
|---|---|---|---|---|
| `--ipc-name` | `string` | No | `"agent-intercom"` | IPC socket name |
| Subcommand | — | **Yes** | — | `list`, `approve`, `reject`, `resume`, `mode` |

---

## 14. Server Lifecycle

### 14.1 Bootstrap Sequence

1. Parse CLI arguments (`clap`).
2. Initialize tracing (text or JSON format, respects `RUST_LOG` env filter).
3. Load `config.toml` → `GlobalConfig`.
4. Override workspace root from CLI if `--workspace` provided.
5. Load Slack credentials from keychain / env vars.
6. Connect to SQLite database (creates file + parent directories if needed, bootstraps schema).
7. Start retention service (hourly purge background task).
8. Build shared `AppState`.
9. Start Slack Socket Mode client (if tokens configured).
10. Generate random IPC auth token (UUID).
11. Check for interrupted sessions from prior crash (posts recovery summary to Slack).
12. Start stdio transport (primary agent connection).
13. Start HTTP/SSE transport.
14. Log "MCP server ready".
15. Wait for shutdown signal (Ctrl+C or SIGTERM).

### 14.2 Graceful Shutdown

On shutdown signal:

1. Cancel all background tasks via `CancellationToken`.
2. Mark all pending approval requests as `Interrupted`.
3. Mark all pending prompts as `Interrupted` (decision set to `Stop`).
4. Mark all active/paused sessions as `Interrupted`.
5. Post final notification to Slack: "⚠️ Server shutting down. N session(s), N approval(s), N prompt(s) interrupted."
6. Brief sleep (500ms) to let the Slack queue drain.
7. Wait for stdio, SSE, and retention task handles to complete.

### 14.3 Startup Recovery

On startup, the server checks for interrupted sessions from a prior crash:

1. Queries all sessions with status `Interrupted`.
2. Counts pending approvals and prompts across those sessions.
3. Posts recovery summary to Slack: "🔄 Server restarted. Found N interrupted session(s) with N pending approval(s) and N pending prompt(s). Agents can use `recover_state` to resume."

---

## 15. Diff & Path Safety

### 15.1 Path Validation (`path_safety.rs`)

All file operations are validated against the workspace root:

1. Canonicalizes the workspace root.
2. Normalizes the candidate path by processing components:
   - `..` (ParentDir) → pops from the normalized stack; error if stack is empty.
   - `.` (CurDir) → ignored.
   - Root/Prefix → rejected (absolute paths not allowed; use relative).
   - Normal → pushed to stack.
3. Joins the normalized relative path to the workspace root.
4. Verifies the result `starts_with(workspace_root)`.
5. **Symlink escape detection:** If the path exists, `canonicalize()` resolves symlinks and re-checks the containment invariant.

**Fast path:** Already-absolute paths that start with the workspace root skip component-level normalization.

### 15.2 Patch Application (`patcher.rs`)

Uses `diffy` crate:

1. Reads the current file contents.
2. Parses the unified diff via `Patch::from_str()`.
3. Applies the patch via `diffy::apply()`.
4. Writes the patched content atomically.

### 15.3 Atomic File Writer (`writer.rs`)

Uses `tempfile` crate:

1. Validates the path against workspace root.
2. Creates parent directories if needed.
3. Writes to a `NamedTempFile` in the same directory.
4. Atomically renames via `persist()` to prevent partial writes on crash.

### 15.4 File Hashing (`util.rs`)

`compute_file_hash(path)`:
- Returns the SHA-256 hex digest of the file contents.
- Returns `"new_file"` if the file does not exist.
- Uses `tokio::fs::read` for async I/O.

---

## 16. Architecture Decision Records

Located in `docs/adrs/`:

| ADR | Title |
|---|---|
| 0001 | Credential Loading: Keyring with Env Fallback |
| 0002 | SurrealDB Idempotent Schema Bootstrap |
| 0003 | MCP Handler AppState Refactor |
| 0004 | SurrealDB Record ID Serde Pattern |
| 0005 | Stall Detector Architecture |
| 0006 | SurrealDB Schemafull Nested Fields |
| 0007 | SurrealDB Flexible Type for Dynamic Maps |
| 0008 | SurrealDB Count Group All |
| 0009 | IPC JSON-Line Protocol Over Local Sockets |
| 0010 | Centralized Interaction Dispatch Guards |
| 0011 | Reconnect & Repost Pending Messages |

**Note:** ADRs 0002, 0004, 0006, 0007, 0008 reference SurrealDB which has since been replaced by SQLite (see `specs/002-sqlite-migration/`).

---

## 17. Dependencies

### 17.1 Production Dependencies

| Crate | Version | Purpose |
|---|---|---|
| `axum` | 0.8 | HTTP/SSE transport |
| `chrono` | 0.4 | Timestamps (serde, clock features) |
| `clap` | 4.5 | CLI argument parsing (derive feature) |
| `diffy` | 0.4 | Unified diff parsing and patch application |
| `glob` | 0.3 | File pattern matching for auto-approve policies |
| `interprocess` | 2.0 | IPC named pipes / Unix domain sockets (tokio feature) |
| `keyring` | 3 | OS keychain credential access |
| `notify` | 6.1 | Filesystem watcher for policy hot-reload |
| `reqwest` | 0.13.2 | HTTP client (rustls, no default features) |
| `rmcp` | 0.5 | MCP SDK (server, transport-sse-server, transport-io features) |
| `serde` | 1.0 | Serialization (derive feature) |
| `serde_json` | 1.0 | JSON serialization |
| `sha2` | 0.10 | SHA-256 file integrity hashing |
| `slack-morphism` | 2.17 | Slack Socket Mode client (hyper feature) |
| `sqlx` | 0.8 | SQLite async driver (runtime-tokio, sqlite, json, chrono, macros features) |
| `tempfile` | 3.10 | Atomic file writes |
| `tokio` | 1.37 | Async runtime (full feature set) |
| `tokio-util` | 0.7.18 | `CancellationToken` for graceful shutdown (rt feature) |
| `toml` | 0.8 | TOML config file parsing |
| `tracing` | 0.1 | Structured logging |
| `tracing-subscriber` | 0.3 | Logging subscriber (env-filter, fmt, json features) |
| `uuid` | 1.7 | Entity IDs (v4, serde features) |

### 17.2 Dev Dependencies

| Crate | Version | Purpose |
|---|---|---|
| `serial_test` | 3 | Sequential test execution |

### 17.3 Workspace Lints

```toml
[workspace.lints.rust]
unsafe_code = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
pedantic = "deny"
unwrap_used = "deny"
expect_used = "deny"
```

Both `src/main.rs` and `ctl/main.rs` have `#![forbid(unsafe_code)]`.

---

## Slack Client Internals

### Socket Mode

Uses `slack-morphism` 2.17 Socket Mode — outbound-only WebSocket (no inbound firewall ports required).

### Message Queue

- Buffered send queue (capacity 256).
- Retry with exponential backoff: 1s initial delay, 30s max, 5 max retries.
- Respects `Retry-After` headers from the Slack API.

### Methods

| Method | Description |
|---|---|
| `enqueue(msg)` | Queue a message for async posting |
| `post_message_direct(msg)` | Post a message synchronously, returns the Slack timestamp |
| `update_message(channel, ts, blocks)` | Update an existing message (used for double-submission prevention) |
| `upload_file(channel, filename, content, thread_ts)` | Upload content as a Slack file snippet |
| `fetch_recent_history(channel, limit)` | Fetch recent channel history |
| `fetch_history_with_more(channel, limit)` | Fetch history returning `(messages, has_more)` |
| `open_modal(trigger_id, view)` | Open a Slack modal |

### Reconnection

On Socket Mode reconnect (`hello` event): re-posts all pending approvals and prompts from the database.

### Mode-Aware Routing

| Method | Active In |
|---|---|
| `should_post_to_slack()` | `Remote`, `Hybrid` |
| `should_post_to_ipc()` | `Local`, `Hybrid` |

### Block Kit Builders (`blocks.rs`)

| Builder | Description |
|---|---|
| `severity_section(level, message)` | Formats with emoji: ✅ success, ⚠️ warning, ❌ error, ℹ️ info |
| `approval_buttons(request_id)` | Accept / Reject buttons |
| `prompt_buttons(prompt_id)` | Continue / Refine / Stop buttons |
| `nudge_buttons(alert_id)` | Nudge / Nudge with Instructions / Stop buttons |
| `wait_buttons(session_id)` | Resume / Resume with Instructions / Stop Session buttons |
| `text_section(text)` | Plain text section |
| `diff_section(diff)` | Code-formatted diff section |
| `action_buttons(block_id, buttons)` | Generic action block builder |

---

## Per-Request Context (`ToolContext`)

Every MCP tool handler receives a `ToolContext` containing:

| Field | Type | Description |
|---|---|---|
| `session` | `Session` | Active session for this request |
| `workspace_root` | `PathBuf` | Absolute workspace root for path validation |
| `config` | `Arc<GlobalConfig>` | Global configuration |
| `db` | `Arc<SqlitePool>` | SQLite connection pool |
| `slack` | `Option<Arc<SlackService>>` | Slack client (absent in local-only mode) |

---

## Run Script (`run-debug.ps1`)

Loads Slack credentials from Windows user-level environment variables and starts the debug binary:

```powershell
$env:SLACK_APP_TOKEN = [System.Environment]::GetEnvironmentVariable("SLACK_APP_TOKEN", "User")
$env:SLACK_BOT_TOKEN = [System.Environment]::GetEnvironmentVariable("SLACK_BOT_TOKEN", "User")
$env:SLACK_TEAM_ID   = [System.Environment]::GetEnvironmentVariable("SLACK_TEAM_ID", "User")
$env:SLACK_MEMBER_IDS = [System.Environment]::GetEnvironmentVariable("SLACK_MEMBER_IDS", "User")
$env:RUST_LOG        = "info"

.\target\debug\agent-intercom.exe --config config.toml
```
