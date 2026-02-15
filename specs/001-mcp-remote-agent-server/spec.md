# Feature Specification: MCP Remote Agent Server

**Feature Branch**: `001-mcp-remote-agent-server`
**Created**: 2026-02-08
**Status**: Draft
**Input**: User description: "Build an MCP server that provides remote I/O capabilities to local AI agents via Slack, enabling asynchronous code review, approval workflows, session orchestration, and continuation prompt forwarding from a mobile device"

## Clarifications

### Session 2026-02-09

- Q: How should the nudge message reach a stalled agent, given MCP is request-response and the server cannot push unsolicited messages? → A: Via an MCP server-to-client notification using a custom method name (e.g., `monocoque/nudge`). The agent registers a notification handler; no out-of-band channel is required.
- Q: How should the system handle conflicts when multiple authorized operators act on the same request? → A: Each agent session is bound to exactly one Slack user (owner) at creation time. Only the session owner may interact with that session's approvals, prompts, stall alerts, and slash commands. The `authorized_user_ids` list determines who may create sessions, but a given session accepts actions only from its owner. First-response-wins applies as a fallback for any residual race conditions.
- Q: Should tool call responses reset the inactivity timer, and should long-running operations suppress stall detection? → A: Hybrid approach. Both agent-initiated tool call requests and server responses reset the timer. A lightweight `heartbeat` MCP tool allows the agent to signal liveness during its own long-running local operations. The server automatically pauses the stall timer while it is executing a known long-running operation (e.g., a custom command). This covers both agent-side and server-side long operations with no blind spots.
- Q: Should all MCP tools be unconditionally exposed to every connected agent, or conditionally hidden based on configuration? → A: All tools are always visible to every connected agent regardless of configuration or session type. The server returns an error if a tool is called in a context where it does not apply. This keeps the tool surface simple and consistent.

### Session 2026-02-10

- Q: How should the server know where the agent is in its task list when a stall occurs mid-todo? → A: The existing `heartbeat` tool is extended to accept an optional structured progress snapshot (a list of todo items with labels and statuses). The server stores the most recent snapshot as opaque structured data on the Session record and uses it to enrich stall alerts, nudge messages, crash recovery responses, and checkpoint metadata. The server does not interpret the todo semantics — it is a store-and-forward cache. The agent is the sole source of truth for its own progress.
- Q: What is the data retention policy for approval requests, session state, checkpoints, and stall alerts? → A: Time-based auto-purge. All persisted data (sessions, approval requests, checkpoints, stall alerts) is automatically purged 30 days after the owning session is terminated. Active sessions are never purged.
- Q: How should Slack tokens and other sensitive credentials be stored? → A: OS keychain as the primary mechanism (Windows Credential Manager / macOS Keychain), with environment variables as a fallback when the keychain is unavailable or not configured.
- Q: What observability signals should the server emit beyond Slack notifications? → A: Structured tracing spans to stderr via `tracing-subscriber`. No metrics endpoint or external collector. Spans cover tool calls, Slack interactions, stall detection events, and session lifecycle transitions.
- Q: Which embedded database should be used for persistent state? → A: SurrealDB (already decided). Made explicit in the spec.
- Q: What is explicitly out of scope for v1? → A: Multi-machine/distributed deployment, web-based dashboard or UI (Slack is the sole remote interface), agent-to-agent communication or multi-agent collaboration, and custom Slack app distribution. Multi-workspace Slack support IS in scope — the server acts as a local service supporting multiple concurrent workspaces.

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

1. **Given** an active session where the agent has been making tool calls, **When** the agent stops making any tool calls or producing output for longer than the configured inactivity threshold, **Then** the server posts a stall alert to Slack with the agent's last known activity (last tool called, elapsed idle time, and the session's original prompt). If the session has a progress snapshot, the alert also renders a checklist showing completed and remaining todo items with the in-progress item highlighted
2. **Given** a stall alert in Slack, **When** the operator taps "Nudge", **Then** the server sends an MCP server-to-client notification with a configurable continuation message (e.g., "Continue working on the current task. Pick up where you left off.") and the agent resumes execution. If the session has a progress snapshot, the nudge notification payload includes a summary of completed items and the next pending item so the agent can reorient
3. **Given** a stall alert in Slack, **When** the operator taps "Nudge with Instructions", **Then** a dialog opens for the operator to type a custom nudge message, and the server injects that message into the agent's input
4. **Given** a stall alert in Slack, **When** the operator taps "Stop", **Then** the session is terminated and the operator receives confirmation
5. **Given** a stall alert has been posted, **When** the agent resumes activity on its own before the operator responds, **Then** the server updates the Slack alert to indicate the agent self-recovered and disables the action buttons
6. **Given** the inactivity threshold has elapsed and the operator does not respond to the stall alert, **When** a second configurable escalation threshold elapses, **Then** the server auto-nudges the agent with the default continuation message and posts a notification to Slack
7. **Given** the server has auto-nudged the agent, **When** the agent still does not resume within a configurable max-retries count, **Then** the server posts an escalated alert (with @channel mention) indicating the agent appears unresponsive and may require manual intervention
8. **Given** the operator has configured stall detection to be disabled for a session, **When** the agent goes idle, **Then** no stall alert is posted
9. **Given** the agent is performing a long-running local operation (e.g., processing a large codebase), **When** the agent calls the `heartbeat` tool periodically, **Then** the stall detection timer is reset and no stall alert is posted despite the absence of other tool calls
10. **Given** the server is executing a long-running custom command on behalf of the agent, **When** the command takes longer than the inactivity threshold, **Then** the stall timer is automatically paused for the duration of the command and no false stall alert is posted
11. **Given** the agent calls the `heartbeat` tool with a structured progress snapshot (a list of todo items with labels and statuses), **When** the server receives the call, **Then** the server stores the snapshot on the session record, resets the stall timer, and returns success. The snapshot replaces any previously stored snapshot for that session
12. **Given** the agent has previously reported a progress snapshot and subsequently stalls, **When** the auto-nudge fires, **Then** the auto-nudge continuation message includes a summary of the progress snapshot (e.g., "You completed 3/7 tasks. Resume with: 'update API docs'") so the agent can reorient without re-deriving its position
13. **Given** the agent calls the `heartbeat` tool without a progress snapshot (status message only or no arguments), **When** the server receives the call, **Then** the server resets the stall timer and leaves the existing progress snapshot (if any) unchanged

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
4. **Given** an active session, **When** the operator issues a "session-checkpoint" command with a label, **Then** the session state is snapshot and stored with the provided label, and a confirmation appears in Slack. If the session has a progress snapshot, it is included in the checkpoint metadata
5. **Given** a stored checkpoint, **When** the operator issues a "session-restore" command with the checkpoint ID, **Then** the server warns about file divergences (if any), and upon confirmation, restores the session to the checkpointed state
6. **Given** the concurrent session limit has been reached, **When** the operator attempts to start another session, **Then** the server returns an error indicating the limit has been exceeded
7. **Given** operator A owns an active session, **When** operator B (also in the authorized user list) attempts to interact with operator A's session, **Then** the interaction is rejected and operator B is informed the session belongs to a different operator

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
4. **Given** a session had a progress snapshot when the server was terminated, **When** the server restarts and the agent invokes state recovery, **Then** the recovered session state includes the last-reported progress snapshot so the agent can determine which todo items were completed and which remain

---

### User Story 10 - Operational Mode Switching (Priority: P3)

The developer switches the server between "remote", "local", and "hybrid" modes depending on their situation. When sitting at the desk, they switch to "local" mode to route approvals through a local CLI tool instead of Slack. When leaving the desk, they switch to "remote" to re-enable Slack notifications.

**Why this priority**: Mode switching provides flexibility but is an optimization over the default remote mode. Most users operate in remote or hybrid mode exclusively.

**Independent Test**: Set the mode to "local" and verify Slack notifications stop. Set it to "remote" and verify approvals flow through Slack again. Set it to "hybrid" and verify both channels are active.

**Acceptance Scenarios**:

1. **Given** the server is in "remote" mode, **When** the agent switches to "local" mode, **Then** subsequent approval requests are routed to the local IPC channel and Slack notifications are suppressed
2. **Given** the server is in any mode, **When** the mode is changed, **Then** the previous and current modes are returned, and the change is persisted across server restarts

---

### User Story 11 - Slack Environment Variable Configuration (Priority: P1)

The server reads Slack connectivity credentials from well-known user environment variables: `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, and `SLACK_TEAM_ID`. This provides a simple, portable configuration mechanism for development environments, CI/CD pipelines, and deployments where OS keychain access is unavailable or impractical. Environment variables serve as the fallback when the OS keychain does not contain the required credentials.

**Why this priority**: Without Slack credentials the server cannot connect to any workspace. Environment variables are the most universally available credential mechanism across all platforms and container runtimes. While the OS keychain is preferred for security, many legitimate deployment scenarios (containers, headless servers, CI runners) lack a keychain entirely. Ensuring explicit, documented support for these three environment variables removes a critical onboarding barrier.

**Independent Test**: Unset any keychain entries for the service. Set `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, and `SLACK_TEAM_ID` as environment variables. Start the server and verify it connects to Slack successfully using the environment-provided credentials.

**Acceptance Scenarios**:

1. **Given** the OS keychain does not contain Slack credentials, **When** `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, and `SLACK_TEAM_ID` are set as environment variables, **Then** the server loads all three values from the environment and connects to Slack successfully
2. **Given** the OS keychain contains Slack credentials, **When** the same credentials are also present as environment variables, **Then** the server uses the keychain values (keychain takes precedence over environment variables)
3. **Given** neither the OS keychain nor environment variables provide the required Slack credentials, **When** the server starts, **Then** the server fails with a clear, actionable error message identifying which credential is missing and how to provide it
4. **Given** the `SLACK_TEAM_ID` environment variable is set, **When** the server connects to Slack, **Then** the team ID is used to scope Socket Mode connections to the correct workspace
5. **Given** the `SLACK_TEAM_ID` environment variable is empty or unset and the keychain has no team ID, **When** the server connects to Slack, **Then** the server connects without a team ID constraint (single-workspace mode)

---

### User Story 12 - Dynamic Slack Channel Selection (Priority: P2)

When a remote agent connects to the server via the HTTP/SSE transport, the connecting client can specify a Slack channel ID as a query string parameter on the SSE endpoint URL (e.g., `/sse?channel_id=C_MY_CHANNEL`). This per-session channel override directs all Slack notifications, approval requests, and interactive messages for that agent session to the specified channel instead of the default channel from the global configuration. This enables multi-workspace setups where each connected IDE or project targets a different Slack channel for its notifications.

**Why this priority**: Multi-workspace support is an in-scope requirement. Different projects often need notifications routed to different Slack channels (e.g., a frontend project to `#frontend-agents` and a backend project to `#backend-agents`). Without per-session channel selection, all agent sessions would post to the same channel, creating noise and confusion. This is critical for the multi-workspace use case but not for single-workspace MVP operation.

**Independent Test**: Start the server with a default channel configured. Connect an agent via SSE with `?channel_id=C_TEST_CHANNEL` in the URL. Have the agent invoke `remote_log` and verify the message appears in `C_TEST_CHANNEL` rather than the default channel.

**Acceptance Scenarios**:

1. **Given** the server is running with a default Slack channel configured, **When** an agent connects via SSE with `?channel_id=C_OTHER_CHANNEL` in the URL, **Then** all Slack messages for that session are posted to `C_OTHER_CHANNEL`
2. **Given** an agent connects via SSE without a `channel_id` query parameter, **When** the agent invokes any tool that posts to Slack, **Then** the default channel from the global configuration is used
3. **Given** an agent connects via SSE with an empty `channel_id` parameter, **When** the agent invokes any tool that posts to Slack, **Then** the default channel from the global configuration is used (empty value is treated as absent)
4. **Given** two agents connect simultaneously with different `channel_id` values, **When** both agents invoke tools that post to Slack, **Then** each session's messages are routed to its respective channel independently
5. **Given** an agent connects via the stdio transport (primary agent), **When** the agent invokes any tool that posts to Slack, **Then** the default channel from the global configuration is always used (channel override is only available on SSE transport)

---

### User Story 13 - Service Rebranding to Remote Control (Priority: P1)

The service is renamed from "monocoque-agent-rem" (remote) to "monocoque-agent-rc" (remote control) across the entire codebase. This includes binary names, crate names, Cargo package metadata, configuration file references, OS keychain service identifiers, SurrealDB namespace and database names, documentation, user-facing CLI output, Slack message branding, the companion CLI tool name, and all internal code references. The rename establishes a consistent, intentional identity that accurately describes the service's purpose as a remote control interface for AI agents — not merely a "remote" endpoint.

**Why this priority**: Naming consistency is a foundational concern that affects every user-facing surface — binary names operators type, keychain entries operators configure, Slack messages operators read, and documentation operators reference. Performing the rename now, before external users adopt the current naming, avoids a disruptive breaking change later. After this rename, all new documentation, integrations, and user muscle memory will build on the correct name from the start.

**Independent Test**: After the rename, verify that `cargo build` produces a binary named `monocoque-agent-rc` (not `monocoque-agent-rem`). Verify that the companion CLI binary is named `monocoque-ctl` (unchanged). Verify that `config.toml` references the new service name. Verify that running the renamed binary starts the server with the correct SurrealDB database name and keychain service name.

**Acceptance Scenarios**:

1. **Given** the rename is complete, **When** `cargo build` is executed, **Then** the output binary is named `monocoque-agent-rc` and `monocoque-ctl`
2. **Given** the rename is complete, **When** the server starts, **Then** the SurrealDB namespace is `monocoque` and the database name is `agent_rc` (changed from `agent_rem`)
3. **Given** the rename is complete, **When** the server loads credentials from the OS keychain, **Then** it looks for the keychain service name `monocoque-agent-rc` (changed from `monocoque-agent-rem`)
4. **Given** the rename is complete, **When** the server emits tracing spans and log messages, **Then** all references use `monocoque-agent-rc` or `agent_rc` as appropriate
5. **Given** the rename is complete, **When** MCP server-to-client notifications are sent (e.g., nudge), **Then** the custom method prefix is `monocoque/nudge` (the `monocoque` namespace prefix is unchanged)
6. **Given** the rename is complete, **When** the user examines Cargo.toml, README, CLI help text, and config.toml comments, **Then** all references consistently use `monocoque-agent-rc` and there are zero remaining references to `monocoque-agent-rem` or `agent-rem` or `agent_rem` (except historical changelog entries)
7. **Given** an existing deployment that used the old `monocoque-agent-rem` keychain service name, **When** the server starts with the new name, **Then** the server does NOT automatically migrate keychain entries (the operator must re-store credentials under the new service name), and the startup error message clearly explains the required action

---

### Edge Cases

* What happens when the Slack WebSocket connection drops mid-approval? The server queues the pending request in the database and re-posts it to Slack upon reconnection.
* What happens when the operator taps "Accept" twice on the same proposal? The server processes only the first interaction and ignores duplicates (the buttons are replaced with a static status indicator after the first action).
* What happens when a diff targets a file path outside the workspace root? The server rejects the operation with a path violation error.
* What happens when the agent sends a log message while the Slack API is rate-limited? Messages are queued in memory and retried with exponential backoff after the rate limit clears.
* What happens when the operator issues a command that is not in the allowlist? The server rejects it with an explicit "command not found" error. No shell execution occurs.
* What happens when the workspace policy file is malformed? The server falls back to "require approval for everything" and logs a warning to both the console and Slack.
* What happens when someone other than the authorized operator interacts with buttons or commands? The interaction is silently ignored. A security event is logged with the unauthorized user's ID and the action they attempted.
* What happens when an authorized user who is not the session owner tries to interact with another user's session? The interaction is rejected with a message indicating the session belongs to a different operator. The event is logged but not treated as a security violation.
* What happens when a checkpoint restore detects workspace files that have diverged? The server warns the operator with a list of changed files and requires explicit confirmation before proceeding.
* What happens when the server receives a SIGTERM while an agent session is active? The server saves pending requests, notifies Slack, terminates spawned agent processes with a grace period, and exits cleanly.
* What happens when the agent stalls during a tool call that is handled by the host IDE (not the MCP server)? The server can only detect silence in the MCP tool call stream. If the agent is blocked on a non-MCP interaction (e.g., a local IDE confirmation dialog), the stall detector fires and the nudge is injected, but the agent may not be able to act on it until the local block is cleared. The stall alert informs the operator of this ambiguity.
* What happens when the agent stalls and the auto-nudge wakes it up, but it immediately stalls again? The server tracks consecutive nudge attempts per session. After exceeding the configurable max-retries, it escalates to the operator rather than continuing to auto-nudge in an infinite loop.
* What happens when multiple stall alerts fire in rapid succession across concurrent sessions? Each session has its own independent stall timer. Alerts are posted with the session ID prominently displayed so the operator can distinguish between them.
* What happens when the agent calls `heartbeat` indefinitely but never makes progress? The heartbeat resets the stall timer, so no stall alert fires. However, the operator retains visibility via `remote_log` messages and session elapsed time. A future enhancement could track heartbeat-without-progress as a distinct anomaly, but for v1 the heartbeat is trusted as a liveness signal.
* What happens when the agent sends a malformed or empty progress snapshot in the `heartbeat` call? The server validates the snapshot structure (an ordered list of items, each with a string label and a status enum). If the snapshot is malformed, the server rejects the heartbeat call with a descriptive error, leaves the existing snapshot unchanged, and still resets the stall timer.
* What happens when a checkpoint is restored and the stored progress snapshot no longer matches the agent's actual state? The restored progress snapshot is informational — the agent is the source of truth. The server includes the checkpoint's progress snapshot in the restore response so the agent can use it for orientation, but the agent is expected to submit a fresh progress snapshot via `heartbeat` once it re-evaluates its position. The server does not enforce consistency between the snapshot and actual file system state.
* What happens when `SLACK_BOT_TOKEN` is set as an environment variable but contains an invalid or expired token? The server accepts the value at startup (it cannot validate token freshness without contacting Slack). The Slack client connection attempt fails with an authentication error. The server logs the failure with a message suggesting the operator verify the token value.
* What happens when the OS keychain has a stale `slack_bot_token` and the environment variable has a fresh one? The keychain value takes precedence per FR-039. The operator must update the keychain entry or remove it to allow the environment variable fallback to activate.
* What happens when an SSE client provides a `channel_id` for a Slack channel the bot is not a member of? The server accepts the channel ID at connection time (no pre-validation). Subsequent Slack API calls to that channel fail with a "not_in_channel" error. The server logs the error and returns it to the agent as a tool call failure.
* What happens when the `channel_id` query parameter contains special characters or an invalid Slack channel ID format? The server does not validate the format of the channel ID — it passes it through to the Slack API, which rejects invalid IDs with an error. The error is surfaced to the agent.
* What happens when a user runs the renamed `monocoque-agent-rc` binary but their keychain still has credentials stored under the old `monocoque-agent-rem` service name? The server does not find the credentials in the keychain (it only checks the new service name `monocoque-agent-rc`). It falls back to environment variables. If those are also absent, the server fails with an error message that includes the expected keychain service name, helping the operator identify the mismatch.
* What happens when a user has a SurrealDB database from the old `agent_rem` name and starts the renamed server? The server creates a new `agent_rc` database. The old `agent_rem` data is not migrated automatically. The operator must manually migrate or re-initialize. The server does not access or delete the old database.

## Out of Scope (v1)

* Multi-machine or distributed deployment (server runs on a single local workstation)
* Web-based dashboard or UI (Slack is the sole remote interface)
* Agent-to-agent communication or multi-agent collaboration protocols
* Custom Slack app distribution or marketplace listing (assumes pre-configured bot)

> **Note**: Multi-workspace Slack support IS in scope. The server acts as a locally hosted service supporting multiple concurrent IDE workspaces (VS Code, GitHub Copilot CLI, etc.), each with its own agent sessions and workspace root.

## Requirements *(mandatory)*

### Functional Requirements

* **FR-001**: System MUST expose an MCP-compatible server interface that AI agents (Claude Code, GitHub Copilot CLI, Cursor, VS Code) can connect to via standard transports
* **FR-002**: System MUST maintain a persistent WebSocket connection to Slack via Socket Mode, requiring no inbound firewall ports or public IP addresses
* **FR-003**: System MUST render code diffs in Slack with size-adaptive formatting: inline code blocks for small diffs and uploaded snippets with syntax highlighting for large diffs
* **FR-004**: System MUST provide interactive "Accept" and "Reject" buttons on code proposals that resolve the agent's blocked state when the operator responds
* **FR-005**: System MUST apply approved code changes directly to the local file system, supporting both full-file writes and unified diff patch application
* **FR-006**: System MUST validate that all file operations resolve within the configured workspace root directory, rejecting path traversal attempts
* **FR-007**: System MUST persist all pending approval requests and session state to SurrealDB (embedded mode) that survives process restarts
* **FR-008**: System MUST forward agent continuation prompts to Slack with "Continue", "Refine", and "Stop" action buttons and return the operator's decision to the agent
* **FR-009**: System MUST support a workspace-level auto-approve policy file that permits pre-authorized operations to bypass the remote approval gate
* **FR-010**: System MUST hot-reload the workspace policy file when it changes, without requiring a server restart
* **FR-011**: System MUST enforce that the workspace policy cannot expand permissions beyond what the global configuration allows
* **FR-012**: System MUST allow the remote operator to start, pause, resume, terminate, checkpoint, and restore agent sessions via Slack slash commands
* **FR-013**: System MUST bind each agent session to exactly one Slack user (the session owner) at creation time. Only the session owner may interact with that session's approvals, prompts, stall alerts, and slash commands. Interactions from non-owner users (even if listed in `authorized_user_ids`) are rejected for that session. The `authorized_user_ids` list determines who may create new sessions.
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
* **FR-025**: System MUST track the timestamp of the most recent MCP activity (tool call requests, tool call responses, and heartbeat calls) for each active session and detect when the idle interval exceeds a configurable inactivity threshold. The stall timer is automatically paused while the server is executing a known long-running operation (e.g., a custom command) and resumes when the operation completes.
* **FR-026**: System MUST post a stall alert to Slack when an active session's inactivity threshold is exceeded, including the last tool called, elapsed idle time, and session context. If the session has a progress snapshot, the alert MUST render a checklist of todo items showing completed (✅), in-progress (🔄), and pending (⬜) items
* **FR-027**: System MUST provide a "Nudge" action on stall alerts that delivers a configurable continuation message to the agent via an MCP server-to-client notification (custom method name) to prompt it to resume work. If the session has a progress snapshot, the nudge notification payload MUST include a summary of completed items and the next pending item
* **FR-028**: System MUST support an auto-nudge escalation policy: after a configurable wait period without operator response, the server auto-nudges the agent and notifies Slack
* **FR-029**: System MUST cap the number of consecutive auto-nudge attempts per session and escalate to the operator with an elevated alert when the cap is exceeded
* **FR-030**: System MUST automatically dismiss stall alerts (updating the Slack message and disabling action buttons) when the agent self-recovers and resumes making tool calls
* **FR-031**: System MUST expose a lightweight `heartbeat` MCP tool that the agent can call during its own long-running local operations to reset the stall detection timer. The tool accepts an optional status message and an optional structured progress snapshot (a list of todo items, each with a label and a status of "done", "in_progress", or "pending"). When a progress snapshot is provided, the server persists it on the session record, replacing any previous snapshot. When omitted, any existing snapshot is preserved. The tool returns immediately with no side effects beyond resetting the timer, updating the progress snapshot, and optionally logging the status to the operator.
* **FR-033**: System MUST persist the most recently reported progress snapshot on the session record in the embedded database so that it survives server restarts. The progress snapshot MUST be included in state recovery responses and checkpoint metadata.
* **FR-034**: System MUST include the progress snapshot in auto-nudge continuation messages, summarizing completed items and identifying the next pending item so the agent can reorient after a stall without re-deriving its position.
* **FR-032**: System MUST unconditionally expose all MCP tools to every connected agent regardless of server configuration or session type. Tools called in inapplicable contexts (e.g., `heartbeat` when stall detection is disabled) MUST return a descriptive error rather than being hidden from the tool listing.
* **FR-035**: System MUST automatically purge all persisted data (sessions, approval requests, checkpoints, stall alerts) 30 days after the owning session is terminated. Active (non-terminated) sessions and their associated data MUST NOT be purged. The retention period MUST be configurable via the global configuration file.
* **FR-036**: System MUST load Slack tokens and other sensitive credentials from the OS keychain (Windows Credential Manager / macOS Keychain) as the primary mechanism. If the keychain is unavailable or credentials are not found, the system MUST fall back to reading from environment variables. Credentials MUST NOT be stored in plaintext configuration files.
* **FR-037**: System MUST emit structured tracing spans to stderr via `tracing-subscriber` covering MCP tool call execution, Slack API interactions, stall detection events, and session lifecycle transitions. No metrics endpoint or external telemetry collector is required.

### Functional Requirements — Slack Environment Variable Configuration (US11)

* **FR-038**: System MUST attempt to load `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, and `SLACK_TEAM_ID` from user environment variables when the corresponding credentials are not found in the OS keychain. The environment variable names are fixed and case-sensitive.
* **FR-039**: System MUST use the OS keychain as the primary credential source and environment variables as the fallback. When both sources contain a credential, the keychain value takes precedence.
* **FR-040**: System MUST fail startup with a clear, actionable error message if any required Slack credential (`SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`) cannot be found in either the OS keychain or the corresponding environment variable. The error message MUST identify the missing credential by name and describe both resolution methods (keychain and environment variable).
* **FR-041**: System MUST treat `SLACK_TEAM_ID` as optional. If absent from both keychain and environment, the server connects to Slack without a team ID constraint (suitable for single-workspace installations). If present, it is used to scope Socket Mode connections to the specified workspace.

### Functional Requirements — Dynamic Slack Channel Selection (US12)

* **FR-042**: System MUST accept an optional `channel_id` query string parameter on the HTTP/SSE transport endpoint URL (e.g., `/sse?channel_id=C_CHANNEL_ID`). When present and non-empty, all Slack messages for that SSE session are routed to the specified channel instead of the default `config.slack.channel_id`.
* **FR-043**: System MUST use the default `config.slack.channel_id` when the `channel_id` query parameter is absent, empty, or when the agent connects via the stdio transport.
* **FR-044**: System MUST support concurrent SSE sessions with different `channel_id` overrides, routing each session's Slack messages independently to its designated channel.

### Functional Requirements — Service Rebranding (US13)

* **FR-045**: System MUST be built and distributed as a binary named `monocoque-agent-rc` (replacing the former `monocoque-agent-rem` binary name). The companion CLI binary remains `monocoque-ctl`.
* **FR-046**: System MUST use the keychain service identifier `monocoque-agent-rc` when loading credentials from the OS keychain. The former service name `monocoque-agent-rem` is NOT checked as a fallback.
* **FR-047**: System MUST use the SurrealDB database name `agent_rc` (within the `monocoque` namespace) for all persistent storage. The former database name `agent_rem` is NOT automatically migrated.
* **FR-048**: System MUST update all user-visible references (CLI help text, tracing output, Slack message content, configuration file comments, README, error messages) to use `monocoque-agent-rc` consistently. Zero references to the former name `monocoque-agent-rem` or `agent-rem` shall remain in the codebase except in historical changelog or migration notes.
* **FR-049**: System MUST update the Cargo.toml package name and all internal Rust crate references to use the `rc` suffix consistently. All module names, test files, and import paths that previously referenced `rem` MUST be updated to `rc`.

### Key Entities

* **Approval Request**: A pending human decision on a code proposal. Attributes include a unique request ID, proposal title, description, diff content, target file path, risk level, status (pending, approved, rejected, expired, consumed), and creation timestamp. Belongs to exactly one Session.
* **Session**: A tracked instance of an agent process. Attributes include a unique session ID, owner Slack user ID (bound at creation, immutable for the session's lifetime), state (created, active, paused, terminated), associated prompt/instruction, creation timestamp, last activity timestamp, and last-reported progress snapshot (an optional ordered list of todo items, each with a label and status of "done", "in_progress", or "pending"). Only the owner may interact with the session's approvals, prompts, stall alerts, and commands. May have zero or more Checkpoints and zero or more Approval Requests.
* **Checkpoint**: A named snapshot of a session's state at a point in time. Attributes include a unique checkpoint ID, human-readable label, creation timestamp, serialized session state, a manifest of workspace file hashes for divergence detection, and the session's progress snapshot at the time of creation (if any). Belongs to exactly one Session.
* **Continuation Prompt**: A forwarded meta-prompt from an agent. Attributes include a unique prompt ID, raw prompt text, prompt type (continuation, clarification, error recovery, resource warning), elapsed execution time, action count, and the operator's decision (continue, refine, stop). Belongs to exactly one Session.
* **Workspace Policy**: The auto-approve configuration for a workspace. Contains approved commands, approved tools, file path patterns, risk level threshold, and notification preferences. Loaded from a per-workspace configuration file.
* **Registry Command**: A pre-approved shell command mapped from a user-facing alias to an executable command string. Defined in the global configuration. Attributes include the alias key and the full command value.
* **Stall Alert**: A watchdog notification triggered by detected agent inactivity. Attributes include the session ID, last tool call name, last activity timestamp, elapsed idle time, nudge attempt count, alert status (pending, nudged, self-recovered, escalated, dismissed), the operator's response action, and the session's progress snapshot at the time of the alert (if any). Belongs to exactly one Session.

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
* **SC-013**: The server starts successfully with Slack credentials provided exclusively via environment variables, with no keychain dependency, in under 10 seconds on standard hardware
* **SC-014**: Multiple concurrent SSE agent sessions, each specifying a different `channel_id`, route 100% of their Slack messages to their designated channels with zero cross-contamination
* **SC-015**: After the service rename, zero references to "monocoque-agent-rem", "agent-rem", or "agent_rem" exist in the codebase (excluding historical changelog entries), verified by automated grep across all source files, configuration files, and documentation

## Assumptions

* The operator has a Slack workspace with a bot application configured for Socket Mode (App-Level Token and Bot Token available)
* Slack credentials (bot token, app token, team ID) are available via the OS keychain under the service name `monocoque-agent-rc`, or as environment variables `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, and `SLACK_TEAM_ID`
* The local workstation has a stable internet connection for Slack WebSocket communication, though temporary interruptions are tolerated
* The AI agent (Claude Code, GitHub Copilot CLI, Cursor, etc.) supports the Model Context Protocol and can connect to a local MCP server
* Each connected agent is associated with a workspace root directory. The server supports multiple concurrent workspaces, each identified by its root path. Workspace roots are specified per-session at connection time rather than as a single global setting
* Only one primary agent connects via the standard transport (stdio); additional spawned sessions connect via an HTTP-based transport on a local port
* The operator's Slack user ID is known in advance and configured in the server's authorized user list
* The host CLI binary for session spawning (e.g., "claude", "gh copilot") is installed and available on the system PATH
* SurrealDB is used in embedded mode as the persistent storage engine for all session state, approval requests, checkpoints, and stall alerts
* The service is branded and distributed as `monocoque-agent-rc` (remote control). All binary names, keychain entries, database identifiers, and documentation use this name consistently
