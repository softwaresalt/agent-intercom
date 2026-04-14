---
description: "Backlog tool integration instructions — teaches agents how to interact with the installed backlog management tool using abstracted operations"
applyTo: '**'
---

# Backlog Integration Instructions

This workspace uses **backlogit** for structured backlog management. All agents MUST use the backlog tool for task tracking rather than creating ad-hoc markdown files or static task lists.

## Tool Configuration

| Setting | Value |
|---------|-------|
| Tool | backlogit |
| Directory | `.backlogit/` |
| Access | both |
| Registry | `.autoharness/backlog-registry.yaml` |

## Operation Reference

Use these operations for all backlog interactions. The operation names are abstract — the actual tool names and parameters are mapped through the backlog registry.

### Core Operations (All Tools)

| Operation | MCP Tool | CLI Command | Purpose |
|-----------|----------|-------------|---------|
| Create task | `backlogit_create_item` | `backlogit add --type {{artifact_type}} --title {{title}}` | Create a new task/artifact |
| List tasks | `backlogit_list_items` | `backlogit list` | List tasks with filters |
| Get task | `backlogit_get_item` | `backlogit get {{id}}` | Retrieve task details |
| Update task | `backlogit_update_item` | `backlogit update {{id}}` | Modify task fields |
| Move task | `backlogit_move_item` | `backlogit move {{id}} --status {{status}}` | Change task status |
| Search | `backlogit_search_items` | `backlogit search {{query}}` | Full-text search |
| Complete | `backlogit_move_item` | `backlogit move {{id}} --status done` | Mark task complete |

### Status Values

| Abstract Status | Tool-Specific Value |
|----------------|---------------------|
| Queued | `queued` |
| Active | `active` |
| Done | `done` |
| Blocked | `blocked` |

### Extended Operations (Tool-Dependent)

| Operation | MCP Tool | CLI Command |
|---|---|---|
| Query (SQL) | `backlogit_query_sql` | `backlogit query "SELECT ..."` |
| Sync Index | `backlogit_sync_index` | `backlogit sync` |
| Append Comment | `backlogit_append_comment` | `backlogit comment <id> "text"` |
| Log Telemetry | `backlogit_log_telemetry` | N/A |
| Save Memory | `backlogit_save_memory` | N/A |
| Create Checkpoint | `backlogit_create_checkpoint` | N/A |
| Get Queue | `backlogit_get_queue` | `backlogit queue` |
| Add Dependency | `backlogit_add_dependency` | `backlogit dep add <id> <dep_id>` |
| Remove Dependency | `backlogit_remove_dependency` | `backlogit dep rm <id> <dep_id>` |
| Get Dependencies | `backlogit_get_dependencies` | `backlogit dep get <id>` |
| Track Commit | `backlogit_track_commit` | `backlogit track <id> <sha>` |
| Create Shipment | `backlogit_create_shipment` | `backlogit ship create "title"` |
| List Shipments | `backlogit_list_shipments` | `backlogit ship list` |
| Get Shipment | `backlogit_get_shipment` | `backlogit ship get <id>` |
| Add to Shipment | `backlogit_add_to_shipment` | `backlogit ship add <ship_id> <item_id>` |
| Claim Shipment | `backlogit_claim_shipment` | `backlogit ship claim <id>` |
| Ship Shipment | `backlogit_ship_shipment` | `backlogit ship close <id>` |
| Stash | `backlogit_stash` | `backlogit stash "text"` |
| Fetch Stash | `backlogit_fetch_stash` | `backlogit stash list` |
| Harvest Stash | `backlogit_harvest_stash` | `backlogit stash harvest <id>` |
| Deliberate | `backlogit_deliberate` | N/A |
| Add Link | `backlogit_add_link` | `backlogit link add <src> <tgt> <type>` |
| Remove Link | `backlogit_remove_link` | `backlogit link rm <src> <tgt> <type>` |
| Get Links | `backlogit_get_links` | `backlogit link get <id>` |
| Archive Item | `backlogit_archive_item` | `backlogit archive <id>` |
| Doctor | `backlogit_doctor` | `backlogit doctor` |

## Agent Workflow Patterns

### Creating a Task

```text
Call backlogit_create_item with:
  title: "Task title"
  artifact_type: "task"
  status: "queued"
  description: "Task description"
  parent_id: "parent-task-id"  (if applicable)
  labels: "label1,label2"      (if applicable)
```

### Claiming a Task (Status → Active)

```text
Call backlogit_move_item with:
  id: "task-id"
  status: "active"
```

### Completing a Task

```text
Call backlogit_move_item with:
  id: "task-id"
```

### Listing Ready Tasks

```text
Call backlogit_list_items with:
  status: "queued"
```

### Adding a Label

```text
Call backlogit_update_item with:
  id: "task-id"
  labels: "existing-label,harness-ready"
```

## Advanced Patterns When Supported

If the registry advertises advanced features, prefer them over ad hoc workarounds:

* **Token-efficient lookup** — use the query operation when `features.sql_query` is true
* **Ready-work selection** — use queue-aware operations when `features.queue` is true
* **Dependency reasoning** — use dependency operations when `features.dependencies` is true
* **Agent continuity** — use memory and checkpoint operations when `features.memory` or `features.checkpoints` are true
* **Traceability** — use comment or commit-tracking operations when `features.comments` or `features.commit_tracking` are true
* **Index freshness** — use sync / rehydration operations when the workspace was edited outside normal mutation tools

If a tool-specific overlay instruction file is installed (for example,
`.github/instructions/backlogit.instructions.md`), follow it in addition to this generic guide.

## Rules

1. **Always use the backlog tool** for task management. Do not create markdown task files outside the `.backlogit/` directory.
2. **Use abstract status values** mapped through the registry, not hardcoded strings.
3. **Check the registry** (`.autoharness/backlog-registry.yaml`) for the exact field names and operation parameters when unsure.
4. **Prefer MCP tools** over CLI when both are available — MCP returns structured JSON, CLI returns human-readable text.
5. **Feature gating**: Before calling an extended operation, verify the feature is supported by checking the `features` section in the registry.

Generated by autoharness | Template: backlog-integration.instructions.md.tmpl
