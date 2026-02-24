# Data Model: 003-agent-intercom-release

**Branch**: `003-agent-intercom-release` | **Date**: 2026-02-23

## Overview

This feature introduces **no new database tables, domain entities, or schema migrations**. All changes are to naming conventions, notification behavior, transport infrastructure, and build/release tooling.

## Existing Entities (Unchanged)

The following entities are part of the existing data model and are not modified by this feature:

| Entity | Table | Impact |
|---|---|---|
| Session | `sessions` | No changes |
| ApprovalRequest | `approval_requests` | No changes |
| Prompt | `prompts` | No changes |
| Checkpoint | `checkpoints` | No changes |
| StallEvent | `stall_events` | No changes |

## Name-Affected Constants

The following constants change as part of the rename but do not affect stored data:

| Constant | Old Value | New Value | Location |
|---|---|---|---|
| `KEYCHAIN_SERVICE` | `"monocoque-agent-rc"` | `"agent-intercom"` | `src/config.rs` |
| `POLICY_DIR` | `".agentrc"` | `".intercom"` | `src/policy/watcher.rs` |
| `POLICY_PATH` | `".agentrc/settings.json"` | `".intercom/settings.json"` | `src/policy/loader.rs` |
| IPC pipe name | `"monocoque-agent-rc"` | `"agent-intercom"` | `src/ipc/socket.rs`, `ctl/main.rs` |
| Slash command root | `"/monocoque"` | `"/intercom"` | `src/slack/commands.rs` |
| Env var prefix | `MONOCOQUE_` | `INTERCOM_` | `src/config.rs`, `src/orchestrator/spawner.rs` |

## Tool Name Mapping

| Old Tool Name | New Tool Name |
|---|---|
| `ask_approval` | `check_clearance` |
| `accept_diff` | `check_diff` |
| `check_auto_approve` | `auto_check` |
| `forward_prompt` | `transmit` |
| `wait_for_instruction` | `standby` |
| `heartbeat` | `signal` |
| `remote_log` | `broadcast` |
| `recover_state` | `reboot` |
| `set_operational_mode` | `switch_freq` |
