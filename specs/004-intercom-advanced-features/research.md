# Research: Intercom Advanced Features

**Feature**: 004-intercom-advanced-features
**Date**: 2026-02-26

## 1. Steering Queue — Message Delivery via `ping`

**Decision**: Extend the `ping` (heartbeat) response to include a `pending_steering` array of unconsumed messages. Messages are fetched from `steering_message` DB table, returned in the response, then marked consumed — all in a single handler call.

**Rationale**: The `ping` tool is already called frequently (heartbeat pattern). Piggybacking steering delivery avoids a new blocking tool and keeps the agent in control of when it reads messages. Zero additional round-trips.

**Alternatives considered**:
- New `check_steering` tool: Adds a tool to the surface, increases agent complexity. Rejected — `ping` already serves as the natural wakeup point.
- Push via MCP notification: MCP notifications are fire-and-forget (no response). Agents may miss them. Rejected — delivery guarantee matters.

## 2. Steering Queue — Database Schema

**Decision**: New `steering_message` table with columns: `id TEXT PK`, `session_id TEXT NOT NULL`, `channel_id TEXT`, `message TEXT NOT NULL`, `source TEXT NOT NULL CHECK(source IN ('slack','ipc'))`, `created_at TEXT NOT NULL`, `consumed INTEGER NOT NULL DEFAULT 0`. Index on `(session_id, consumed)`.

**Rationale**: Follows existing schema patterns (TEXT PKs, TEXT timestamps, CHECK constraints). Channel-scoping via `channel_id` enables routing. `consumed` as INTEGER (0/1) for SQLite boolean idiom.

**Alternatives considered**:
- Separate `consumed_at` timestamp instead of boolean: Adds complexity for marginal benefit. The retention purge already handles cleanup by age. Rejected.

## 3. Task Inbox — Cold-Start Delivery

**Decision**: Deliver inbox items in the `reboot` (recover_state) tool response. The `reboot` tool is already called at session startup to check for interrupted sessions. Add a `pending_tasks` array to its response.

**Rationale**: `reboot` is the canonical session-initialization tool. Agents call it on startup. Adding inbox delivery here requires no new tools and follows the same pattern as steering via `ping`.

**Alternatives considered**:
- New `check_inbox` tool: Adds tool surface area. Rejected — `reboot` already runs at cold start.
- Delivery via `on_initialized` server hook: rmcp `on_initialized` doesn't support returning data to the agent. Rejected.

## 4. Task Inbox — Database Schema

**Decision**: New `task_inbox` table: `id TEXT PK`, `channel_id TEXT`, `message TEXT NOT NULL`, `source TEXT NOT NULL CHECK(source IN ('slack','ipc'))`, `created_at TEXT NOT NULL`, `consumed INTEGER NOT NULL DEFAULT 0`. Index on `(channel_id, consumed)`.

**Rationale**: Channel-scoped per clarification Q1. No `session_id` column because inbox items exist before any session starts. Items matched to sessions at delivery time via the session's channel_id.

## 5. Server Startup — Single Instance Enforcement

**Decision**: Use the HTTP port bind as the single-instance check. If `axum::serve()` fails to bind, log a clear error and call `std::process::exit(1)`. The port bind is the first critical step — if it fails, no MCP communication is possible.

**Rationale**: Port binding is a natural mutex. No additional lock files, PIDs, or named mutexes needed. Aligns with constitution Principle VI (simplicity).

**Alternatives considered**:
- PID file in a known location: Stale PIDs can cause false positives after crashes. Requires cleanup logic. Rejected.
- Named mutex (Windows) / flock (Unix): Platform-specific, adds complexity. Port bind is already platform-agnostic. Rejected.

## 6. Slack Modal Instruction Capture

**Decision**: 3-step flow: (1) Button handler extracts `trigger_id` from `BlockActions` payload, calls `views.open` with a plain-text input modal; stores session context in `private_metadata`. (2) New `ViewSubmission` match arm in `events.rs` extracts typed text and session context. (3) Modal submit handler resolves the pending oneshot channel with real text.

**Rationale**: This is the canonical Slack modal pattern. `trigger_id` threading is the only way to open modals from button actions. `private_metadata` is the standard way to pass context between button press and modal submission.

**Alternatives considered**:
- Collect text in-thread via message: Poor UX (no modal), hard to distinguish instruction text from general chat. Rejected.
- Ephemeral message with text input: Slack doesn't support text inputs in ephemeral messages. Rejected.

## 7. SSE Disconnect Detection

**Decision**: Hook into the axum response stream lifecycle. When the SSE/Streamable HTTP stream closes (detected via `Drop` on the response body or stream completion), trigger `session_repo.set_terminated()` for the associated session.

**Rationale**: The transport layer already knows when a connection drops. No polling needed. The rmcp `StreamableHttpService` manages session state internally — we need to hook the outer axum layer.

**Alternatives considered**:
- Server-to-client heartbeat polling: Adds traffic and complexity. The transport already detects closure. Rejected.
- Rely on stall detection: Too slow (configured threshold may be minutes). Sessions should be cleaned up promptly. Rejected.

## 8. Policy Hot-Reload Wiring

**Decision**: Add `PolicyCache` (from existing `policy/watcher.rs`) to `AppState`. Change `auto_check` tool handler to read from `PolicyCache::get_policy()` instead of `PolicyLoader::load()`. The watcher's `notify` subscription triggers cache updates.

**Rationale**: The infrastructure already exists (`PolicyWatcher::register()`, `PolicyCache::get_policy()`). The remaining work is purely wiring — adding the cache to `AppState` and updating call sites.

**Alternatives considered**:
- Reload from disk on every call with file mtime check: Current behavior. Unnecessary I/O. The watcher approach is already built. Rejected.

## 9. Audit Logging — JSONL with Daily Rotation

**Decision**: New `src/audit/` module with `AuditLogger` trait and `JsonlAuditWriter` implementation. Writes to `.intercom/logs/audit-YYYY-MM-DD.jsonl`. Each line is a self-contained JSON object with `timestamp`, `session_id`, `event_type`, `event_data`, and optional `operator_id`. File rotation is date-based: a new file is opened when the date changes.

**Rationale**: JSONL is machine-parseable, human-readable, and aligns with the IPC protocol's JSON-line convention. Daily rotation keeps files manageable without external tooling. Filesystem-based (not DB) per the spec assumption to avoid SQLite growth.

**Alternatives considered**:
- Write to SQLite: Increases DB size, complicates retention. Rejected per spec assumption.
- tracing-subscriber JSON output: Already exists for operational logs. Audit logs need a separate, persistent, queryable format. Rejected.

## 10. Policy Regex Pre-Compilation

**Decision**: Option A from backlog — eager compile in loader. `PolicyLoader::load()` returns a `CompiledWorkspacePolicy` containing both the raw `WorkspacePolicy` and a `regex::RegexSet`. `PolicyEvaluator::check()` takes `&CompiledWorkspacePolicy`. The `match_command_pattern` function is replaced with a `RegexSet::matches()` call.

**Rationale**: `RegexSet` compiles all patterns into a single DFA, enabling one-pass matching. Currently `match_command_pattern` (line 172-189 of `src/policy/evaluator.rs`) calls `Regex::new(pattern)` for every pattern on every check. Pre-compilation eliminates this O(n) compilation cost.

**Alternatives considered**:
- `OnceLock` on `WorkspacePolicy`: Adds interior mutability to a data struct. Less clean than Option A. Rejected.

## 11. Context Detail Levels

**Decision**: Add `slack_detail_level` field to `GlobalConfig` (values: `minimal`, `standard`, `verbose`; default: `standard`). Pass to `SlackService`. In `blocks.rs`, message builders check the level and include/omit fields accordingly. Exception: approval messages, error notifications, and failure reports always use `verbose` regardless of setting.

**Rationale**: Per clarification Q4, selective application prevents detail level from interfering with safety-critical workflows (operator must always see full diff for approvals).

## 12. Ping Fallback to Most-Recent Session

**Decision**: In `heartbeat.rs`, when `list_active` returns multiple sessions, sort by `updated_at DESC` and pick the first instead of returning an ambiguity error.

**Rationale**: Simple, defensive change. The SSE disconnect cleanup (research item 7) should prevent most stale sessions, but this provides a safety net.

## 13. Slack Queue Drain Race

**Decision**: Move the 500ms queue drain sleep and abort into `shutdown_with_timeout` unconditionally, after all task handles are awaited. Remove the conditional check on `config.slack.channel_id`.

**Rationale**: Straightforward fix. The drain should always run regardless of channel configuration.

## 14. Approval File Attachment

**Decision**: Upload the original file content as a Slack file attachment alongside the diff in `ask_approval.rs` (the `check_clearance` handler). After computing the `original_hash` and before posting the approval message, read the original file content and call `upload_file()` — the same 3-step external upload flow already used for large diff snippets. The original file is always uploaded as a file attachment (never inlined), named `{file_path}.original` for disambiguation from the diff snippet.

**Rationale**: The operator needs to see the surrounding context of the file being changed to make an informed approval decision. Currently, only the diff is shown — for targeted edits in large files, the diff alone is insufficient. The `upload_file()` infrastructure already exists and is battle-tested for diff snippet uploads. Uploading as a file attachment avoids Slack's 3000-character block text limit and handles arbitrarily large files. New files (no original on disk) skip the upload — there is no original to review.

**Alternatives considered**:
- Inline original content in message blocks: Slack blocks have a 3000-character text limit. Files routinely exceed this. Rejected.
- Upload only a context window (e.g., ±50 lines around changes): Adds complexity to determine context boundaries, and operators may need to see the full file for structural understanding. Rejected — upload the full file; Slack handles large file rendering well.
- Modify `accept_diff.rs` too: `accept_diff` runs *after* approval — the operator has already decided. No review context is needed at that stage. Rejected — the fix belongs exclusively in `ask_approval.rs`.