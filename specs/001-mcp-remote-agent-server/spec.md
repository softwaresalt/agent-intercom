# Feature Specification: MCP Remote Agent Server

**Feature Branch**: `001-mcp-remote-agent-server`
**Created**: 2026-02-08
**Status**: Draft
**Input**: User description: "Build an MCP server that provides remote I/O capabilities to local AI agents via Slack, enabling asynchronous code review, approval workflows, session orchestration, and continuation prompt forwarding from a mobile device"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Remote Code Review and Approval (Priority: P1)

A developer runs an AI coding agent on their workstation at home. While away from their desk (commuting, at a cafe, in a meeting), the agent generates code changes and needs approval before writing files. The developer receives a Slack notification on their mobile phone showing the proposed diff, reviews it, and taps "Accept" or "Reject" without ever touching the workstation keyboard.

**Why this priority**: This is the core value proposition. Without remote approval, the agent blocks indefinitely whenever it needs human confirmation, making unattended operation impossible. Every other feature builds on this foundation.

**Independent Test**: Start the server, configure it with a Slack workspace, connect an AI agent, have the agent invoke the approval tool with a sample diff, and verify the diff appears in Slack with actionable buttons. Tap "Accept" and confirm the agent receives the approval response and proceeds.

**Acceptance Scenarios**:

1. **Given** the server is running and connected to Slack, **When** the agent submits a small code change (fewer than 20 lines) for approval, **Then** the diff is rendered inline in the Slack message with "Accept" and "Reject" buttons
2. **Given** the server is running and connected to Slack, **When** the agent submits a large code change (20 lines or more) for approval, **Then** the diff is uploaded as a collapsible Slack snippet with syntax highlighting, accompanied by "Accept" and "Reject" buttons
3. **Given** a pending approval request exists in Slack, **When** the operator taps "Accept", **Then** the server returns an "approved" status to the agent within 5 seconds
4. **Given** a pending approval request exists in Slack, **When** the operator taps "Reject", **Then** the server returns a "rejected" status with an optional rejection reason to the agent
5. **Given** a pending approval request exists, **When** no response is received within the configured timeout period, **Then** the server returns a "timeout" status and notifies the Slack channel

---

### User Story 2 - Programmatic Diff Application (Priority: P1)

After the remote operator approves a code proposal, the server applies the approved changes directly to the file system on behalf of the agent. This eliminates the need for anyone to manually click "Keep" or "Accept" in the IDE's diff viewer, enabling fully hands-free file writing from the remote Slack interface.

**Why this priority**: Without programmatic application, approved changes sit in limbo requiring local UI interaction, which negates the entire remote orchestration model. This completes the end-to-end remote workflow initiated by Story 1.

**Independent Test**: Submit a diff for approval, approve it via Slack, then invoke the diff application tool with the approval ID. Verify the file is written to disk with correct content and the Slack channel receives a confirmation message.

**Acceptance Scenarios**:

1. **Given** an approved proposal with a full file payload, **When** the agent requests diff application, **Then** the server writes the file to the target path (creating directories as needed) and returns a success response with the file path and byte count
2. **Given** an approved proposal with a unified diff, **When** the agent requests diff application, **Then** the server applies the patch to the existing file and returns a success response
3. **Given** an approved proposal, **When** the agent requests application but the local file has changed since the proposal was created, **Then** the server returns a conflict error (unless force mode is enabled)
4. **Given** an approved proposal that has already been applied, **When** the agent requests application again, **Then** the server returns an "already consumed" error without modifying the file system

---

### User Story 3 - Remote Status Logging (Priority: P2)

The agent sends progress updates ("Running tests...", "Build completed", "Deploying to staging") to the Slack channel in real time. These messages do not block the agent and keep the remote operator informed of what the agent is doing without requiring them to check the local terminal.

**Why this priority**: Visibility into agent activity is essential for trust and situational awareness. Without status logging, the operator has no way to know whether the agent is active, stuck, or progressing, leading to anxiety and unnecessary interventions.

**Independent Test**: Have the agent invoke the logging tool with messages at different severity levels (info, success, warning, error) and verify each message appears in the Slack channel with correct formatting and visual indicators.

**Acceptance Scenarios**:

1. **Given** the server is connected to Slack, **When** the agent sends an informational log message, **Then** the message appears in the Slack channel as plain text without blocking the agent
2. **Given** the server is connected to Slack, **When** the agent sends messages with different severity levels, **Then** each message is visually differentiated (success with checkmark, warning with caution icon, error with error icon)
3. **Given** a Slack thread timestamp, **When** the agent sends a log message referencing that thread, **Then** the message appears as a reply within the specified thread

---

### User Story 4 - Agent Stall Detection and Remote Nudge (Priority: P1)

An AI agent is working through a multi-step todo list (e.g., "implement auth module, write tests, update docs"). Midway through, the agent silently stalls — it stops producing output, stops making tool calls, and does not emit any continuation prompt or error. It just freezes. The server detects this silence after a configurable inactivity threshold, alerts the remote operator via Slack with context about what the agent was last doing, and provides a "Nudge" button. The operator taps "Nudge" and the server injects a continuation prompt into the agent's input stream, waking it up to resume work.

**Why this priority**: Silent stalls are the most insidious failure mode in unattended agent operation. Unlike continuation prompts (which the agent actively emits), stalls produce *no signal at all*. Without a watchdog, the operator has no way to know the agent has stopped, and work sits incomplete indefinitely. This is a P1 because it directly undermines the core promise of remote unattended operation — if the agent can stall silently, the operator must constantly check on it, defeating the purpose of the system.

**Independent Test**: Connect an agent to the server, have the agent make several tool calls, then simulate the agent going silent (no tool calls for the configured threshold). Verify the server posts a stall alert to Slack within the threshold window. Tap "Nudge" and verify the server injects a continuation message that the agent receives.

**Acceptance Scenarios**:

1. **Given** an active session where the agent has been making tool calls, **When** the agent stops making any tool calls or producing output for longer than the configured inactivity threshold, **Then** the server posts a stall alert to Slack with the agent's last known activity (last tool called, elapsed idle time, and the session's original prompt)
2. **Given** a stall alert in Slack, **When** the operator taps "Nudge", **Then** the server injects a configurable continuation message (e.g., "Continue working on the current task. Pick up where you left off.") into the agent's input and the agent resumes execution
3. **Given** a stall alert in Slack, **When** the operator taps "Nudge with Instructions", **Then** a dialog opens for the operator to type a custom nudge message, and the server injects that message into the agent's input
4. **Given** a stall alert in Slack, **When** the operator taps "Stop", **Then** the session is terminated and the operator receives confirmation
5. **Given** a stall alert has been posted, **When** the agent resumes activity on its own before the operator responds, **Then** the server updates the Slack alert to indicate the agent self-recovered and disables the action buttons
6. **Given** the inactivity threshold has elapsed and the operator does not respond to the stall alert, **When** a second configurable escalation threshold elapses, **Then** the server auto-nudges the agent with the default continuation message and posts a notification to Slack
7. **Given** the server has auto-nudged the agent, **When** the agent still does not resume within a configurable max-retries count, **Then** the server posts an escalated alert (with @channel mention) indicating the agent appears unresponsive and may require manual intervention
8. **Given** the operator has configured stall detection to be disabled for a session, **When** the agent goes idle, **Then** no stall alert is posted

---

### User Story 5 - Continuation Prompt Forwarding (Priority: P2)

AI agents periodically emit meta-level prompts asking whether to continue working, especially after extended execution. These prompts block the local terminal. The server intercepts these prompts and forwards them to Slack with actionable response buttons (Continue, Refine, Stop), allowing the remote operator to keep the agent running, adjust its focus, or halt it entirely.

**Why this priority**: Without prompt forwarding, continuation prompts block the agent indefinitely, causing unattended sessions to stall. This is the second most common blocking interaction after code approval.

**Independent Test**: Have the agent invoke the prompt forwarding tool with a continuation prompt. Verify the prompt appears in Slack with three action buttons. Tap "Continue" and verify the agent receives the decision. Tap "Refine" and verify a dialog opens for providing revised instructions.

**Acceptance Scenarios**:

1. **Given** the agent has been working and emits a continuation prompt, **When** the prompt forwarding tool is invoked, **Then** the prompt text appears in Slack with "Continue", "Refine", and "Stop" buttons along with elapsed time and action count context
2. **Given** a forwarded prompt in Slack, **When** the operator selects "Continue", **Then** the server returns a "continue" decision to the agent within 5 seconds
3. **Given** a forwarded prompt in Slack, **When** the operator selects "Refine", **Then** a dialog opens for the operator to type revised instructions, and upon submission, the server returns a "refine" decision with the new instruction text
4. **Given** a forwarded prompt in Slack, **When** the operator selects "Stop", **Then** the server returns a "stop" decision and the agent halts its current task
5. **Given** a forwarded prompt with no response, **When** the configured timeout elapses, **Then** the server auto-responds with "continue" and posts a timeout notification to Slack

---

### User Story 6 - Workspace Auto-Approve Policy (Priority: P2)

A developer configures their workspace to allow certain safe operations (running tests, linting, reading files) to proceed without requiring remote approval. This reduces Slack notification noise for routine operations and speeds up the agent's workflow for pre-trusted actions.

**Why this priority**: Frequent low-risk approval requests create notification fatigue and slow down the agent unnecessarily. Auto-approve lets experienced operators pre-authorize safe operations, improving both user experience and agent throughput.

**Independent Test**: Create a workspace policy file that auto-approves "cargo test". Have the agent check the auto-approve status for that command and verify it returns "auto-approved: true". Verify that operations exceeding the risk threshold still require explicit approval.

**Acceptance Scenarios**:

1. **Given** a workspace policy file exists with "cargo test" in the auto-approve list, **When** the agent checks whether "cargo test" is auto-approved, **Then** the server returns "auto_approved: true" with the matched rule
2. **Given** a workspace policy exists, **When** the agent checks an operation that is not in the auto-approve list, **Then** the server returns "auto_approved: false"
3. **Given** a workspace policy exists, **When** the policy file is modified, **Then** the server hot-reloads the new rules without requiring a restart
4. **Given** a workspace policy auto-approves a command, **When** that command is not in the global server allowlist, **Then** the auto-approve is denied (global policy supersedes workspace policy)

---

### User Story 7 - Remote Session Orchestration (Priority: P3)

The remote operator initiates, pauses, resumes, and terminates agent sessions entirely from Slack. They can also create checkpoints to snapshot a session's state and restore a prior checkpoint if an experiment goes wrong. This transforms the operator from a passive reviewer into an active orchestrator.

**Why this priority**: Session management enables multi-task workflows and recovery from failed experiments. While the core approval workflow (P1) can function without it, session orchestration unlocks advanced use cases like context switching between tasks and rolling back mistakes.

**Independent Test**: Use the Slack slash command to start a new agent session with a prompt. Verify the agent process spawns and connects. Pause the session, verify it enters a paused state. Resume it and verify it continues. Create a checkpoint, make further changes, then restore the checkpoint and confirm the prior state is recovered.

**Acceptance Scenarios**:

1. **Given** the server is running, **When** the operator issues a "session-start" command with a prompt via Slack, **Then** a new agent process spawns on the local workstation and connects to the server, and a confirmation with the session ID appears in Slack
2. **Given** an active session, **When** the operator issues a "session-pause" command, **Then** the agent enters a wait state and no further tool calls are processed until resumed
3. **Given** a paused session, **When** the operator issues a "session-resume" command, **Then** the agent continues from where it was suspended
4. **Given** an active session, **When** the operator issues a "session-checkpoint" command with a label, **Then** the session state is snapshot and stored with the provided label, and a confirmation appears in Slack
5. **Given** a stored checkpoint, **When** the operator issues a "session-restore" command with the checkpoint ID, **Then** the server warns about file divergences (if any), and upon confirmation, restores the session to the checkpointed state
6. **Given** the concurrent session limit has been reached, **When** the operator attempts to start another session, **Then** the server returns an error indicating the limit has been exceeded

---

### User Story 8 - Remote File Browsing and Command Execution (Priority: P3)

The remote operator browses the workspace file structure and views file contents directly from Slack, without needing the agent to be running. They can also execute pre-approved shell commands (such as "git status" or "cargo test") from Slack and see the output.

**Why this priority**: File browsing and command execution give the operator situational awareness and manual control independent of the agent. This is valuable but not required for the primary approval workflow.

**Independent Test**: Issue a "list-files" command via Slack and verify a directory tree appears. Issue a "show-file" command and verify the file contents appear with syntax highlighting. Issue a custom command (e.g., "git status") and verify the output appears in Slack.

**Acceptance Scenarios**:

1. **Given** the server is running, **When** the operator issues "list-files" via Slack, **Then** a formatted directory tree of the workspace appears in the channel
2. **Given** a valid file path, **When** the operator issues "show-file" via Slack, **Then** the file contents appear with syntax highlighting appropriate to the file type
3. **Given** a command alias defined in the server configuration, **When** the operator issues that command via Slack, **Then** the corresponding shell command executes and the output is posted to Slack
4. **Given** a file path that would resolve outside the workspace root, **When** the operator issues "show-file" with that path, **Then** the server returns a permission denied error

---

### User Story 9 - State Recovery After Crash (Priority: P3)

The server persists all pending approval requests and session state to an embedded database. If the server crashes or is restarted, the agent can recover the last known state, including pending requests that were in flight, without losing work.

**Why this priority**: Crash recovery prevents data loss during long-running agent sessions. While it does not affect the happy path, it is essential for reliability in production use.

**Independent Test**: Submit an approval request, kill the server process, restart it, then invoke the state recovery tool and verify the pending request is returned with its original data.

**Acceptance Scenarios**:

1. **Given** a pending approval request was in flight when the server was terminated, **When** the server restarts and the agent invokes state recovery, **Then** the pending request is returned with its original title, type, and creation timestamp
2. **Given** no pending state exists, **When** the agent invokes state recovery, **Then** the server returns a "clean" status indicating a fresh start
3. **Given** the server is shutting down gracefully, **When** shutdown is initiated, **Then** all pending requests are marked as "interrupted" in the database and a final notification is posted to Slack

---

### User Story 10 - Operational Mode Switching (Priority: P3)

The developer switches the server between "remote", "local", and "hybrid" modes depending on their situation. When sitting at the desk, they switch to "local" mode to route approvals through a local CLI tool instead of Slack. When leaving the desk, they switch to "remote" to re-enable Slack notifications.

**Why this priority**: Mode switching provides flexibility but is an optimization over the default remote mode. Most users operate in remote or hybrid mode exclusively.

**Independent Test**: Set the mode to "local" and verify Slack notifications stop. Set it to "remote" and verify approvals flow through Slack again. Set it to "hybrid" and verify both channels are active.

**Acceptance Scenarios**:

1. **Given** the server is in "remote" mode, **When** the agent switches to "local" mode, **Then** subsequent approval requests are routed to the local IPC channel and Slack notifications are suppressed
2. **Given** the server is in any mode, **When** the mode is changed, **Then** the previous and current modes are returned, and the change is persisted across server restarts

---

### Edge Cases

* What happens when the Slack WebSocket connection drops mid-approval? The server queues the pending request in the database and re-posts it to Slack upon reconnection.
* What happens when the operator taps "Accept" twice on the same proposal? The server processes only the first interaction and ignores duplicates (the buttons are replaced with a static status indicator after the first action).
* What happens when a diff targets a file path outside the workspace root? The server rejects the operation with a path violation error.
* What happens when the agent sends a log message while the Slack API is rate-limited? Messages are queued in memory and retried with exponential backoff after the rate limit clears.
* What happens when the operator issues a command that is not in the allowlist? The server rejects it with an explicit "command not found" error. No shell execution occurs.
* What happens when the workspace policy file is malformed? The server falls back to "require approval for everything" and logs a warning to both the console and Slack.
* What happens when someone other than the authorized operator interacts with buttons or commands? The interaction is silently ignored. A security event is logged with the unauthorized user's ID and the action they attempted.
* What happens when a checkpoint restore detects workspace files that have diverged? The server warns the operator with a list of changed files and requires explicit confirmation before proceeding.
* What happens when the server receives a SIGTERM while an agent session is active? The server saves pending requests, notifies Slack, terminates spawned agent processes with a grace period, and exits cleanly.
* What happens when the agent stalls during a tool call that is handled by the host IDE (not the MCP server)? The server can only detect silence in the MCP tool call stream. If the agent is blocked on a non-MCP interaction (e.g., a local IDE confirmation dialog), the stall detector fires and the nudge is injected, but the agent may not be able to act on it until the local block is cleared. The stall alert informs the operator of this ambiguity.
* What happens when the agent stalls and the auto-nudge wakes it up, but it immediately stalls again? The server tracks consecutive nudge attempts per session. After exceeding the configurable max-retries, it escalates to the operator rather than continuing to auto-nudge in an infinite loop.
* What happens when multiple stall alerts fire in rapid succession across concurrent sessions? Each session has its own independent stall timer. Alerts are posted with the session ID prominently displayed so the operator can distinguish between them.

## Requirements *(mandatory)*

### Functional Requirements

* **FR-001**: System MUST expose an MCP-compatible server interface that AI agents (Claude Code, GitHub Copilot CLI, Cursor, VS Code) can connect to via standard transports
* **FR-002**: System MUST maintain a persistent WebSocket connection to Slack via Socket Mode, requiring no inbound firewall ports or public IP addresses
* **FR-003**: System MUST render code diffs in Slack with size-adaptive formatting: inline code blocks for small diffs and uploaded snippets with syntax highlighting for large diffs
* **FR-004**: System MUST provide interactive "Accept" and "Reject" buttons on code proposals that resolve the agent's blocked state when the operator responds
* **FR-005**: System MUST apply approved code changes directly to the local file system, supporting both full-file writes and unified diff patch application
* **FR-006**: System MUST validate that all file operations resolve within the configured workspace root directory, rejecting path traversal attempts
* **FR-007**: System MUST persist all pending approval requests and session state to an embedded database that survives process restarts
* **FR-008**: System MUST forward agent continuation prompts to Slack with "Continue", "Refine", and "Stop" action buttons and return the operator's decision to the agent
* **FR-009**: System MUST support a workspace-level auto-approve policy file that permits pre-authorized operations to bypass the remote approval gate
* **FR-010**: System MUST hot-reload the workspace policy file when it changes, without requiring a server restart
* **FR-011**: System MUST enforce that the workspace policy cannot expand permissions beyond what the global configuration allows
* **FR-012**: System MUST allow the remote operator to start, pause, resume, terminate, checkpoint, and restore agent sessions via Slack slash commands
* **FR-013**: System MUST restrict Slack interactions to authorized user IDs defined in the server configuration, silently ignoring unauthorized interactions
* **FR-014**: System MUST execute only commands explicitly listed in the server configuration allowlist when triggered remotely, rejecting all others
* **FR-015**: System MUST transmit non-blocking status log messages from the agent to the Slack channel with severity-based visual formatting
* **FR-016**: System MUST provide a local IPC channel (named pipe or Unix domain socket) for local overrides when the operator is physically present
* **FR-017**: System MUST support switching between "remote", "local", and "hybrid" operational modes at runtime
* **FR-018**: System MUST expose the Slack channel's recent chat history as an MCP resource, allowing the agent to read operator instructions from the channel
* **FR-019**: System MUST provide a comprehensive command discovery mechanism via a "help" command that lists all available commands grouped by category
* **FR-020**: System MUST handle Slack API rate limits by queuing messages and retrying with exponential backoff
* **FR-021**: System MUST shut down gracefully on process termination signals, saving state, notifying Slack, and terminating spawned agent processes
* **FR-022**: System MUST prevent double-submission of approval and prompt responses by replacing interactive buttons with static status text after the first action
* **FR-023**: System MUST enforce a configurable maximum on concurrent agent sessions to prevent resource exhaustion
* **FR-024**: System MUST verify file integrity (via content hashing) before applying diffs and before restoring checkpoints, warning the operator of divergences
* **FR-025**: System MUST track the timestamp of the most recent MCP tool call for each active session and detect when the idle interval exceeds a configurable inactivity threshold
* **FR-026**: System MUST post a stall alert to Slack when an active session's inactivity threshold is exceeded, including the last tool called, elapsed idle time, and session context
* **FR-027**: System MUST provide a "Nudge" action on stall alerts that injects a configurable continuation message into the agent's input stream to prompt it to resume work
* **FR-028**: System MUST support an auto-nudge escalation policy: after a configurable wait period without operator response, the server auto-nudges the agent and notifies Slack
* **FR-029**: System MUST cap the number of consecutive auto-nudge attempts per session and escalate to the operator with an elevated alert when the cap is exceeded
* **FR-030**: System MUST automatically dismiss stall alerts (updating the Slack message and disabling action buttons) when the agent self-recovers and resumes making tool calls

### Key Entities

* **Approval Request**: A pending human decision on a code proposal. Attributes include a unique request ID, proposal title, description, diff content, target file path, risk level, status (pending, approved, rejected, expired, consumed), and creation timestamp. Belongs to exactly one Session.
* **Session**: A tracked instance of an agent process. Attributes include a unique session ID, state (created, active, paused, terminated), associated prompt/instruction, creation timestamp, and last activity timestamp. May have zero or more Checkpoints and zero or more Approval Requests.
* **Checkpoint**: A named snapshot of a session's state at a point in time. Attributes include a unique checkpoint ID, human-readable label, creation timestamp, serialized session state, and a manifest of workspace file hashes for divergence detection. Belongs to exactly one Session.
* **Continuation Prompt**: A forwarded meta-prompt from an agent. Attributes include a unique prompt ID, raw prompt text, prompt type (continuation, clarification, error recovery, resource warning), elapsed execution time, action count, and the operator's decision (continue, refine, stop). Belongs to exactly one Session.
* **Workspace Policy**: The auto-approve configuration for a workspace. Contains approved commands, approved tools, file path patterns, risk level threshold, and notification preferences. Loaded from a per-workspace configuration file.
* **Registry Command**: A pre-approved shell command mapped from a user-facing alias to an executable command string. Defined in the global configuration. Attributes include the alias key and the full command value.
* **Stall Alert**: A watchdog notification triggered by detected agent inactivity. Attributes include the session ID, last tool call name, last activity timestamp, elapsed idle time, nudge attempt count, alert status (pending, nudged, self-recovered, escalated, dismissed), and the operator's response action. Belongs to exactly one Session.

## Success Criteria *(mandatory)*

### Measurable Outcomes

* **SC-001**: The operator can review and approve a code proposal from a mobile device in under 30 seconds from the moment the notification arrives
* **SC-002**: Approved code changes are written to the local file system within 2 seconds of the operator tapping "Accept"
* **SC-003**: The server maintains Slack connectivity for 24-hour unattended sessions, automatically reconnecting after network interruptions within 5 minutes
* **SC-004**: 100% of pending approval requests survive a server restart and are recoverable by the agent upon reconnection
* **SC-005**: Auto-approved operations complete without any Slack round-trip, reducing agent blocking time for routine operations to zero
* **SC-006**: The operator can start, manage, and switch between up to 3 concurrent agent sessions from Slack without physical access to the workstation
* **SC-007**: Continuation prompt forwarding eliminates 100% of agent stalls caused by meta-level prompts during unattended operation
* **SC-008**: All file operations are constrained to the workspace root with zero path traversal escapes across all usage scenarios
* **SC-009**: Unauthorized Slack users are unable to interact with any server functionality, with 100% of unauthorized attempts logged
* **SC-010**: The server starts and becomes operational (MCP interface ready, Slack connected) within 10 seconds on standard hardware
* **SC-011**: Silent agent stalls are detected and the operator is alerted within the configured inactivity threshold (default: 5 minutes), eliminating undetected idle periods during unattended operation
* **SC-012**: Auto-nudge recovers stalled agents without operator intervention in at least 80% of stall events, reducing the need for manual nudges

## Assumptions

* The operator has a Slack workspace with a bot application configured for Socket Mode (App-Level Token and Bot Token available)
* The local workstation has a stable internet connection for Slack WebSocket communication, though temporary interruptions are tolerated
* The AI agent (Claude Code, GitHub Copilot CLI, Cursor, etc.) supports the Model Context Protocol and can connect to a local MCP server
* The workspace root directory is pre-configured in the server's global configuration file before startup
* Only one primary agent connects via the standard transport (stdio); additional spawned sessions connect via an HTTP-based transport on a local port
* The operator's Slack user ID is known in advance and configured in the server's authorized user list
* The host CLI binary for session spawning (e.g., "claude", "gh copilot") is installed and available on the system PATH
