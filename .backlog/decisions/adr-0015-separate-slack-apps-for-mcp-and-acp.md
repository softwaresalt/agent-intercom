---
id: adr-0015
title: "ADR-0015: Separate Slack Apps for MCP and ACP Protocols"
type: decision
created_date: '2026-03-27 22:39'
tags:
  - adr
---
# ADR-0015: Separate Slack Apps for MCP and ACP Protocols

**Status**: Accepted  
**Date**: 2026-03-01  
**Phase**: 005-intercom-acp-server, Phase 9  

## Context

The `agent-intercom` server supports two protocol modes: MCP (Model Context
Protocol via rmcp) and ACP (Agent Communication Protocol via NDJSON
stdin/stdout streams). Both modes use Slack as the operator interface for
approvals, prompt forwarding, status updates, and session management.

During Phase 9 implementation, the question arose: when an operator runs both
an MCP bridge and an ACP bridge against the same workspace, how should Slack
messages be routed to avoid thread collisions and interaction confusion?

Three options were evaluated:

| Option | Description | Feasibility |
|--------|-------------|-------------|
| A | Two server processes, two Slack apps | Works today, no code changes |
| B | Two server processes, one Slack app | Broken — Socket Mode allows one WebSocket per app token |
| C | One process, dual-mode with per-session driver routing | Significant refactor, couples failure modes |

### Current Architecture Constraints

1. **`AppState.driver`** is a single `Arc<dyn AgentDriver>` chosen at startup
   based on `--mode mcp` or `--mode acp`. Slack event handlers call
   `state.driver.resolve_clearance()` etc. with no per-session routing.

2. **Socket Mode** establishes exactly one WebSocket per app token. Two
   processes sharing the same token would steal each other's connection.

3. **`session.protocol_mode`** (MCP or ACP) is stored in the database but is
   not used for driver dispatch — it is metadata only.

## Decision

Use **separate Slack apps** for MCP and ACP server instances (Option A).

Each `agent-intercom` process runs with its own Slack app credentials
(bot token + app token) stored in the OS keychain under distinct service
names or environment variable prefixes.

Both apps may post to the **same Slack channel** — Slack threads are
per-bot, so there is no collision. Alternatively, operators may configure
different channels per app for complete visual separation.

### Credential Resolution (Mode-Prefixed)

`load_credentials(mode)` resolves each credential using a four-step
fallback chain. The first non-empty value wins:

| Priority | Source | ACP Example | MCP Example |
|----------|--------|-------------|-------------|
| 1 | Keyring `agent-intercom-{mode}` | `agent-intercom-acp` / `slack_bot_token` | `agent-intercom` / `slack_bot_token` |
| 2 | Env var `{VAR}_{MODE}` | `SLACK_BOT_TOKEN_ACP` | `SLACK_BOT_TOKEN` |
| 3 | Keyring `agent-intercom` (shared) | `agent-intercom` / `slack_bot_token` | *(same as #1)* |
| 4 | Env var `{VAR}` (shared) | `SLACK_BOT_TOKEN` | *(same as #2)* |

MCP is the default protocol, so its mode suffix is empty — steps 1–2
are identical to 3–4, preserving full backwards compatibility.

ACP-mode env vars: `SLACK_APP_TOKEN_ACP`, `SLACK_BOT_TOKEN_ACP`,
`SLACK_TEAM_ID_ACP`, `SLACK_MEMBER_IDS_ACP`.

## Consequences

### Positive

- **Backwards compatible**: existing deployments with un-prefixed env vars
  continue to work unchanged for MCP mode.
- **Visual clarity**: different bot names and avatars make MCP vs ACP messages
  immediately distinguishable in Slack.
- **Independent rate limits**: ACP sessions (potentially many concurrent child
  processes) cannot exhaust the MCP app's Slack API budget.
- **Failure isolation**: a crash or hang in one server does not affect the
  other's Slack connectivity.
- **Thread isolation for free**: each bot's messages naturally form separate
  threads even in the same channel.

### Negative

- **Two Slack apps to manage**: operators must install and configure two apps
  in their Slack workspace (two OAuth flows, two sets of scopes).
- **Two credential entries**: two keychain entries (or two sets of environment
  variables) for Slack tokens.
- **Two config files**: each process needs its own `config.toml` (or the same
  file with a `--profile` flag, which does not exist yet).

### Risks

- If a future requirement demands a unified Slack experience (single bot,
  single thread per workspace), this decision would need to be revisited
  with Option C (dual-mode server). That refactor is estimated at 2–3 phases
  of work: dispatcher driver, per-session routing in all Slack handlers, and
  concurrent transport management.

## IPC Pipe Isolation

When two server instances run on the same machine, they must bind different
named pipes to avoid collisions. The server auto-suffixes the `ipc_name`
config field when running in ACP mode and the name is still the default:

| Mode | Default `ipc_name` | Named Pipe (Windows) |
|------|--------------------|----------------------|
| MCP  | `agent-intercom`     | `\\.\pipe\agent-intercom` |
| ACP  | `agent-intercom-acp` | `\\.\pipe\agent-intercom-acp` |

The `agent-intercom-ctl` companion CLI has a `--mode` flag that performs
the same derivation:

```powershell
# Talk to the MCP server (default)
agent-intercom-ctl list

# Talk to the ACP server
agent-intercom-ctl --mode acp list

# Explicit override (takes precedence over --mode)
agent-intercom-ctl --ipc-name custom-name list
```

## Deployment Pattern

```text
┌─────────────────────────────────────────────────┐
│  Slack Workspace                                │
│                                                 │
│  #intercom-ops channel                          │
│    ├── 🟢 Intercom MCP  (thread: session abc…) │
│    ├── 🟢 Intercom MCP  (thread: session def…) │
│    ├── 🔵 Intercom ACP  (thread: session 123…) │
│    └── 🔵 Intercom ACP  (thread: session 456…) │
└─────────────────────────────────────────────────┘
        ▲                          ▲
        │ Socket Mode              │ Socket Mode
        │                          │
┌───────┴────────┐        ┌───────┴────────┐
│ agent-intercom │        │ agent-intercom │
│ --mode mcp     │        │ --mode acp     │
│ Bot: MCP App   │        │ Bot: ACP App   │
│ SLACK_BOT_TOKEN│        │ SLACK_BOT_     │
│ (shared name)  │        │ TOKEN_ACP      │
│ Port: 3000     │        │ Port: 3001     │
│ DB: intercom   │        │ DB: intercom   │
│   -mcp.db      │        │   -acp.db      │
└────────────────┘        └────────────────┘
```

Both servers may share the same workspace root directories and policy files
(`.agentrc/settings.json`). They use separate SQLite databases to avoid
write contention.

### Example: Running Both Servers

```powershell
# Terminal 1 — MCP server (uses default/shared env vars)
$env:SLACK_BOT_TOKEN  = "xoxb-mcp-bot-token"
$env:SLACK_APP_TOKEN  = "xapp-mcp-app-token"
$env:SLACK_MEMBER_IDS = "U0123456789"
agent-intercom --mode mcp --config config-mcp.toml

# Terminal 2 — ACP server (ACP-suffixed env vars take priority)
$env:SLACK_BOT_TOKEN_ACP  = "xoxb-acp-bot-token"
$env:SLACK_APP_TOKEN_ACP  = "xapp-acp-app-token"
$env:SLACK_MEMBER_IDS_ACP = "U0123456789"
agent-intercom --mode acp --config config-acp.toml
```

