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

## Consequences

### Positive

- **No code changes required**: the existing `--mode` flag and single-driver
  architecture work as-is.
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
│ Port: 3000     │        │ Port: 3001     │
│ DB: intercom   │        │ DB: intercom   │
│   -mcp.db      │        │   -acp.db      │
└────────────────┘        └────────────────┘
```

Both servers may share the same workspace root directories and policy files
(`.agentrc/settings.json`). They use separate SQLite databases to avoid
write contention.
