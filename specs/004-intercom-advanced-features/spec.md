# Feature Specification: Intercom Advanced Features

**Feature Branch**: `004-intercom-advanced-features`
**Created**: 2026-02-26
**Status**: Draft
**Input**: Backlog feature group 004 — operator steering queue, task inbox, server reliability, Slack modal capture, SSE cleanup, policy hot-reload, audit logging, config docs, context detail levels, agent failure reporting, auto-approve suggestions, heartbeat pattern, regex pre-compilation, ping fallback, and Slack queue drain race fix.


## Clarifications

### Session 2026-02-26

- Q: Should task inbox items be workspace-agnostic or scoped to the originating channel? → A: Channel-scoped — inbox items delivered only to sessions connected via the same channel, consistent with steering queue routing.
- Q: Should the steering queue have a maximum depth cap? → A: No cap — messages accumulate without limit, governed only by the existing retention policy.
- Q: What format and rotation strategy should audit logs use? → A: JSON Lines (`.jsonl`) with daily file rotation (one file per day).
- Q: Should context detail levels apply uniformly to all Slack message types? → A: Selective — detail levels apply to status/informational messages only; approval requests, error notifications, and failure reports always show full detail regardless of configured level.
- Q: Should the system have a fallback if the Slack modal trigger_id expires before the modal opens? → A: No fallback — prioritize fast modal open within Slack's 3-second window; operator re-presses button on failure.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Operator Steering Queue (Priority: P1)

An operator wants to send instructions to a running agent session without waiting for the agent to ask. Today, the only way to communicate with an agent is reactively — the agent must first call a blocking tool (like `standby`) before the operator can respond. The operator steering queue introduces proactive communication: the operator types a message in Slack (or via a local CLI command), and that message is stored in a persistent queue. The next time the agent calls `ping` (heartbeat), any pending steering messages are returned in the response and marked as consumed. This enables real-time course corrections without the agent needing to pause or block.

**Why this priority**: This is the most impactful new capability in the feature set. It transforms the operator-agent relationship from purely reactive to proactive, enabling real-time guidance without workflow interruption. It is also a prerequisite for the heartbeat loop pattern (User Story 12) and complements the task inbox (User Story 3).

**Independent Test**: Can be fully tested by sending a Slack message to the agent channel, then calling `ping` and verifying the message appears in the response.

**Acceptance Scenarios**:

1. **Given** an active agent session, **When** the operator sends a free-text message via Slack (app mention or `/intercom steer <text>`), **Then** the message is stored in the steering queue associated with that session.
2. **Given** one or more unconsumed steering messages in the queue, **When** the agent calls `ping`, **Then** the response includes all pending messages in a `pending_steering` array and those messages are marked as consumed.
3. **Given** no unconsumed steering messages, **When** the agent calls `ping`, **Then** the `pending_steering` field is an empty array (or absent).
4. **Given** a local operator at a terminal, **When** they run `intercom-ctl steer "refocus on error handling"`, **Then** the message is stored in the steering queue for the active session.
5. **Given** multiple active sessions, **When** the operator steers via Slack in a specific channel, **Then** the message is routed to the session associated with that channel.

---

### User Story 2 - Server Startup Reliability (Priority: P1)

When the server's network port is already in use (e.g., another instance is running), the server currently continues running in a degraded state — the Slack connection succeeds but no agent can connect via the network transport. The operator sees no clear error and ends up with a zombie process. The server should detect the port conflict at startup and exit immediately with a clear error message. Additionally, only one instance of the server should be allowed to run at a time on the same machine.

**Why this priority**: A zombie server silently blocks all agent communication and wastes operator time debugging. This is a reliability and safety issue that affects every user.

**Independent Test**: Can be tested by starting two server instances and verifying the second one exits with a clear error message.

**Acceptance Scenarios**:

1. **Given** the configured network port is available, **When** the server starts, **Then** it binds successfully and begins accepting connections.
2. **Given** the configured network port is already in use, **When** the server starts, **Then** it logs a clear error message indicating the port conflict and exits immediately.
3. **Given** another server instance is already running, **When** a second instance is launched, **Then** it detects the existing instance and exits with a message recommending the operator stop the first instance.
4. **Given** the network transport fails to bind, **When** any other service (e.g., Slack) has already started, **Then** all started services are shut down cleanly before the process exits.

---

### User Story 3 - Task Inbox for Cold-Start Queuing (Priority: P2)

An operator wants to queue work items for an agent that hasn't started yet. Today, if no agent session is active, the operator has no way to leave instructions that will be picked up when the next session begins. The task inbox provides a persistent queue where work items accumulate (via Slack or local CLI) and are delivered to the agent at session startup.

**Why this priority**: Complements the steering queue by covering the cold-start scenario. Enables asynchronous workflows where the operator queues tasks during off-hours and the agent picks them up when launched.

**Independent Test**: Can be tested by queuing a task via `/intercom task <text>`, then starting an agent session and verifying the task appears at startup.

**Acceptance Scenarios**:

1. **Given** no active agent session, **When** the operator runs `/intercom task "review PR #42"` in Slack, **Then** the task is stored in the persistent inbox.
2. **Given** no active agent session, **When** the operator runs `intercom-ctl task "fix lint warnings"` locally, **Then** the task is stored in the persistent inbox.
3. **Given** one or more unconsumed inbox items, **When** a new agent session starts, **Then** the items are delivered to the agent (either in the session initialization response or via a dedicated tool call) and marked as consumed.
4. **Given** an empty inbox, **When** a new agent session starts, **Then** no inbox items are delivered and the session proceeds normally.
5. **Given** multiple tasks queued over time, **When** they are delivered at session start, **Then** they are ordered chronologically (oldest first).

---

### User Story 4 - Slack Modal Instruction Capture (Priority: P2)

When an operator presses "Resume with Instructions" or "Refine" on a Slack message, the system should collect their actual typed instructions via a Slack modal dialog. Today, these actions resolve with a placeholder string `"(instruction via Slack)"` instead of real operator input, making the instruction flow non-functional.

**Why this priority**: This is a broken user flow that undermines the core value of operator-agent collaboration. Without real instruction capture, the `standby` and `transmit` tools cannot deliver operator guidance to agents.

**Independent Test**: Can be tested by pressing "Resume with Instructions" in Slack, typing text in the modal, submitting, and verifying the agent receives the actual typed text.

**Acceptance Scenarios**:

1. **Given** an agent is waiting (via `standby` or `transmit`), **When** the operator presses "Resume with Instructions", **Then** a Slack modal dialog opens with a text input field.
2. **Given** the modal is open, **When** the operator types instructions and submits, **Then** the agent receives the exact typed text (not a placeholder).
3. **Given** the modal is open, **When** the operator dismisses it without submitting, **Then** the agent remains in its waiting state (no resolution occurs).
4. **Given** an agent called `transmit` with a proposed change, **When** the operator presses "Refine" and submits feedback via the modal, **Then** the agent receives the refinement instructions as the rejection reason.
5. **Given** the operator presses "Resume with Instructions", **When** the Slack modal fails to open (e.g., `trigger_id` expired), **Then** the agent remains in its waiting state and the operator can re-press the button to retry.

---

### User Story 5 - SSE Disconnect Session Cleanup (Priority: P2)

When an agent's network connection drops (e.g., IDE restart, network hiccup, window reload), the corresponding server-side session should be marked as terminated or interrupted. Today, disconnected sessions remain marked as "Active" indefinitely, which can cause ambiguity when the same agent reconnects or when the operator inspects session status.

**Why this priority**: Stale active sessions cause confusion in multi-session scenarios and can interfere with features like the steering queue (which routes messages to active sessions) and the ping fallback (which picks the most recent session).

**Independent Test**: Can be tested by connecting an agent, forcefully closing the connection, and verifying the session status changes within a reasonable time.

**Acceptance Scenarios**:

1. **Given** an active agent session connected via the network transport, **When** the connection drops, **Then** the server detects the disconnection and marks the session as terminated or interrupted.
2. **Given** a disconnected session, **When** the operator views session status, **Then** the session shows as terminated (not active).
3. **Given** a session that was interrupted by a transient network hiccup, **When** the agent reconnects, **Then** a new session is created (the old one remains terminated).

---

### User Story 6 - Policy Hot-Reload Wiring (Priority: P2)

Workspace auto-approve policies (`.intercom/settings.json`) should take effect immediately when modified without requiring a server restart. The policy file watcher and caching infrastructure already exist, but the cached policy is not yet wired into the main application state — the `auto_check` tool still loads the policy file from disk on every call.

**Why this priority**: Eliminates a server restart cycle when operators adjust auto-approve rules, making policy management seamless. The infrastructure is mostly built — this completes the wiring.

**Independent Test**: Can be tested by modifying the workspace policy file while the server is running and verifying that the next `auto_check` call reflects the change without a restart.

**Acceptance Scenarios**:

1. **Given** the server is running with a loaded workspace policy, **When** the operator modifies `.intercom/settings.json`, **Then** the policy watcher detects the change and updates the in-memory cache.
2. **Given** the policy cache has been updated, **When** an agent calls `auto_check`, **Then** the response reflects the new policy rules.
3. **Given** the policy file is deleted, **When** the watcher detects the deletion, **Then** the system falls back to default (deny-all) behavior.
4. **Given** the policy file contains invalid content, **When** the watcher detects the change, **Then** the system retains the last valid policy and logs a warning.

---

### User Story 7 - Audit Logging (Priority: P3)

All agent interactions — tool calls, approval decisions, session lifecycle events, and command approvals/rejections — should be recorded in structured audit log entries. This provides operators with a persistent, queryable record of what happened during each session for debugging, compliance, and post-incident review.

**Why this priority**: Operational visibility is essential for trust. Without audit logs, operators cannot reconstruct what an agent did or why a decision was made.

**Independent Test**: Can be tested by running an agent session with tool calls and approvals, then inspecting the audit log directory for structured entries covering all events.

**Acceptance Scenarios**:

1. **Given** an agent calls any tool, **When** the call completes, **Then** a structured log entry is written to the audit log with timestamp, session ID, tool name, parameters, and result.
2. **Given** the operator approves or rejects a file change, **When** the decision is recorded, **Then** the audit log captures the decision, operator identity, request ID, and reason (if provided).
3. **Given** the operator approves or rejects a terminal command, **When** the decision is recorded, **Then** the audit log captures the command, decision, operator identity, and timestamp.
4. **Given** a session starts or terminates, **When** the lifecycle event occurs, **Then** the audit log captures the event type, session ID, and timestamp.
5. **Given** the audit log directory does not exist, **When** the server starts, **Then** the directory is created automatically.

---

### User Story 8 - Agent Failure Reporting (Priority: P3)

When an agent session hangs or fails unexpectedly (e.g., no heartbeat for an extended period, process crash, or unrecoverable error), the operator should be notified via Slack with details about the failure and recommended next steps for recovery.

**Why this priority**: Operators need to know when something goes wrong without constantly monitoring the system. Proactive failure notification reduces mean time to recovery.

**Independent Test**: Can be tested by simulating a stalled agent (no heartbeat) and verifying a Slack notification is sent with failure details and recovery suggestions.

**Acceptance Scenarios**:

1. **Given** an active agent session, **When** no heartbeat is received within the configured stall detection threshold, **Then** the operator receives a Slack notification with the session ID, last known state, and suggested recovery actions.
2. **Given** an agent process crashes, **When** the server detects the process exit, **Then** the operator receives a Slack notification with the exit code and session details.
3. **Given** a failure notification has been sent, **When** the operator views it, **Then** it includes actionable next steps (e.g., "run `intercom-ctl spawn` to restart" or "check logs at ...").

---

### User Story 9 - Configuration Documentation (Priority: P3)

The README should provide a comprehensive breakdown of all configuration options in `config.toml`, including each setting's purpose, valid values, defaults, and examples. Currently, the documentation shows a basic example but doesn't explain what each option does. Additionally, default values should reflect the primary use case (e.g., `host_cli` defaults to "copilot", `host_cli_args` defaults to `["--sse"]`).

**Why this priority**: New users struggle to configure the system without comprehensive documentation. Good defaults reduce setup friction.

**Independent Test**: Can be tested by having a new user follow the README to configure the system and verifying they can set up all options without external guidance.

**Acceptance Scenarios**:

1. **Given** a new user reading the README, **When** they look for configuration guidance, **Then** they find a section with every `config.toml` option documented with purpose, valid values, and examples.
2. **Given** a user creates a new `config.toml` from the example, **When** they use the defaults, **Then** the server starts in the most common configuration (network transport mode with the primary CLI tool).
3. **Given** the `config.toml.example` file, **When** compared with the documentation, **Then** every option in the example is covered in the docs and vice versa.

---

### User Story 10 - Context Detail Levels for Slack (Priority: P3)

Operators should be able to configure how much context the server shares in Slack messages — from minimal (short status updates) to verbose (full diffs, tool parameters, session details). Different operators and teams have different preferences for signal-to-noise ratio in their Slack channels.

**Why this priority**: Reduces Slack notification fatigue for operators who want less detail, while preserving full context for those who need it.

**Independent Test**: Can be tested by setting different detail levels and verifying that Slack messages contain the expected amount of information for each level.

**Acceptance Scenarios**:

1. **Given** the detail level is set to "minimal", **When** the server posts to Slack, **Then** messages contain only essential information (status, outcome, errors).
2. **Given** the detail level is set to "standard" (default), **When** the server posts to Slack, **Then** messages include status, context summaries, and key parameters.
3. **Given** the detail level is set to "verbose", **When** the server posts to Slack, **Then** messages include full details (diffs, parameters, session metadata).
4. **Given** the operator changes the detail level in configuration, **When** new messages are posted, **Then** they reflect the updated detail level.

---

### User Story 11 - Auto-Approve Suggestion for Commands (Priority: P3)

After an operator manually approves a terminal command, the system should offer to add a matching pattern to the workspace's auto-approve policy so the same command is automatically approved in future sessions. This reduces repetitive approval prompts for commands the operator considers safe.

**Why this priority**: Reduces friction in the approval workflow. Operators who repeatedly approve the same commands benefit from a self-learning policy that adapts to their preferences.

**Independent Test**: Can be tested by approving a command, accepting the suggestion, and verifying the command is auto-approved on the next invocation.

**Acceptance Scenarios**:

1. **Given** the operator approves a terminal command that is not currently auto-approved, **When** the approval is recorded, **Then** the operator is presented with an option (e.g., a Slack button) to add the command pattern to the workspace policy.
2. **Given** the operator accepts the auto-approve suggestion, **When** the pattern is saved, **Then** it is written to `.intercom/settings.json` as an efficient regex pattern.
3. **Given** the operator declines the suggestion, **When** the same command is submitted again, **Then** it still requires manual approval.
4. **Given** similar commands are approved multiple times, **When** the system generates a regex pattern, **Then** the pattern is generalized appropriately (e.g., `cargo test` with varying arguments becomes a single regex).

---

### User Story 12 - Agent Heartbeat Loop Pattern (Priority: P3)

A documented and reusable agent-side pattern that keeps a session alive and responsive without human interaction. The loop: call `ping` → process any steering messages → call `standby` with a timeout → on timeout, loop again; on instruction, act then loop. This is a documentation and prompt template deliverable, not a server-side feature.

**Why this priority**: Codifies the recommended keep-alive pattern so operators can easily instruct agents to enter autonomous monitoring mode. Depends on the steering queue (User Story 1) for useful message delivery.

**Independent Test**: Can be tested by loading the prompt template into an agent and verifying it enters the ping/standby loop, processes steering messages, and remains responsive.

**Acceptance Scenarios**:

1. **Given** the heartbeat loop prompt template exists, **When** an agent loads it, **Then** it enters a cycle of ping → process messages → standby → repeat.
2. **Given** the agent is in heartbeat loop mode, **When** a steering message arrives, **Then** the agent processes it during the next ping cycle.
3. **Given** the agent is in heartbeat loop mode, **When** the standby timeout elapses, **Then** the agent loops back to ping (no hang or exit).

---

### User Story 13 - Policy Regex Pre-Compilation (Priority: P4)

Auto-approve command patterns should be compiled once at policy load time rather than recompiled from scratch on every `auto_check` call. This improves response time for the auto-approve evaluation, especially for workspaces with many command patterns.

**Why this priority**: Performance optimization. While not user-facing, it reduces latency for every tool call that checks auto-approve policy and prevents redundant computation.

**Independent Test**: Can be tested by loading a policy with many regex patterns and measuring that `auto_check` response time does not scale with pattern count on repeated calls.

**Acceptance Scenarios**:

1. **Given** a workspace policy with command patterns, **When** the policy is loaded, **Then** all regex patterns are compiled once into an efficient match structure.
2. **Given** the pre-compiled patterns are available, **When** `auto_check` evaluates a command, **Then** it uses the pre-compiled patterns (no per-call compilation).
3. **Given** the policy file changes, **When** the hot-reload triggers, **Then** patterns are recompiled for the new policy.
4. **Given** a policy contains an invalid regex pattern, **When** compilation is attempted, **Then** the invalid pattern is skipped with a warning and all valid patterns remain functional.

---

### User Story 14 - Ping Fallback to Most Recent Session (Priority: P4)

When `ping` finds multiple active sessions (e.g., a stale session that wasn't cleaned up plus a newly spawned session), it should fall back to the session with the most recent activity timestamp instead of returning an ambiguity error. This makes `ping` more resilient to edge cases.

**Why this priority**: Low-priority resilience improvement. The SSE disconnect cleanup (User Story 5) should eliminate most stale sessions, but this provides a safety net.

**Independent Test**: Can be tested by creating two active sessions with different timestamps, calling `ping`, and verifying it returns the most recently active session.

**Acceptance Scenarios**:

1. **Given** multiple active sessions exist, **When** the agent calls `ping`, **Then** the response corresponds to the session with the most recent activity.
2. **Given** one active session and one stale session, **When** the agent calls `ping`, **Then** the stale session is ignored in favor of the active one.
3. **Given** exactly one active session, **When** the agent calls `ping`, **Then** behavior is unchanged from current implementation.

---

### User Story 15 - Slack Queue Drain Race Fix (Priority: P4)

During graceful shutdown, when no global Slack channel is configured (all channels are per-workspace via connection parameters), the Slack message queue drain step is skipped. If any messages are enqueued between the shutdown signal and the queue worker abort, those messages are dropped. The drain should be unconditional.

**Why this priority**: Low-priority edge case. In practice, this race is unlikely because per-session messages require a live connection that stops accepting work before the drain would run. However, the fix is straightforward and improves shutdown correctness.

**Independent Test**: Can be tested by configuring the server without a global channel, enqueuing a message during shutdown, and verifying it is delivered before the process exits.

**Acceptance Scenarios**:

1. **Given** the server is shutting down with no global channel configured, **When** messages exist in the Slack queue, **Then** the queue is drained before the worker is terminated.
2. **Given** the server is shutting down with a global channel configured, **When** messages exist in the queue, **Then** behavior is unchanged (queue drains as it does today).
3. **Given** the queue is empty at shutdown, **When** the drain runs, **Then** shutdown completes without unnecessary delay.

---

### User Story 16 - Approval File Attachment (Priority: P2)

When an agent submits a code proposal via `check_clearance`, the operator sees a diff (inline for small changes, or as a Slack file snippet for large changes). However, the operator never sees the **original file content** that the diff applies to. Without surrounding context, operators cannot verify that the proposed changes make sense relative to the current file state — especially for targeted edits in large files. The system should attach the original file content alongside the diff when posting approval requests to Slack, giving operators full context for informed decision-making.

**Why this priority**: This directly undermines the core review and approval workflow. Operators are asked to approve changes they cannot fully evaluate. The `upload_file()` infrastructure already exists, making this a targeted enhancement to `ask_approval.rs` (the `check_clearance` handler).

**Independent Test**: Can be tested by calling `check_clearance` with a diff for an existing file and verifying that both the diff and the original file content appear as Slack file attachments in the channel.

**Acceptance Scenarios**:

1. **Given** an agent calls `check_clearance` for a modification to an existing file, **When** the approval message is posted to Slack, **Then** the original file content is uploaded as a Slack file attachment alongside the diff.
2. **Given** an agent calls `check_clearance` for a new file (no original), **When** the approval message is posted, **Then** only the diff/content is shown (no original file attachment).
3. **Given** the original file is very large (>100KB), **When** the approval message is posted, **Then** the file is uploaded as a Slack file attachment (never inlined in the message body).
4. **Given** the original file cannot be read (e.g., deleted between proposal creation and Slack post), **When** the approval message is posted, **Then** a warning is shown in the message but the approval flow continues without blocking.
5. **Given** the file path points to a binary file, **When** the approval message is posted, **Then** the file is uploaded as-is with an appropriate filename; no attempt to render it inline.

---

### Edge Cases

- What happens when the steering queue receives a message for a session that terminates before the next `ping`? The message should remain unconsumed and be available if the session is recovered, or purged by retention rules.
- What happens when multiple operators steer the same session simultaneously? Messages should be queued in arrival order — no deduplication or conflict resolution.
- What happens when the task inbox has items but the new session connects via a different channel? Inbox items are channel-scoped — they are delivered only to sessions connected via the same channel where the task was submitted. Items for other channels remain unconsumed until a matching session starts.
- What happens when a Slack modal times out (Slack enforces a 30-minute view lifetime)? The agent should remain in its waiting state; the operator can re-press the button to open a new modal.
- What happens when the audit log directory runs out of disk space? The server should log a warning and continue operating — audit logging failure should not crash the server.
- What happens when a policy regex pattern is catastrophically slow (ReDoS)? The pre-compiled regex should enforce a complexity or time limit during compilation.
- What happens when the server starts but the configured port is blocked by a firewall (no bind error, just no traffic)? This is outside scope — the server can only detect bind failures, not firewall rules.

## Requirements *(mandatory)*

### Functional Requirements

**Operator Steering Queue**

- **FR-001**: System MUST provide a persistent message queue for operator-to-agent steering messages, associated with a specific session.
- **FR-002**: System MUST accept steering messages from Slack (app mentions and a `/intercom steer <text>` slash command).
- **FR-003**: System MUST accept steering messages from the local CLI companion (`intercom-ctl steer "<text>"`).
- **FR-004**: System MUST deliver all unconsumed steering messages for a session in the `ping` (heartbeat) response.
- **FR-005**: System MUST mark steering messages as consumed once delivered via `ping`.
- **FR-006**: System MUST route Slack-originated steering messages to the session associated with the originating channel.

**Server Startup Reliability**

- **FR-007**: System MUST exit immediately with a clear error message if the network transport port fails to bind at startup.
- **FR-008**: System MUST prevent multiple server instances from running simultaneously on the same machine.
- **FR-009**: System MUST shut down all started services cleanly if any critical startup step fails.

**Task Inbox**

- **FR-010**: System MUST provide a persistent inbox queue for work items submitted when no agent session is active.
- **FR-011**: System MUST accept inbox items from Slack (a `/intercom task <text>` slash command).
- **FR-012**: System MUST accept inbox items from the local CLI companion (`intercom-ctl task "<text>"`).
- **FR-013**: System MUST deliver all unconsumed inbox items scoped to the session's channel at session startup, ordered chronologically.
- **FR-014**: System MUST mark inbox items as consumed once delivered.

**Slack Modal Instruction Capture**

- **FR-015**: System MUST open a Slack modal with a text input field when the operator presses "Resume with Instructions" or "Refine".
- **FR-016**: System MUST deliver the operator's typed modal text to the waiting agent (replacing the current placeholder string behavior).
- **FR-017**: System MUST leave the agent in its waiting state if the operator dismisses the modal without submitting.

**SSE Disconnect Session Cleanup**

- **FR-018**: System MUST detect when a network transport connection drops and mark the corresponding session as terminated or interrupted.
- **FR-019**: System MUST update the session status within a reasonable time after disconnection (not indefinitely active).

**Policy Hot-Reload Wiring**

- **FR-020**: System MUST wire the existing policy cache into the main application state so that `auto_check` reads from the cache rather than loading from disk.
- **FR-021**: System MUST reflect policy file changes in `auto_check` responses without requiring a server restart.
- **FR-022**: System MUST retain the last valid policy if a modified policy file contains invalid content.

**Audit Logging**

- **FR-023**: System MUST write structured audit log entries for all tool calls, including parameters and results.
- **FR-024**: System MUST write structured audit log entries for all approval and rejection decisions, including operator identity and reason.
- **FR-025**: System MUST write structured audit log entries for session lifecycle events (start, terminate, interrupt).
- **FR-026**: System MUST write structured audit log entries for terminal command approvals and rejections.
- **FR-027**: System MUST create the audit log directory automatically if it does not exist.
- **FR-028a**: System MUST write audit log entries in JSON Lines format (one JSON object per line, `.jsonl` extension).
- **FR-028b**: System MUST rotate audit log files daily (one file per calendar day).

**Agent Failure Reporting**

- **FR-028**: System MUST send a Slack notification to the operator when a session is detected as stalled (no heartbeat within the threshold).
- **FR-029**: System MUST send a Slack notification to the operator when an agent process exits unexpectedly.
- **FR-030**: System MUST include recommended recovery actions in failure notifications.

**Configuration Documentation**

- **FR-031**: The README MUST document every `config.toml` option with its purpose, valid values, default, and an example.
- **FR-032**: The `config.toml.example` MUST use defaults that reflect the primary use case (network transport mode with the default CLI tool).

**Context Detail Levels**

- **FR-033**: System MUST support configurable detail levels for Slack messages (minimal, standard, verbose).
- **FR-034**: System MUST default to "standard" detail level if not configured.
- **FR-035**: System MUST apply the configured detail level to status and informational Slack messages only. Approval requests, error notifications, and failure reports MUST always include full detail regardless of the configured level.

**Auto-Approve Suggestion**

- **FR-036**: System MUST offer to add a command pattern to the workspace auto-approve policy after an operator manually approves a command.
- **FR-037**: System MUST generate an efficient regex pattern when the operator accepts the suggestion.
- **FR-038**: System MUST write accepted patterns to the workspace policy file (`.intercom/settings.json`).

**Heartbeat Loop Pattern**

- **FR-039**: A reusable prompt template MUST be provided that instructs agents to enter a ping/standby keep-alive loop.
- **FR-040**: The pattern documentation MUST describe the interaction with the steering queue for message processing between iterations.

**Policy Regex Pre-Compilation**

- **FR-041**: System MUST compile all command regex patterns once at policy load time rather than on each `auto_check` call.
- **FR-042**: System MUST recompile patterns when the policy is reloaded (via hot-reload or restart).
- **FR-043**: System MUST skip invalid regex patterns with a warning rather than failing the entire policy load.

**Ping Fallback**

- **FR-044**: System MUST fall back to the session with the most recent activity timestamp when `ping` finds multiple active sessions, rather than returning an ambiguity error.

**Slack Queue Drain**

- **FR-045**: System MUST drain the Slack message queue unconditionally during graceful shutdown, regardless of whether a global channel is configured.

**Approval File Attachment**

- **FR-046**: System MUST upload the original file content as a Slack file attachment when posting approval requests for modifications to existing files.
- **FR-047**: System MUST NOT attempt to attach an original file when the proposal is for a new file (no pre-existing content).
- **FR-048**: System MUST handle unreadable or missing original files gracefully, posting a warning in the Slack message without blocking the approval flow.
- **FR-049**: System MUST upload large files (>100KB) exclusively as Slack file attachments, never inline in the message body.

### Key Entities

- **Steering Message**: A text message from an operator to an active agent session. Attributes: unique ID, session association, message text, source (Slack or IPC), creation timestamp, consumed flag. No queue depth cap; governed by retention policy.
- **Task Inbox Item**: A work item queued for the next agent session startup, scoped to the originating channel. Attributes: unique ID, channel association, message text, source (Slack or IPC), creation timestamp, consumed flag.
- **Audit Log Entry**: A structured record of an agent interaction event, stored in JSON Lines format with daily file rotation. Attributes: timestamp, session ID, event type (tool call, approval, rejection, lifecycle), event details, operator identity (when applicable).
- **Compiled Policy**: A workspace policy with pre-compiled regex patterns for command matching. Attributes: raw policy data, compiled pattern set.

## Assumptions

- The steering queue and task inbox share the same retention policy as other session data (purged after the configured retention period).
- Audit log entries are written to the filesystem in JSON Lines format (``.jsonl``) with daily file rotation — not the database — for simplicity and to avoid database size growth concerns.
- The heartbeat loop pattern is an agent-side convention documented as a prompt template — no new server-side tools are required beyond the existing `ping` and `standby`.
- The Slack modal instruction capture uses Slack's standard `views.open` API with `trigger_id` from the button interaction payload.
- SSE disconnect detection relies on the transport layer's stream lifecycle events — no polling-based heartbeat is needed from the server to the client.
- The single-instance check uses the network port bind as the detection mechanism (if the port is occupied, another instance is running).
- Context detail levels are configured per-server (in ``config.toml``), not per-workspace or per-session. Detail levels apply only to status/informational messages; approval requests, error notifications, and failure reports always display full detail.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Operators can send steering messages to running agents and see them delivered on the next heartbeat, with end-to-end delivery time under 5 seconds from message send to `ping` response inclusion.
- **SC-002**: Starting a second server instance on the same machine results in an immediate, clear exit with an informative error — no zombie processes.
- **SC-003**: 100% of `standby` and `transmit` instruction interactions deliver the operator's actual typed text to the agent (zero placeholder strings).
- **SC-004**: Disconnected sessions are marked as terminated within 30 seconds of connection loss.
- **SC-005**: Policy file changes take effect in `auto_check` within 5 seconds of file save, without a server restart.
- **SC-006**: Every tool call, approval decision, and session lifecycle event has a corresponding audit log entry with full structured details.
- **SC-007**: Agent failures are reported to Slack within the stall detection threshold plus 10 seconds.
- **SC-008**: A new user can configure the system using only the README documentation, without consulting source code or external resources.
- **SC-009**: The `auto_check` response time does not increase linearly with the number of command regex patterns (pre-compilation eliminates per-call overhead).
- **SC-010**: Task inbox items queued while no session is active are delivered to the next starting session with 100% reliability.
- **SC-011**: Every `check_clearance` approval request for an existing file includes the original file content as a Slack attachment, enabling operators to review changes in full context.
