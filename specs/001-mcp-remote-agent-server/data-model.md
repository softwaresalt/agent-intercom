# Data Model: MCP Remote Agent Server

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)
**Date**: 2026-02-10 (updated from 2026-02-09)

## Entity Relationship Diagram

```text
┌─────────────────┐      ┌──────────────────┐      ┌───────────────────┐
│     Session      │──1:N─│    Checkpoint     │      │  WorkspacePolicy  │
│                  │      │                  │      │ (in-memory, per   │
│  session_id  PK  │      │  checkpoint_id PK│      │  workspace_root)  │
│  owner_user_id   │      │  session_id   FK │      │                   │
│  workspace_root  │      │  label           │      │  auto_approve     │
│  status          │      │  session_state   │      │  commands[]       │
│  prompt          │      │  file_hashes{}   │      │  tools[]          │
│  mode            │      │  progress_snap   │      │  file_patterns{}  │
│  created_at      │      │  workspace_root  │      │  risk_threshold   │
│  updated_at      │      │  created_at      │      └───────────────────┘
│  last_tool       │      └──────────────────┘
│  nudge_count     │      ┌──────────────────┐      ┌───────────────────┐
│  stall_paused    │──1:N─│ ApprovalRequest   │      │  RegistryCommand  │
│  progress_snap   │      │                  │      │   (config.toml)   │
│  terminated_at   │      │  request_id   PK │      │                   │
└─────────────────┘      │  session_id   FK │      │  alias         PK │
        │                 │  title           │      │  command          │
        │                 │  description     │      └───────────────────┘
        │ 1:N             │  diff_content    │
        ▼                 │  file_path       │      ┌───────────────────┐
┌─────────────────┐      │  risk_level      │      │   GlobalConfig    │
│ContPrompt       │      │  status          │      │   (config.toml    │
│                  │      │  original_hash   │      │   + keychain/env) │
│  prompt_id   PK  │      │  slack_ts        │      │                   │
│  session_id  FK  │      │  created_at      │      │  default_ws_root  │
│  prompt_text     │      │  consumed_at     │      │  slack_app_token* │
│  prompt_type     │      └──────────────────┘      │  slack_bot_token* │
│  elapsed_secs    │                                │  channel_id       │
│  actions_taken   │                                │  authorized_users │
│  decision        │                                │  max_sessions     │
│  instruction     │                                │  timeouts{}       │
│  slack_ts        │      ┌──────────────────┐      │  stall_config{}   │
│  created_at      │      │   StallAlert     │      │  retention_days   │
└─────────────────┘      │                  │      └───────────────────┘
                          │  alert_id     PK │      * loaded from OS
                          │  session_id   FK │        keychain or env
                          │  last_tool       │
                          │  last_activity_at│
                          │  idle_seconds    │
                          │  nudge_count     │
                          │  status          │
                          │  nudge_message   │
                          │  progress_snap   │
                          │  slack_ts        │
                          │  created_at      │
                          └──────────────────┘

Relationships (all via session_id FK):
  Session ──1:N── Checkpoint
  Session ──1:N── ApprovalRequest
  Session ──1:N── ContinuationPrompt
  Session ──1:N── StallAlert
```

## Entities

### Session

Represents a tracked instance of an agent process connected to the MCP server.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `session_id` | `string` | PK, unique, generated UUID | Unique identifier for the session |
| `owner_user_id` | `string` | Required, immutable after creation | Slack user ID bound at creation time |
| `workspace_root` | `string` | Required, immutable after creation | Absolute path to the workspace directory for this session |
| `status` | `string` | Required, enum | Current lifecycle state |
| `prompt` | `string` | Optional | Initial instruction/prompt for the session |
| `mode` | `string` | Required, default `"remote"` | Operational mode: `remote`, `local`, `hybrid` |
| `created_at` | `datetime` | Required, auto-set | When the session was created |
| `updated_at` | `datetime` | Required, auto-updated | Last MCP activity timestamp (tool call, response, heartbeat) |
| `terminated_at` | `datetime` | Optional | When the session was terminated (used for retention purge) |
| `last_tool` | `string` | Optional | Name of the last tool called by the agent |
| `nudge_count` | `int` | Required, default `0` | Consecutive auto-nudge attempts for current stall |
| `stall_paused` | `bool` | Required, default `false` | Whether stall detection is paused (long-running op) |
| `progress_snapshot` | `object` | Optional | Last-reported progress snapshot from `heartbeat` (ordered list of `{label, status}` items) |

**Status values**: `created` → `active` → `paused` | `terminated` | `interrupted`

**State machine**:

```text
created ──▶ active ──▶ paused ──▶ active (resume)
              │                      │
              ├──▶ terminated        ├──▶ terminated
              └──▶ interrupted       └──▶ interrupted
```

**Validation rules**:

- `owner_user_id` must be in the global `authorized_user_ids` list at creation time.
- `owner_user_id` cannot be changed after session creation.
- `workspace_root` must be a valid absolute path and cannot be changed after creation.
- Only the session owner may interact with the session's requests and commands.
- Total active + paused sessions must not exceed `max_concurrent_sessions`.
- `terminated_at` is set when status transitions to `terminated` or `interrupted`. Used by the retention purge service (FR-035).
- `progress_snapshot` is updated by the `heartbeat` tool. When provided, replaces the previous snapshot. When omitted from heartbeat, existing snapshot is preserved.

### ApprovalRequest

Represents a pending human decision on a code proposal.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `request_id` | `string` | PK, unique, generated UUID | Unique identifier |
| `session_id` | `string` | FK → Session, required | Owning session |
| `title` | `string` | Required | Concise summary of the proposal |
| `description` | `string` | Optional | Contextual details |
| `diff_content` | `string` | Required | Unified diff or raw file content |
| `file_path` | `string` | Required | Target file path relative to workspace root |
| `risk_level` | `string` | Required, enum: `low`, `high`, `critical` | Risk classification |
| `status` | `string` | Required, enum | Current state of the request |
| `original_hash` | `string` | Required | SHA-256 hash of file at proposal time |
| `slack_ts` | `string` | Optional | Slack message timestamp for updates |
| `created_at` | `datetime` | Required, auto-set | When the request was created |
| `consumed_at` | `datetime` | Optional | When the approved diff was applied |

**Status values**: `pending` → `approved` → `consumed` | `rejected` | `expired` | `interrupted`

**Validation rules**:

- `file_path` must resolve within `workspace_root` (path traversal check).
- Only one `pending` approval request per session at a time (agent blocks until resolved).
- Transition to `consumed` requires prior `approved` status and valid `request_id` passed to `accept_diff`.
- Transition to `consumed` is idempotent — duplicate `accept_diff` calls return `already_consumed` error.

### Checkpoint

A named snapshot of a session's state at a point in time.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `checkpoint_id` | `string` | PK, unique, generated UUID | Unique identifier |
| `session_id` | `string` | FK → Session, required | Owning session |
| `label` | `string` | Optional | Human-readable name (e.g., "before-refactor") |
| `session_state` | `object` | Required | Serialized session state snapshot |
| `file_hashes` | `object` | Required | Map of `file_path → SHA-256 hash` for divergence detection |
| `workspace_root` | `string` | Required | Workspace root at checkpoint time (for restore fidelity) |
| `progress_snapshot` | `object` | Optional | Session's progress snapshot at checkpoint time |
| `created_at` | `datetime` | Required, auto-set | When the checkpoint was created |

**Validation rules**:

- On restore, each `file_hashes` entry is compared to the current file's hash. Diverged files trigger a warning to the operator requiring explicit confirmation before proceeding.
- Restoring a checkpoint terminates any currently active session for that session ID.

### ContinuationPrompt

A forwarded meta-prompt from an agent requiring operator decision.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `prompt_id` | `string` | PK, unique, generated UUID | Unique identifier |
| `session_id` | `string` | FK → Session, required | Owning session |
| `prompt_text` | `string` | Required | Raw text of the continuation prompt |
| `prompt_type` | `string` | Required, enum | Category of the prompt |
| `elapsed_seconds` | `int` | Optional | Seconds since last user interaction |
| `actions_taken` | `int` | Optional | Count of actions performed in this iteration |
| `decision` | `string` | Optional, enum | Operator's response |
| `instruction` | `string` | Optional | Revised instruction text (when decision is `refine`) |
| `slack_ts` | `string` | Optional | Slack message timestamp |
| `created_at` | `datetime` | Required, auto-set | When the prompt was created |

**Prompt type values**: `continuation`, `clarification`, `error_recovery`, `resource_warning`

**Decision values**: `continue`, `refine`, `stop`

**Validation rules**:

- `error_recovery` prompts are never auto-approved regardless of workspace policy.
- Auto-timeout decision defaults to `continue` after `prompt_timeout_seconds`.

### StallAlert

A watchdog notification triggered by detected agent inactivity.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `alert_id` | `string` | PK, unique, generated UUID | Unique identifier |
| `session_id` | `string` | FK → Session, required | Owning session |
| `last_tool` | `string` | Optional | Name of last tool called before stall |
| `last_activity_at` | `datetime` | Required | Timestamp of last detected MCP activity |
| `idle_seconds` | `int` | Required | Elapsed idle time when alert was created |
| `nudge_count` | `int` | Required, default `0` | Number of nudge attempts for this alert |
| `status` | `string` | Required, enum | Current state of the alert |
| `nudge_message` | `string` | Optional | Custom nudge message from operator |
| `progress_snapshot` | `object` | Optional | Session's progress snapshot at alert time |
| `slack_ts` | `string` | Optional | Slack message timestamp for updates |
| `created_at` | `datetime` | Required, auto-set | When the alert was created |

**Status values**: `pending` → `nudged` | `self_recovered` | `escalated` | `dismissed`

**Validation rules**:

- Only one active stall alert (`pending` or `nudged`) per session at a time.
- Self-recovery (agent resumes activity) dismisses the alert and disables Slack buttons via `chat.update`.
- After `max_retries` auto-nudges, status transitions to `escalated` with `@channel` mention.

### WorkspacePolicy (in-memory, per workspace root, not persisted)

The auto-approve configuration loaded from `.monocoque/settings.json` relative to the session's `workspace_root`.

| Field | Type | Description |
|-------|------|-------------|
| `enabled` | `bool` | Master switch for auto-approve |
| `commands` | `string[]` | Shell commands that bypass approval (glob wildcards allowed) |
| `tools` | `string[]` | MCP tool names that bypass approval |
| `file_patterns.write` | `string[]` | Glob patterns for auto-approved file writes |
| `file_patterns.read` | `string[]` | Glob patterns for auto-approved file reads |
| `risk_level_threshold` | `string` | Maximum risk level for auto-approve (`low`, `high`) |
| `log_auto_approved` | `bool` | Whether to post auto-approved actions to Slack |
| `summary_interval_seconds` | `int` | Interval for summary notifications |

**Validation rules**:

- `commands` entries must exist in the global `config.toml` allowlist — workspace policy cannot introduce new commands.
- On parse error, fall back to "require approval for everything" and log warning to console and Slack.
- Hot-reloaded via `notify` file watcher without server restart.

### GlobalConfig (config.toml + OS keychain/env vars, read-only at startup)

| Field | Type | Description |
|-------|------|-------------|
| `default_workspace_root` | `string` | Default workspace root for the primary stdio agent (optional; per-session override takes precedence) |
| `slack.app_token` | `string` | Slack App-Level Token for Socket Mode — loaded from OS keychain (`monocoque-agent-rc/slack_app_token`) or `SLACK_APP_TOKEN` env var (FR-038, FR-039, FR-046) |
| `slack.bot_token` | `string` | Slack Bot User OAuth Token — loaded from OS keychain (`monocoque-agent-rc/slack_bot_token`) or `SLACK_BOT_TOKEN` env var (FR-038, FR-039, FR-046) |
| `slack.team_id` | `string` | Slack workspace team ID — loaded from OS keychain (`monocoque-agent-rc/slack_team_id`) or `SLACK_TEAM_ID` env var; optional (FR-041) |
| `slack.channel_id` | `string` | Default target Slack channel ID. Can be overridden per-session via the `?channel_id=` query parameter on the SSE endpoint (FR-042, FR-043) |
| `authorized_user_ids` | `string[]` | Slack user IDs permitted to create sessions |
| `max_concurrent_sessions` | `int` | Maximum concurrent sessions (default: 3) |
| `host_cli` | `string` | CLI binary for spawning sessions (e.g., `claude`, `gh`) |
| `host_cli_args` | `string[]` | Default arguments for the host CLI |
| `timeouts.approval_seconds` | `int` | Approval request timeout (default: 3600) |
| `timeouts.prompt_seconds` | `int` | Continuation prompt timeout (default: 1800) |
| `timeouts.wait_seconds` | `int` | Wait-for-instruction timeout (default: 0 = indefinite) |
| `stall.enabled` | `bool` | Enable stall detection (default: true) |
| `stall.inactivity_threshold_seconds` | `int` | Idle threshold before alert (default: 300) |
| `stall.escalation_threshold_seconds` | `int` | Wait before auto-nudge (default: 120) |
| `stall.max_retries` | `int` | Max auto-nudge attempts before escalation (default: 3) |
| `stall.default_nudge_message` | `string` | Default nudge continuation message |
| `commands` | `map<string, string>` | Custom command alias → shell command mapping |
| `http_port` | `int` | Port for SSE transport (default: 3000) |
| `ipc_name` | `string` | Named pipe / socket name (default: `monocoque-agent-rc`) (FR-048) |
| `retention_days` | `int` | Days after session termination before data is purged (default: 30) |

### RegistryCommand (derived from GlobalConfig)

| Field | Type | Description |
|-------|------|-------------|
| `alias` | `string` | User-facing command name (e.g., `status`) |
| `command` | `string` | Full shell command (e.g., `git status`) |

**Validation rules**:

- Only commands in this registry may be executed remotely.
- Workspace policy can auto-approve registered commands but cannot add new ones.
