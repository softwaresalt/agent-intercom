---
title: backlogit command map
description: Agent-readable backlogit metadata and command reference
---

## Workspace

* Storage root: `.backlogit`
* Queue path: `.backlogit\queue`
* Archive path: `.backlogit\archive`
* Logs path: `.backlogit\logs`
* Stash path: `.backlogit\stash.jsonl`

## Artifact Types

### `deliberation`

A collaborative deliberation artifact linked to a stashed idea or issue

* Prefix: `DL`
* Hierarchy level: `1`
* ID format: `{NNN}{suffix}`
* Fields:
  * `priority` (enum, required) values: `low`, `medium`, `high`, `critical` default: `medium`
  * `status` (enum, required) values: `queued`, `active`, `blocked`, `review`, `done`, `accepted`, `rejected`, `archived` default: `queued`
* Template sections:
  * `problem-frame` (required): The operator and agent's shared understanding of the problem
  * `options` (optional): Approaches or alternatives considered during deliberation
  * `chosen-direction` (optional): Selected direction and decision rationale
  * `open-questions` (optional): Questions or risks that remain unresolved
  * `notes` (optional): Supporting research, references, or follow-up notes

### `feature`

A feature-level work item

* Prefix: `F`
* Hierarchy level: `1`
* ID format: `{NNN}{suffix}`
* Allowed children: `task`, `review`
* Fields:
  * `harness_status` (enum, optional) values: `pending`, `scaffolded`, `passing`, `failing` default: `pending`
  * `status` (enum, required) values: `queued`, `active`, `blocked`, `review`, `done`, `accepted`, `rejected`, `archived` default: `queued`
* Template sections:
  * `description` (required): Detailed description of the feature
  * `goals` (optional): Goals and intended outcomes
  * `dod` (optional): Definition of Done for this feature

### `shipment`

A shipment artifact representing a branch and pull request scope

* Prefix: `S`
* Hierarchy level: `1`
* ID format: `{NNN}{suffix}`
* Fields:
  * `branch` (string, optional)
  * `items` (list, optional)
  * `status` (enum, required) values: `queued`, `active`, `shipped`, `abandoned` default: `queued`
* Template sections:
  * `description` (required): Detailed description of the shipment scope
  * `items` (optional): Work items included in this shipment
  * `blocked-returns` (optional): Items removed from the shipment and returned to backlog

### `review`

A review artifact tied to a feature branch lifecycle

* Prefix: `R`
* Hierarchy level: `2`
* ID format: `{NNN}{suffix}`
* Fields:
  * `source_branch` (string, optional)
  * `status` (enum, required) values: `queued`, `active`, `blocked`, `review`, `done`, `accepted`, `rejected`, `archived` default: `queued`
* Template sections:
  * `summary` (required): High-level review outcome and scope
  * `findings` (optional): Findings, recommendations, and reviewer notes
  * `decisions` (optional): Disposition of findings and next actions

### `task`

A discrete unit of work

* Prefix: `T`
* Hierarchy level: `2`
* ID format: `{NNN}{suffix}`
* Allowed children: `subtask`, `bug`
* Fields:
  * `priority` (enum, required) values: `low`, `medium`, `high`, `critical` default: `medium`
  * `status` (enum, required) values: `queued`, `active`, `blocked`, `review`, `done`, `accepted`, `rejected`, `archived` default: `queued`
* Template sections:
  * `description` (required): Detailed description of the work item
  * `acceptance-criteria` (optional): Conditions that must be met for completion
  * `implementation-notes` (optional): Technical notes and implementation details

### `bug`

* Prefix: `B`
* Hierarchy level: `3`
* ID format: `{NNN}{suffix}`
* Fields:
  * `severity` (enum, required) values: `low`, `medium`, `high`, `critical` default: `medium`
  * `status` (enum, required) values: `queued`, `active`, `blocked`, `review`, `done`, `accepted`, `rejected`, `archived` default: `queued`

### `subtask`

A discrete unit of work

* Prefix: `ST`
* Hierarchy level: `3`
* ID format: `{NNN}{suffix}`
* Fields:
  * `status` (enum, required) values: `queued`, `active`, `blocked`, `review`, `done`, `accepted`, `rejected`, `archived` default: `queued`
* Template sections:
  * `description` (required): Detailed description of the discrete work item
  * `implementation-notes` (optional): Technical notes and implementation details


## Stash

* Path: `.backlogit\stash.jsonl`
* Supported kinds: `feature`, `task`, `bug`, `epic`, `unknown`, `spike`, `subtask`, `deliberation`, `review`, `shipment`

* Supported priorities: `low`, `medium`, `high`, `critical`

* Default priority: `medium`

* Deliberation type: `deliberation`

## CLI Commands

### `backlogit`

Backlogit — AI-native agile workspace

```text
backlogit init
  backlogit add --type feature --title "Authentication hardening"
  backlogit list --status active
  backlogit get 001-F --format json
  backlogit queue view --group-by status
  backlogit stash add "Defer audit dashboard split" --kind feature
  backlogit migrate --source .\.backlog --adapter backlog-md --dry-run
  backlogit mcp
```

### `backlogit add`

Create a new artifact

```text
backlogit add --type feature --title "Authentication hardening"
  backlogit add --type task --title "Add token rotation" --parent 001-F
  backlogit add --type subtask --title "Write expiry tests" --parent 001.001-T --section description="Cover refresh and expiry flows"
```

### `backlogit adopt`

Adopt an orphaned item under a new parent feature

```text
backlogit adopt 015.009-T --parent 016-F
```

### `backlogit archive`

Archive a completed artifact

```text
backlogit archive 001.001-T
  backlogit archive --all-done
```

### `backlogit checkpoint`

Manage session state checkpoints

### `backlogit checkpoint cleanup`

Archive resolved and stale checkpoints

```text
backlogit checkpoint cleanup --retention-days 7
```

### `backlogit checkpoint get`

Get and validate a specific checkpoint

```text
backlogit checkpoint get checkpoint-20260423-100000.json
```

### `backlogit checkpoint list`

List session state checkpoints

```text
backlogit checkpoint list --agent ship --status active
```

### `backlogit checkpoint resolve`

Mark a checkpoint as resolved

```text
backlogit checkpoint resolve checkpoint-20260423-100000.json
```

### `backlogit delete`

Delete an artifact

```text
backlogit delete 001.001-T --force
```

### `backlogit deliberate`

Create a deliberation artifact linked to a stash entry

```text
backlogit deliberate ABCD1234 --title "Audit dashboard split follow-up"
  backlogit deliberate ABCD1234 --options "- Keep the current feature set narrow\n- Pull the work into the next feature wave"
  backlogit deliberate ABCD1234 --chosen-direction "Split the backlog work and defer reporting polish"
```

### `backlogit dep`

Manage artifact dependencies

### `backlogit dep add`

Add a dependency edge

```text
backlogit dep add 001.002-T 001.001-T
  backlogit dep add 010-T 002-F --type blocks
```

### `backlogit dep list`

List dependencies for an artifact

```text
backlogit dep list 001.002-T
  backlogit dep list 001.001-T --reverse
```

### `backlogit dep remove`

Remove a dependency edge

```text
backlogit dep remove 001.002-T 001.001-T
```

### `backlogit docs`

Lint and migrate documentation frontmatter (docline base schema)

```text
backlogit docs lint
  backlogit docs lint --profile ingestion --format json
  backlogit docs migrate
  backlogit docs migrate --apply --yes --path docs/decisions
  backlogit docs scope
  backlogit docs classify docs/decisions/x.md
```

### `backlogit docs classify`

Print the derived doc_type for a repo-relative path

### `backlogit docs lint`

Validate in-scope documentation frontmatter

### `backlogit docs migrate`

Plan (default) or apply an idempotent frontmatter migration

### `backlogit docs scope`

Print the active docline scope, profiles, and taxonomy

### `backlogit doctor`

Check workspace integrity

```text
backlogit doctor
  backlogit doctor --check-orphans=false
  backlogit doctor --fix-orphans
  backlogit doctor --fix-archived-from
  backlogit doctor --format json
```

### `backlogit get`

Retrieve an artifact by ID

```text
backlogit get 001-F
  backlogit get 001-F --format json
  backlogit get 001-F --section description
```

### `backlogit init`

Initialize a new backlogit workspace

```text
backlogit init
  backlogit init D:\Source\MyProject
```

### `backlogit list`

List artifacts in the workspace

```text
backlogit list
  backlogit list --status active --type task
  backlogit list --group-by status
  backlogit list --json
```

### `backlogit manifest`

Print a JSON-RPC manifest of all backlogit MCP tool definitions

```text
backlogit manifest
  backlogit manifest | jq '.tools[].name'
  backlogit --jsonrpc manifest
```

### `backlogit mcp`

Start the backlogit MCP stdio server

```text
backlogit mcp
  backlogit --cwd D:\Source\MyProject mcp
```

### `backlogit metadata`

Discover backlogit metadata for agents and tooling

### `backlogit metadata catalog`

Print the unified metadata catalog

```text
backlogit metadata catalog
  backlogit metadata catalog --json
```

### `backlogit metadata export-command-map`

Write an agent-readable command map into the workspace

```text
backlogit metadata export-command-map .github\instructions\backlogit-command-map.md
  backlogit metadata export-command-map .github\instructions\backlogit-command-map.json --format json
```

### `backlogit migrate`

Migrate backlog data between supported formats and layouts

```text
backlogit migrate --source .\.backlog --adapter backlog-md --dry-run
  backlogit migrate --source .\.backlog --adapter backlog-md --validate
  backlogit migrate --source .\.backlog --adapter backlog-md
  backlogit migrate --dry-run
  backlogit migrate --rollback
```

### `backlogit move`

Change artifact status

```text
backlogit move 001.001-T --status review
  backlogit move 001-F --status done
```

### `backlogit query`

Execute a read-only SQL query against the index

```text
backlogit query "SELECT id, title, status FROM items ORDER BY updated_at DESC LIMIT 20"
  backlogit query "SELECT stash_id, kind, state FROM stash_entries ORDER BY updated_at DESC"
```

### `backlogit queue`

Manage the work queue

### `backlogit queue bulk-status`

Update status for multiple items

```text
backlogit queue bulk-status --ids 001.001-T,001.002-T,001.003-T --status active
```

### `backlogit queue move`

Reorder an item in the queue

```text
backlogit queue move 001.001-T --position 1
```

### `backlogit queue view`

View queue items

```text
backlogit queue view
  backlogit queue view --status active --group-by type
  backlogit queue view --sort priority
```

### `backlogit search`

Full-text search across artifacts

```text
backlogit search authentication
  backlogit search "token rotation" --limit 10
```

### `backlogit shipment`

Manage shipment work groups

### `backlogit shipment claim`

Claim a queued shipment

```text
backlogit shipment claim 001-S
```

### `backlogit shipment create`

Create a shipment

```text
backlogit shipment create --title "Sprint 1" --items 001-F,001.001-T
```

### `backlogit shipment get`

Get a shipment by ID

```text
backlogit shipment get 001-S
```

### `backlogit shipment list`

List shipments

```text
backlogit shipment list --status active
```

### `backlogit shipment return-blocked`

Return a blocked item from a shipment

```text
backlogit shipment return-blocked --shipment 001-S --item 001.001-T --reason "blocked"
```

### `backlogit shipment ship`

Close a released shipment and archive the released scope

```text
backlogit shipment ship 001-S --sha deadbeef --message "merge: release" --author "dev@example.com"
```

### `backlogit stash`

Manage the deferred work stash

### `backlogit stash add`

Add an item to the stash

```text
backlogit stash add "Investigate tenant-specific rate limits" --kind feature --priority high
  backlogit stash add "Document migration edge cases" --kind task
```

### `backlogit stash archive`

Archive an active stash entry

```text
backlogit stash archive ABCD1234
  backlogit stash remove ABCD1234
```

### `backlogit stash edit`

Edit a stash entry's text, kind, or priority

```text
backlogit stash edit ABCD1234 --kind feature
  backlogit stash edit ABCD1234 --priority high
  backlogit stash edit ABCD1234 --text "Updated description"
```

### `backlogit stash get`

Get a stash entry by ID

```text
backlogit stash get ABCD1234
```

### `backlogit stash harvest`

Harvest a stash item into a planned work item

```text
backlogit stash harvest ABCD1234 --type feature
  backlogit stash harvest ABCD1234 --type task --parent-id 001-F --status active
  backlogit stash harvest --priority critical --type feature
```

### `backlogit stash list`

List the current active stash entries

```text
backlogit stash list
  backlogit stash list --priority high
  backlogit stash list --kind feature
  backlogit stash list --group-by-priority
```

### `backlogit status`

Show workspace artifact summary

```text
backlogit status
  backlogit --cwd D:\Source\MyProject status
```

### `backlogit sync`

Rehydrate the SQLite index from Markdown source files

```text
backlogit sync
  backlogit --cwd D:\Source\MyProject sync
```

### `backlogit telemetry`

Inspect Copilot CLI token usage and tool telemetry

### `backlogit telemetry branch`

Show per-branch telemetry metrics with type classification and enrichment

### `backlogit telemetry harvest`

Parse Copilot CLI logs and write telemetry-sessions.jsonl

### `backlogit telemetry list`

List harvested session summaries

### `backlogit telemetry report`

Generate a formatted telemetry report from harvested data

### `backlogit telemetry schema`

Show telemetry JSONL and SQL table schemas

### `backlogit telemetry top`

Show top N servers by token usage

### `backlogit telemetry trend`

Show token usage trends grouped by date, branch, or model class

### `backlogit update`

Update artifact fields or sections

```text
backlogit update 001.001-T --status review
  backlogit update 001.001-T --priority high
  backlogit update 001-F --section goals="Ship passwordless sign-in"
  backlogit update 001-F --harness-status passing
```

### `backlogit version`

Print version, commit, build date, and Go runtime information

```text
backlogit version
  backlogit version --format json
```

## MCP Tools

* `backlogit_ack_hook_events`: Acknowledge processing of hook events up to and including seq
* `backlogit_add_dependency`: Add a dependency between two artifacts with cycle detection
* `backlogit_add_link`: Add a directed semantic link between two artifacts
* `backlogit_add_to_shipment`: Add an item to a shipment
* `backlogit_adopt_item`: Adopt an orphaned item under a new parent feature
* `backlogit_append_comment`: Append a comment event to the item's JSONL log
* `backlogit_archive_item`: Archive a completed artifact to the archive directory
* `backlogit_claim_shipment`: Move a queued shipment to active
* `backlogit_cleanup_checkpoints`: Archive resolved and stale checkpoints based on retention policy
* `backlogit_create_checkpoint`: Save a session state checkpoint
* `backlogit_create_item`: Create a new backlogit artifact
* `backlogit_create_shipment`: Create a new shipment artifact
* `backlogit_delete_item`: Delete an artifact by ID
* `backlogit_deliberate`: Create a deliberation artifact linked to an active stash entry
* `backlogit_docs_lint`: Validate in-scope documentation frontmatter against the docline base schema. Returns a success envelope {valid, violation_count, findings} even when violations exist.
* `backlogit_docs_migrate`: Plan (default) or apply an idempotent, body-preserving frontmatter migration. apply=true is gated server-side and requires an explicit scoped path.
* `backlogit_docs_scope`: Return the active docline scope globs, taxonomy, path map, and validation profiles.
* `backlogit_doctor`: Scan the workspace for structural integrity issues such as orphaned artifacts and duplicate IDs. Use fix_orphans=true to archive orphaned artifacts automatically. Returns a DoctorReport with findings, fix_actions, and checked_at timestamp.
* `backlogit_export_command_map`: Write an agent-readable command map file into the .backlogit/ workspace directory
* `backlogit_fetch_stash`: Fetch the current active stash entries from .backlogit/stash.jsonl
* `backlogit_get_checkpoint`: Get and validate a specific checkpoint by filename
* `backlogit_get_dependencies`: Get dependency graph for an artifact including upstream and downstream edges
* `backlogit_get_item`: Get a backlogit item by ID
* `backlogit_get_links`: Get all outgoing semantic links from an artifact
* `backlogit_get_metadata_catalog`: Get a unified workspace metadata catalog for agent discovery
* `backlogit_get_queue`: Get prioritized work queue items respecting dependency constraints
* `backlogit_get_shipment`: Get a shipment by ID
* `backlogit_get_version`: Return backlogit version, commit SHA, build date, and Go runtime version
* `backlogit_get_wit_metadata`: Get complete WIT metadata for an artifact type including fields, sections, and relationships
* `backlogit_harvest_stash`: Harvest a stash entry or all stash entries at a priority into backlogit work items
* `backlogit_list_checkpoints`: List session state checkpoints with optional filters
* `backlogit_list_items`: List artifacts with optional filters
* `backlogit_list_shipments`: List shipments with an optional status filter
* `backlogit_list_templates`: List registered template types and their section definitions
* `backlogit_list_types`: List all configured WIT types with hierarchy levels and descriptions
* `backlogit_log_telemetry`: Write agent telemetry to telemetry.jsonl
* `backlogit_merge_sync`: Perform an incremental sync of the .backlogit workspace cache. Computes a diff against the in-memory manifest and applies targeted upserts/deletes. Falls back to full rehydration when the delta exceeds the threshold.
* `backlogit_move_item`: Change an artifact's status
* `backlogit_poll_hook_events`: Poll for unacknowledged hook events since the consumer's last checkpoint
* `backlogit_query_sql`: Execute a read-only SQL query against the backlogit index
* `backlogit_remove_dependency`: Remove a dependency between two artifacts
* `backlogit_remove_link`: Remove a directed semantic link between two artifacts
* `backlogit_resolve_checkpoint`: Mark a checkpoint as resolved
* `backlogit_return_blocked`: Return a blocked item from a shipment
* `backlogit_save_memory`: Save a key-value pair to agent memories
* `backlogit_search_items`: Full-text search across artifact titles and descriptions
* `backlogit_ship_shipment`: Close a released shipment, archive the released scope, and record merge commit traceability
* `backlogit_stash`: Add a deferred work item to the stash
* `backlogit_stash_archive`: Archive an active stash entry
* `backlogit_stash_edit`: Edit a stash entry's text, kind, or priority
* `backlogit_stash_get`: Get a single stash entry by ID
* `backlogit_stash_remove`: [Deprecated: use backlogit_stash_archive] Remove an active stash entry
* `backlogit_sync_index`: Rehydrate the SQLite index from Markdown source files
* `backlogit_telemetry_harvest`: Parse Copilot CLI logs, correlate token usage by session and tool, write telemetry-sessions.jsonl, and rehydrate telemetry tables
* `backlogit_track_commit`: Associate a git commit SHA with an artifact for traceability
* `backlogit_update_item`: Update an existing backlogit artifact
