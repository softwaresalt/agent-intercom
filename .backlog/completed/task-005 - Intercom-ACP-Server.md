---
id: TASK-005
title: "Intercom ACP Server"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - feature
dependencies: []
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
# Feature Specification: Intercom ACP Server

**Feature Branch**: `005-intercom-acp-server`
**Created**: 2026-02-28
**Status**: Draft
**Input**: Implement Agent Client Protocol (ACP) server mode for agent-intercom to actively send prompts to agents and receive responses, alongside the existing MCP passive server mode. Includes workspace-to-channel mapping refactor, session threading in Slack, and multi-session channel routing.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Dual-Mode Startup (Priority: P1)

An operator wants to choose how agent-intercom connects to agents. Today, agent-intercom runs exclusively as a passive MCP server — it waits for agents to connect and initiate tool calls. The operator needs an alternative mode where agent-intercom acts as an active controller, connecting outbound to a headless agent process and sending prompts to it. A startup flag selects the mode: MCP (passive, the current default) or ACP (active controller). Both modes share the same Slack integration, persistence layer, and policy engine — only the agent communication protocol differs.

**Why this priority**: This is the foundational architectural change. Every other user story in this feature depends on the mode selection mechanism existing and the shared core being properly abstracted behind a protocol-agnostic interface.

**Independent Test**: Can be fully tested by starting the server in ACP mode and verifying it attempts to connect to a configured agent endpoint, or by starting in MCP mode and confirming existing behavior is unchanged.

**Acceptance Scenarios**:

1. **Given** the server is started with `--mode mcp` (or no mode flag), **When** the startup sequence completes, **Then** the server behaves identically to the current implementation — it starts the MCP HTTP/SSE and/or stdio transports and waits for agent connections.
2. **Given** the server is started with `--mode acp`, **When** the startup sequence completes, **Then** the server spawns and connects to the configured agent process via stdio instead of listening for inbound MCP connections.
3. **Given** an invalid mode flag is passed, **When** the server starts, **Then** it exits with a clear error message listing valid mode options.
4. **Given** ACP mode is selected but no agent endpoint is configured, **When** the server starts, **Then** it exits with a clear error message specifying the missing configuration.

---

### User Story 2 - Agent Driver Abstraction (Priority: P1)

The Slack event loop and persistence layer should not know or care which protocol is used to communicate with the agent. When an operator clicks "Accept" or "Reject" in Slack, the system resolves the pending request regardless of whether the agent is connected via MCP or ACP. Similarly, when the agent requests clearance or sends a status update, the event reaches the Slack channel identically in both modes. This requires a protocol-agnostic abstraction layer that translates between the wire protocol and the shared application core.

**Why this priority**: Without this abstraction, every Slack handler, persistence operation, and policy check would need branching logic for each protocol. This makes the system untestable and unmaintainable. The driver abstraction is the architectural keystone that keeps the shared core clean.

**Independent Test**: Can be tested by implementing a mock driver that simulates both MCP and ACP event patterns and verifying the Slack handlers produce identical outputs for equivalent inputs.

**Acceptance Scenarios**:

1. **Given** an agent connected via MCP requests clearance, **When** the operator approves in Slack, **Then** the approval is delivered back to the agent through the MCP oneshot channel mechanism.
2. **Given** an agent connected via ACP requests clearance, **When** the operator approves in Slack, **Then** the approval is delivered back to the agent through the ACP response mechanism.
3. **Given** an agent sends a status update via either protocol, **When** the update is received, **Then** it is posted to the appropriate Slack channel in identical format regardless of protocol.
4. **Given** the agent disconnects unexpectedly via either protocol, **When** the disconnection is detected, **Then** the session is marked as interrupted and the operator is notified in Slack.

---

### User Story 3 - ACP Session Lifecycle via Slack (Priority: P1)

An operator wants to start, monitor, and terminate ACP agent sessions directly from Slack. The operator types `/intercom session-start "build a web server"` in a Slack channel, and the server spawns a headless agent process, connects to it via ACP, sends the initial prompt, and begins streaming status updates back to a new Slack thread. The operator can steer the session, approve file changes, and terminate the agent — all from the Slack thread. This extends the existing session-start mechanism to support ACP mode.

**Why this priority**: The operator experience is the primary value proposition. Without Slack-initiated sessions, ACP mode would require manual process management, defeating the purpose of a remote agent control plane.

**Independent Test**: Can be tested by issuing `/intercom session-start` in Slack, verifying an agent process is spawned, and confirming status updates appear in a dedicated Slack thread.

**Acceptance Scenarios**:

1. **Given** the server is running in ACP mode and the operator runs `/intercom session-start "implement feature X"`, **When** the command is processed, **Then** a new agent session is created in the database, a headless agent process is spawned, and the initial prompt is sent to the agent.
2. **Given** a new ACP session is created, **When** the initial Slack message is posted, **Then** it becomes the root of a new Slack thread, and all subsequent messages for that session are posted as thread replies.
3. **Given** an active ACP session, **When** the agent sends status updates or clearance requests, **Then** they appear as threaded replies in the session's Slack thread.
4. **Given** an active ACP session, **When** the operator runs `/intercom session-stop` or presses a "Terminate" button, **Then** the agent process is terminated, the session is marked as terminated, and a final status message is posted to the thread.
5. **Given** the agent process exits on its own (task completed or crash), **When** the exit is detected, **Then** the session is updated with the exit status and a final message is posted to the Slack thread.

---

### User Story 4 - Workspace-to-Channel Mapping (Priority: P2)

Currently, each MCP client passes a `channel_id` as a query parameter in the connection URL configured in `.vscode/mcp.json`. This tightly couples the Slack channel to the client's connection URL and requires updating each workspace's `mcp.json` when channels change. The operator wants to configure workspace-to-channel mappings centrally in `config.toml` and have workspaces identified by a namespace string instead. The connection URL would carry a `workspace_id` (or namespace) instead of `channel_id`, and the server would look up the corresponding channel from its configuration.

**Why this priority**: This is a prerequisite for multi-workspace support with proper channel routing. Without centralized mapping, each workspace needs hardcoded channel IDs in its local config, making channel changes error-prone and preventing dynamic reconfiguration.

**Independent Test**: Can be tested by configuring a workspace-to-channel mapping in `config.toml`, connecting with a `workspace_id` query parameter, and verifying Slack messages route to the mapped channel.

**Acceptance Scenarios**:

1. **Given** `config.toml` contains workspace-to-channel mappings, **When** an agent connects with `?workspace_id=my-project`, **Then** the server resolves the Slack channel from the mapping and routes all messages there.
2. **Given** an agent connects with `?workspace_id=unknown-project` and no mapping exists, **When** the connection is established, **Then** the server logs a warning and the session operates without Slack channel routing (local-only mode for that session).
3. **Given** an agent connects with both `?channel_id=C123` and `?workspace_id=my-project`, **When** the connection is established, **Then** `workspace_id` takes precedence and the `channel_id` parameter is ignored (with a deprecation warning logged).
4. **Given** a workspace-to-channel mapping is updated in `config.toml` while the server is running, **When** the configuration is hot-reloaded, **Then** new sessions for that workspace use the updated channel; existing sessions are unaffected.
5. **Given** a legacy client connects with only `?channel_id=C123` (no `workspace_id`), **When** the connection is established, **Then** the server falls back to using the provided `channel_id` directly (backward compatibility).

---

### User Story 5 - Session Threading in Slack (Priority: P2)

Each agent session should own a dedicated Slack thread for all its communication. Today, messages from multiple sessions can intermingle in a single channel, making it difficult to follow any individual session's activity. When a session starts, the first message posted to Slack becomes the thread root. All subsequent messages for that session — status updates, clearance requests, operator responses — are posted as replies to that thread. This applies to both MCP and ACP sessions.

**Why this priority**: Message organization is critical for operator sanity when managing multiple concurrent sessions. Without threading, channels become unreadable noise as sessions interleave their messages.

**Independent Test**: Can be tested by starting two concurrent sessions and verifying that each session's messages appear only in their respective Slack threads, with no cross-contamination.

**Acceptance Scenarios**:

1. **Given** a new session starts (either MCP or ACP), **When** the first Slack message for that session is posted, **Then** it is posted as a top-level message in the channel and its timestamp is recorded as the session's `thread_ts`.
2. **Given** an active session with a recorded `thread_ts`, **When** any subsequent message is sent for that session (status update, clearance request, broadcast), **Then** the message is posted as a reply to the session's thread.
3. **Given** two concurrent sessions in the same channel, **When** both sessions send messages, **Then** each session's messages appear only in their respective threads.
4. **Given** an operator clicks a button in a threaded message, **When** the interaction is processed, **Then** the response is posted in the same thread.
5. **Given** a session has a recorded `thread_ts`, **When** the session terminates, **Then** a final summary message is posted to the thread indicating the session has ended.

---

### User Story 6 - Multi-Session Channel Routing (Priority: P2)

When multiple agent sessions are active across different workspaces and channels, operator actions (button clicks, slash commands, steering messages) must be correctly routed to the right session. Today, some handlers pick the first active session from the database regardless of channel, which can cause cross-channel message misrouting. Every operator interaction must be scoped to the correct session by matching the originating Slack channel (and thread, when available) to the session's recorded channel and thread identifiers.

**Why this priority**: This is the correctness foundation for multi-workspace operation. Misrouted approvals or steering messages can cause agents to take incorrect actions on the wrong codebase.

**Independent Test**: Can be tested by running two sessions in different channels, performing operator actions in each channel, and verifying actions route to the correct session.

**Acceptance Scenarios**:

1. **Given** sessions A and B are active in channels X and Y respectively, **When** the operator clicks "Approve" on a clearance request in channel X, **Then** the approval is delivered to session A only.
2. **Given** the operator sends a steering message in channel Y, **When** the message is processed, **Then** it is queued for session B only.
3. **Given** a slash command is issued in a channel with no active session, **When** the command is processed, **Then** the operator receives a message indicating no active session exists in that channel.
4. **Given** multiple sessions exist in the same channel (different workspaces), **When** the operator interacts with a threaded message, **Then** the `thread_ts` is used to disambiguate and route to the correct session.

---

### User Story 7 - ACP Stream Processing (Priority: P2)

When running in ACP mode, the server needs to read and write messages on a continuous bidirectional stream to the agent process. The agent sends messages (clearance requests, status updates, progress reports) as line-delimited or framed payloads, and the server must parse them reliably even when the stream delivers partial data or multiple messages in a single read. Similarly, the server must serialize and send responses (approvals, prompts, cancellations) back to the agent in the correct wire format.

**Why this priority**: Robust stream processing is the foundation of reliable ACP communication. Without proper framing and parsing, partial messages will cause crashes or silent data corruption.

**Independent Test**: Can be tested by sending various message patterns (single messages, batched messages, partial deliveries) through a mock stream and verifying all are correctly parsed and dispatched.

**Acceptance Scenarios**:

1. **Given** the agent sends a complete message followed by a newline, **When** the server reads from the stream, **Then** the message is parsed and dispatched to the appropriate handler.
2. **Given** the agent sends two messages in quick succession, **When** both arrive in a single read, **Then** both messages are parsed and dispatched independently.
3. **Given** a message is split across two reads (partial delivery), **When** the second fragment arrives, **Then** the complete message is reassembled and dispatched.
4. **Given** the agent sends malformed data, **When** the server attempts to parse it, **Then** the malformed payload is logged and skipped without terminating the connection.
5. **Given** the server needs to send an approval response, **When** the response is serialized, **Then** it is written to the stream in the correct wire format with proper framing.

---

### User Story 8 - Offline Agent Message Queuing (Priority: P3)

When an agent session is offline, disconnected, or stalled, the operator may still want to send messages or queue instructions for it. The system should detect when an agent is unreachable, queue messages for it, and automatically deliver those messages when the agent reconnects. The operator should see clear feedback in Slack indicating the agent is offline and that their messages are being queued for later delivery.

**Why this priority**: Graceful handling of agent disconnections is important for reliability but not blocking for initial ACP functionality. The existing inbox queue mechanism (from feature 004) provides a foundation for this.

**Independent Test**: Can be tested by disconnecting an agent mid-session, sending messages from Slack, reconnecting the agent, and verifying queued messages are delivered.

**Acceptance Scenarios**:

1. **Given** an active session whose agent has disconnected, **When** the operator sends a steering message, **Then** the message is queued and the operator sees a notification that the agent is offline and the message has been queued.
2. **Given** queued messages exist for a session, **When** the agent reconnects, **Then** all queued messages are delivered in chronological order.
3. **Given** the stall detector marks an agent as unresponsive, **When** the status change is detected, **Then** the system automatically switches to queuing mode for that session and notifies the operator in Slack.
4. **Given** an agent reconnects after being offline, **When** queued messages are flushed, **Then** the Slack thread is updated to indicate the agent is back online and messages have been delivered.

---

### User Story 9 - ACP Stall Detection and Recovery (Priority: P3)

The existing stall detection mechanism (monitoring `ping` frequency and escalating via nudge messages) should work in ACP mode as well. When the agent stream goes silent for longer than the configured threshold, the stall detector should trigger. In ACP mode, nudge recovery means writing a prompt directly to the agent stream rather than waiting for the agent to call a tool. If nudge retries are exhausted, the operator is notified and can choose to terminate or restart the session.

**Why this priority**: Stall detection is important for unattended operation but can be built incrementally after the core ACP communication is working.

**Independent Test**: Can be tested by connecting an agent in ACP mode, stopping all agent output, and verifying the stall detector triggers and nudge messages are sent through the stream.

**Acceptance Scenarios**:

1. **Given** an active ACP session, **When** the agent stream is silent for longer than the configured inactivity threshold, **Then** the stall detector fires and a nudge message is sent directly on the agent stream.
2. **Given** a nudge message is sent, **When** the agent resumes sending messages, **Then** the stall detector resets and the session continues normally.
3. **Given** nudge retries are exhausted, **When** the maximum retry count is reached, **Then** the operator is notified in Slack with options to terminate or restart the session.
4. **Given** the operator chooses to restart the session from Slack, **When** the restart is initiated, **Then** the existing agent process is terminated, a new process is spawned with the original prompt, and the session continues in the same Slack thread.

### Edge Cases

- What happens when the agent process crashes during an ACP session while a clearance request is pending? The pending request should be marked as timed out and the operator should be notified.
- What happens when two workspaces map to the same Slack channel? Sessions are disambiguated by `thread_ts`; slash commands without thread context default to the most recently active session in that channel.
- What happens when the ACP stream connection is established but the agent never sends an initial handshake? A startup timeout fires and the session is marked as failed with a notification to the operator.
- What happens when `config.toml` is hot-reloaded and removes a workspace mapping for an active session? Existing sessions are unaffected; only new sessions use the updated mappings.
- What happens when the operator attempts to start an ACP session but the host CLI binary is missing or not executable? The server reports a clear error to Slack indicating the configured `host_cli` path is invalid.
- What happens when a Slack thread exceeds Slack's reply limit? The system posts a continuation message as a new top-level message in the channel, linking back to the original thread. The session's `thread_ts` remains unchanged (immutable) — subsequent messages continue in the original thread. Note: Slack does not expose a deterministic thread reply limit via its API, so this behavior is best-effort based on observed post failures.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support a `--mode` startup flag accepting `mcp` (default) and `acp` values to select the agent communication protocol.
- **FR-002**: In MCP mode, the system MUST behave identically to the current implementation — no regressions in existing MCP functionality.
- **FR-003**: In ACP mode, the system MUST spawn a configured agent process and establish bidirectional communication via its stdio streams (stdin/stdout), sending the initial prompt through the ACP stream protocol.
- **FR-004**: System MUST provide a protocol-agnostic interface (agent driver) that abstracts the communication protocol from the shared application core (Slack handlers, persistence, policy engine).
- **FR-005**: The agent driver MUST support resolving clearance requests (approve/reject), sending prompts, and interrupting agent execution regardless of the underlying protocol.
- **FR-006**: In ACP mode, the system MUST spawn and manage headless agent processes when initiated via `/intercom session-start`.
- **FR-007**: System MUST read from the ACP stream using a framing mechanism that handles partial reads, batched messages, and malformed payloads without crashing.
- **FR-008**: System MUST write responses to the ACP stream using the correct wire format and framing expected by the agent.
- **FR-009**: The ACP agent endpoint (host CLI binary path and arguments) MUST be configurable in `config.toml`.
- **FR-010**: System MUST support workspace-to-channel mappings in `config.toml` where each workspace namespace maps to a Slack channel ID.
- **FR-011**: The MCP connection URL MUST accept a `workspace_id` query parameter that replaces the current `channel_id` parameter for channel resolution.
- **FR-012**: System MUST maintain backward compatibility with the existing `channel_id` query parameter; if only `channel_id` is provided, it is used directly.
- **FR-013**: When both `workspace_id` and `channel_id` are provided, `workspace_id` MUST take precedence and a deprecation warning MUST be logged.
- **FR-014**: Workspace-to-channel mappings MUST be hot-reloadable — changes to `config.toml` take effect for new sessions without server restart.
- **FR-015**: Each session MUST record a `thread_ts` (Slack thread timestamp) after the first Slack message is posted.
- **FR-016**: All subsequent Slack messages for a session MUST be posted as threaded replies using the session's `thread_ts`.
- **FR-017**: Operator interactions (button clicks, slash commands, steering messages) MUST be routed to the correct session by matching the originating channel and thread context.
- **FR-018**: When no active session exists in the originating channel, slash commands MUST respond with a message indicating no active session.
- **FR-019**: When an agent is offline or disconnected, operator messages MUST be queued for later delivery (leveraging the inbox/steering queue from feature 004).
- **FR-020**: When a disconnected agent reconnects, queued messages MUST be delivered in chronological order.
- **FR-021**: The operator MUST see clear status indicators in Slack when an agent is offline, queued messages are pending, or the agent has reconnected.
- **FR-022**: Stall detection MUST function in ACP mode, monitoring stream activity instead of tool call frequency.
- **FR-023**: In ACP mode, nudge messages MUST be sent directly on the agent stream rather than waiting for the agent to call a tool.
- **FR-024**: System MUST handle agent process crashes gracefully — mark the session as interrupted, resolve any pending requests as timed out, and notify the operator.
- **FR-025**: System MUST support concurrent sessions across multiple workspaces, each with independent channel routing and threading.
- **FR-026**: Session lifecycle events (start, active, paused, terminated, interrupted) MUST be persisted in the database with the protocol mode recorded.
- **FR-027**: In ACP mode, the session's `channel_id` MUST be derived from the Slack channel where the `/intercom session-start` command was issued, not from a URL query parameter.
- **FR-028**: Spawned agent processes MUST NOT inherit the server's credential environment variables (e.g., Slack tokens). The spawner MUST clear the child process environment and explicitly allowlist only safe variables (e.g., `PATH`, `HOME`, `RUST_LOG`).
- **FR-029**: When a slash command is issued without thread context in a channel with multiple active sessions, the system MUST default to the most recently active session and include a disambiguation hint in the response listing other active sessions.
- **FR-030**: The initial prompt for an ACP session MUST be delivered via the ACP stream protocol (stdin), never as a command-line argument to the agent process. `host_cli_args` in configuration MUST be static and not include user-provided content.
- **FR-031**: All session-modifying actions (clearance resolution, prompt delivery, interruption, steering) MUST verify that the acting Slack user matches the session's `owner_user_id`. Actions from non-owners MUST be rejected with a descriptive error message.

### Remediation Requirements (Findings)

*Added post-implementation based on adversarial analysis (ES-*) and HITL testing (HITL-*) findings.*

#### Phase 13 — Critical & High-Priority Fixes

- **FR-032** *(HITL-003)*: In ACP mode, the MCP HTTP transport on the configured port MUST remain active so that agent subprocesses can reach MCP tools (`check_clearance`, `auto_check`, `check_diff`, `transmit`, `standby`, `reboot`). The transport MUST authenticate requests using a `session_id` query parameter and route tool calls through the `AcpDriver` for the matching session.
- **FR-033** *(HITL-003)*: MCP tool requests from ACP subprocesses MUST include a valid `session_id` query parameter. Requests with a missing or invalid `session_id` MUST be rejected with HTTP 401 Unauthorized.
- **FR-034** *(HITL-005)*: The `session-checkpoint` command MUST create the checkpoint on the session explicitly identified by the provided session ID argument. When no session ID is provided, it MUST fall back to the most recently active session in the channel. The `parse_checkpoint_args` function MUST correctly extract session ID and label from the argument list.
- **FR-035** *(HITL-006)*: `resolve_command_session` MUST resolve sessions in `Interrupted` status when an explicit session ID is provided. Implicit resolution (no session ID) MAY continue to match only `Active` sessions.
- **FR-036** *(HITL-006)*: The system MUST provide a `session-cleanup` slash command that force-terminates all `Interrupted` sessions in the originating channel. On server startup, if interrupted sessions exist, the system MUST post a Slack message listing them with a one-click "Clear All" button.

#### Phase 14 — Security Hardening

- **FR-037** *(ES-004)*: The ACP spawner MUST terminate the entire process tree when a session ends — using Job Objects on Windows and process group signals (`SIGTERM` to process group) on Unix. `kill_on_drop(true)` alone is insufficient for grandchild processes.
- **FR-038** *(ES-010)*: On startup, the server MUST log a `CRITICAL` tracing event if `host_cli` resolves to a path outside the system `PATH` or standard installation directories. The server MUST NOT block startup, but the warning MUST be prominent.
- **FR-039** *(ES-010)*: The `host_cli` configuration value MUST be validated as an existing, executable file at startup. If it does not exist or is not executable, the server MUST exit with a descriptive error (consistent with existing FR-004/FR-005 behavior for ACP mode).
- **FR-040** *(ES-008)*: All outbound ACP messages MUST include a monotonically increasing `seq` field (per session). This enables gap detection if the agent tracks received sequence numbers.
- **FR-041** *(ES-008)*: Write failures to the ACP stream (partial writes, broken pipe) MUST be logged at `WARN` level with the message method, session ID, and sequence number. The session MUST be marked as interrupted on write failure.

#### Phase 15 — Reliability & Observability

- **FR-042** *(HITL-001)*: The server MUST post a Slack notification via the HTTP REST API (not Socket Mode) when the Socket Mode WebSocket connection drops, and again when it recovers. Notifications MUST go to all channels with active sessions.
- **FR-043** *(HITL-007)*: ACP session lifecycle events MUST be written to the audit log. Required event types: `acp_session_start`, `acp_session_stop`, `acp_session_pause`, `acp_session_resume`, `acp_steer_delivered`, `acp_task_queued`. Each entry MUST include session ID, channel ID, and acting user ID.
- **FR-044** *(ES-005)*: The ACP reader MUST enforce a token-bucket rate limit on inbound messages (default: 10 messages/second, configurable via `[acp] max_msg_rate`). Exceeding the rate MUST trigger a `WARN` log. Sustained abuse (>3× rate for >5 seconds) MUST terminate the session.
- **FR-045** *(ES-006)*: On server startup, the stall detector MUST initialize timers for existing active/interrupted sessions using their persisted `last_activity_at` timestamps. The elapsed time since last activity (`now - last_activity_at`) MUST be used as the timer's initial value.
- **FR-046** *(ES-007)*: The session record MUST be committed to the database and the session MUST be registered in the driver map BEFORE the ACP reader task is started. Events received before registration MUST NOT cause panics or data loss.
- **FR-047** *(ES-009)*: During ACP session creation, channel resolution from workspace mappings MUST hold a read lock on the workspace configuration for the entire resolution+session-creation transaction to prevent races with hot-reload.

#### Phase 16 — Usability Improvements

- **FR-048** *(HITL-002)*: The `/arc sessions` command MUST support a `--all` flag that displays sessions in all statuses (active, paused, interrupted, terminated) with their status indicated. Without the flag, only active and paused sessions are shown.
- **FR-049** *(HITL-002)*: The session record MUST store a `title` field containing a truncated version of the initial prompt (max 80 characters). The title MUST be displayed in all session listings alongside the session ID.
- **FR-050** *(HITL-004)*: The `session-checkpoint` help text MUST accurately reflect parameter requirements. If session ID resolution falls back to most-recent-active, the help text MUST show `[session_id]` (optional). Error messages for failed resolution MUST clearly state the cause (e.g., "no active session in this channel" rather than treating the label as a session ID).
- **FR-051** *(HITL-008)*: The `/arc sessions` listing MUST include paused sessions with a `⏸` visual indicator. The `list_active` query (or a new `list_visible` query) MUST return both active and paused sessions.

### Key Entities

- **Agent Driver**: Protocol-agnostic abstraction for agent communication. Supports clearance resolution, prompt delivery, and session interruption. Has protocol-specific implementations for MCP and ACP.
- **Workspace Mapping**: Association between a workspace namespace (string identifier) and a Slack channel ID. Configured in `config.toml` and hot-reloadable.
- **Session Thread**: Extension of the Session entity with a `thread_ts` field linking the session to its dedicated Slack thread. All messages for the session are posted as replies to this thread.
- **ACP Stream**: Bidirectional communication channel between the server and an agent process. Uses line-delimited framing for message boundaries. Managed by the ACP driver implementation.

## Assumptions

- The host CLI agent (e.g., GitHub Copilot CLI) supports a headless/stdio mode where it reads prompts from stdin and writes responses to stdout, using line-delimited JSON or a similar framing protocol.
- The existing `host_cli` and `host_cli_args` configuration fields in `config.toml` are sufficient for specifying the agent binary and its arguments for ACP mode.
- A single server instance serves multiple workspaces simultaneously; workspace isolation is achieved through session-level scoping, not process-level isolation.
- The Slack Bot Token has sufficient permissions to post threaded replies (this is standard for bot tokens with `chat:write` scope).
- The existing inbox queue (feature 004) provides a persistence foundation for offline message queuing; this feature extends rather than replaces that mechanism.
- Feature 004 (Advanced Features — steering queue, inbox) MUST be complete before User Story 8 (Offline Agent Message Queuing) implementation begins, as US8 depends on the `steering_message` and `task_inbox` persistence tables.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Operator can start an ACP agent session from Slack and receive status updates within the same Slack thread within 10 seconds of issuing the command.
- **SC-002**: Switching between MCP and ACP modes requires only a startup flag change — no configuration file restructuring or code changes.
- **SC-003**: All existing MCP functionality passes regression testing with zero failures after the ACP mode is added.
- **SC-004**: Concurrent sessions in different workspaces route all messages to the correct Slack channel and thread with zero cross-contamination.
- **SC-005**: Agent disconnection is detected and the operator is notified within the configured stall detection threshold.
- **SC-006**: Queued messages for an offline agent are delivered within 5 seconds of the agent reconnecting.
- **SC-007**: Malformed agent stream messages are handled gracefully — logged and skipped — without interrupting the session or crashing the server.
- **SC-008**: Workspace-to-channel mapping changes in `config.toml` take effect for new sessions without requiring a server restart.


# Behavioral Matrix: Intercom ACP Server

**Input**: Design documents from `/specs/005-intercom-acp-server/`
**Prerequisites**: spec.md (required), plan.md (required), data-model.md, contracts/
**Created**: 2026-02-28

## Summary

| Metric | Count |
|---|---|
| **Total Scenarios** | 120 |
| Happy-path | 52 |
| Edge-case | 25 |
| Error | 20 |
| Boundary | 8 |
| Concurrent | 8 |
| Security | 7 |

**Non-happy-path coverage**: 57% (minimum 30% required)

## Dual-Mode Startup (US-1, FR-001, FR-002)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S001 | Server starts in MCP mode by default | No `--mode` flag | `agent-intercom --config config.toml` | MCP HTTP/SSE and stdio transports start, Slack connects | Server running, MCP transports listening | happy-path |
| S002 | Server starts in MCP mode explicitly | `--mode mcp` flag | `agent-intercom --mode mcp` | Identical behavior to S001 | Server running, MCP transports listening | happy-path |
| S003 | Server starts in ACP mode | `--mode acp`, valid `host_cli` configured | `agent-intercom --mode acp` | ACP driver initialized, no MCP transports started, Slack connects | Server running, ACP mode active | happy-path |
| S004 | ACP mode with missing host_cli config | `--mode acp`, `host_cli` empty or missing | `agent-intercom --mode acp` | Error: "ACP mode requires host_cli configuration" | Process exits, exit code 1 | error |
| S005 | ACP mode with non-existent host_cli binary | `--mode acp`, `host_cli = "/nonexistent/path"` | `agent-intercom --mode acp` | Error: "host_cli binary not found: /nonexistent/path" | Process exits, exit code 1 | error |
| S006 | Invalid mode flag value | `--mode invalid` | `agent-intercom --mode invalid` | clap error: "invalid value 'invalid' for '--mode'" | Process exits, exit code 2 | error |
| S007 | MCP mode regression — all existing tools visible | `--mode mcp`, agent connects | Agent sends `tools/list` | All 9 tools returned in response | Session active, tools available | happy-path |

---

## AgentDriver Abstraction (US-2, FR-004, FR-005)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S008 | MCP driver resolves clearance approved | Pending approval request `req-001` in MCP mode | Slack operator clicks "Accept" | oneshot channel sends `ApprovalResponse { status: "approved" }` | Approval resolved, pending map entry removed | happy-path |
| S009 | MCP driver resolves clearance rejected | Pending approval `req-001`, rejection reason "wrong file" | Slack operator clicks "Reject" | oneshot sends `ApprovalResponse { status: "rejected", reason: "wrong file" }` | Approval resolved, pending map entry removed | happy-path |
| S010 | ACP driver resolves clearance approved | Pending clearance `req-001` in ACP mode | Slack operator clicks "Accept" | `clearance/response` JSON written to agent stream with `status: "approved"` | Stream contains response, request removed from pending | happy-path |
| S011 | ACP driver resolves clearance rejected | Pending clearance `req-001` in ACP mode | Slack operator clicks "Reject" | `clearance/response` JSON with `status: "rejected"` written to stream | Stream contains response | happy-path |
| S012 | Resolve clearance with unknown request_id | No pending request for `req-999` | `driver.resolve_clearance("req-999", ...)` | Returns `AppError::NotFound` | No state change | error |
| S013 | Send prompt in ACP mode | Active ACP session | `driver.send_prompt(session_id, "do X")` | `prompt/send` JSON written to agent stream | Stream contains prompt message | happy-path |
| S014 | Send prompt to disconnected ACP session | ACP session, stream closed | `driver.send_prompt(session_id, "do X")` | Returns `AppError::Acp("write failed: stream closed")` | Session remains interrupted | error |
| S015 | Interrupt ACP session | Active ACP session | `driver.interrupt(session_id)` | `session/interrupt` JSON written to stream | Agent receives interrupt | happy-path |
| S016 | Interrupt already-terminated session | Terminated session | `driver.interrupt(session_id)` | Returns `Ok(())` (idempotent) | No state change | edge-case |
| S017 | Concurrent clearance resolution | Two pending requests, resolved simultaneously | Two threads call `resolve_clearance` concurrently | Both resolve independently, no data race | Both entries removed from pending map | concurrent |

---

## ACP Session Lifecycle (US-3, FR-003, FR-006)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S018 | Start ACP session from Slack | ACP mode, valid host_cli | `/intercom session-start "build a web server"` | Agent process spawned, session created in DB, initial prompt sent, Slack thread root posted | Session active, process running, thread_ts recorded | happy-path |
| S019 | ACP session status update appears in thread | Active ACP session with thread_ts | Agent sends `status/update` message | Status posted as Slack thread reply | Thread contains status message | happy-path |
| S020 | ACP session clearance request appears in thread | Active ACP session | Agent sends `clearance/request` message | Clearance request posted as Slack thread reply with buttons | Thread contains approval request | happy-path |
| S021 | Stop ACP session from Slack | Active ACP session | `/intercom session-stop` | Agent process terminated, session marked terminated, final message posted to thread | Session terminated, process killed | happy-path |
| S022 | Agent process exits on its own (success) | Active ACP session, agent exits with code 0 | Agent process EOF on stdout | Session marked terminated, "Task completed" posted to thread | Session terminated, exit_code: 0 | happy-path |
| S023 | Agent process crashes (non-zero exit) | Active ACP session, agent exits code 1 | Agent process crashes | Session marked interrupted, "Agent crashed (exit code: 1)" posted to thread | Session interrupted | error |
| S024 | Start ACP session at max concurrent limit | `max_concurrent_sessions` reached | `/intercom session-start "..."` | Error: "Maximum concurrent sessions reached" posted to Slack | No new session created | error |
| S025 | Agent process never sends initial message | Active ACP session, startup timeout = 30s | Agent hangs after spawn | After 30s: session marked failed, "Agent did not respond within startup timeout" posted | Session terminated, process killed | edge-case |
| S026 | Start session with empty prompt | ACP mode | `/intercom session-start ""` | Error: "prompt must not be empty" | No session created | boundary |

---

## Workspace-to-Channel Mapping (US-4, FR-010–FR-014)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S027 | Resolve workspace_id to channel | config: `[[workspace]] id="myproj" channel_id="C123"` | Agent connects `?workspace_id=myproj` | Channel resolved to C123, session gets `channel_id=C123` | Session created with channel_id | happy-path |
| S028 | Unknown workspace_id | config has no matching workspace | Agent connects `?workspace_id=unknown` | Warning logged, session operates without Slack channel | Session created, channel_id=NULL | edge-case |
| S029 | Legacy channel_id only | No workspace_id param | Agent connects `?channel_id=C456` | Deprecation warning logged, channel_id=C456 used directly | Session created with channel_id=C456 | happy-path |
| S030 | Both workspace_id and channel_id | workspace_id=myproj maps to C123, channel_id=C456 provided | Agent connects `?workspace_id=myproj&channel_id=C456` | workspace_id wins (C123 used), deprecation warning for channel_id | Session created with channel_id=C123 | happy-path |
| S031 | No workspace_id and no channel_id | Neither parameter provided | Agent connects to `/mcp` | Session operates in local-only mode (no Slack) | Session created, channel_id=NULL | edge-case |
| S032 | Duplicate workspace_id in config | Two `[[workspace]]` entries with `id="myproj"` | Server startup with bad config | Error: "duplicate workspace id: myproj" | Server exits, exit code 1 | error |
| S033 | Empty workspace_id in config | `[[workspace]] id="" channel_id="C123"` | Server startup | Error: "workspace id must not be empty" | Server exits, exit code 1 | error |
| S034 | Hot-reload workspace mapping | Active session using old mapping; config changed | Config file saved with new channel for workspace | Warning logged, new sessions use updated mapping; active session unaffected | Active session retains old channel | happy-path |
| S035 | Hot-reload removes workspace mapping | Active session for workspace "myproj"; mapping removed | Config file saved without "myproj" entry | Active session unaffected; new connections for "myproj" get no channel | Active session continues normally | edge-case |

---

## Session Threading in Slack (US-5, FR-015, FR-016)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S036 | First message creates thread root | New session, thread_ts=NULL | First Slack message posted | Message posted as top-level; `thread_ts` recorded from response ts | session.thread_ts set | happy-path |
| S037 | Subsequent messages are threaded | Session with thread_ts="1234.5678" | Status update received | Message posted with `thread_ts="1234.5678"` | Message appears in thread | happy-path |
| S038 | Clearance request is threaded | Session with thread_ts | Clearance request received | Approval buttons posted as thread reply | Buttons appear in session thread | happy-path |
| S039 | Button click response stays in thread | Operator clicks "Accept" in threaded message | Slack interaction event | Response posted in same thread | Thread contains acceptance message | happy-path |
| S040 | Terminal message in thread | Session with thread_ts, termination | Session terminates | "Session ended" message posted as thread reply | Thread has final summary | happy-path |
| S041 | Two sessions create separate threads | Two concurrent sessions in same channel | Both sessions post first message | Each gets unique thread_ts, messages don't cross | Two independent threads | concurrent |
| S042 | thread_ts is immutable after set | Session with thread_ts="1234.5678" | Attempt to update thread_ts | thread_ts unchanged | thread_ts remains "1234.5678" | boundary |

---

## Multi-Session Channel Routing (US-6, FR-017, FR-018, FR-025)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S043 | Route approval to correct session by channel | Session A in channel X, Session B in channel Y | Operator approves in channel X | Approval delivered to Session A only | Session B unaffected | happy-path |
| S044 | Route steering to correct session by channel | Session B in channel Y | Operator sends steering in channel Y | Steering queued for Session B only | Session A unaffected | happy-path |
| S045 | Slash command in channel with no session | No active session in channel Z | `/intercom session-stop` in channel Z | "No active session in this channel" | No state change | edge-case |
| S046 | Disambiguate by thread_ts in same channel | Session A (thread_ts=T1) and B (thread_ts=T2) in channel X | Operator clicks button in thread T1 | Action routed to Session A | Session B unaffected | happy-path |
| S047 | Slash command defaults to most recent session | Sessions A (older) and B (newer) in channel X | `/intercom status` in channel X (no thread) | Status for Session B (most recently active) | Correct session selected | edge-case |
| S048 | Three sessions, approval in correct channel | Sessions in channels X, Y, Z | Approvals in each channel | Each approval routes to correct session | All sessions handle independently | concurrent |

---

## ACP Stream Processing (US-7, FR-007, FR-008)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S049 | Parse single complete message | `{"method":"status/update","params":{"message":"hello"}}\n` | Read from agent stdout | `AgentEvent::StatusUpdated` emitted with message "hello" | Event dispatched to core | happy-path |
| S050 | Parse two batched messages | Two JSON lines in single read buffer | Read from agent stdout | Two separate `AgentEvent`s emitted in order | Both events dispatched | happy-path |
| S051 | Parse partially delivered message | First read: `{"method":"st`, second read: `atus/update","params":{"message":"hi"}}\n` | Sequential reads | Single `AgentEvent::StatusUpdated` emitted after reassembly | One event dispatched | happy-path |
| S052 | Handle malformed JSON line | `{not valid json}\n` | Read from agent stdout | Warning logged: "received malformed JSON", line skipped | Stream continues reading | error |
| S053 | Handle unknown method | `{"method":"unknown/method","params":{}}\n` | Read from agent stdout | Debug logged: "unknown method: unknown/method", skipped | Stream continues reading | edge-case |
| S054 | Handle missing required field | `{"method":"clearance/request","params":{"title":"x"}}\n` (missing file_path) | Read from agent stdout | Warning logged: "missing required field", skipped | Stream continues reading | error |
| S055 | Stream EOF detection | Agent process closes stdout | Read returns EOF | Reader emits EOF event; system awaits process exit code. If exit code 0 → `SessionTerminated` (per S022). If non-zero → `SessionInterrupted` (per S023). If process already exited → use cached exit code. | Session state depends on exit code, not EOF alone | happy-path |
| S056 | Write clearance response | Approval for request `req-001` | `driver.resolve_clearance("req-001", true, None)` | `{"method":"clearance/response","id":"req-001","params":{"status":"approved"}}\n` written to stdin | Agent receives response | happy-path |
| S057 | Message exceeds max line length | Single JSON line > 1 MB | Read from agent stdout | Error logged: "line exceeded max length", connection may close | Stream error handled | boundary |
| S058 | Empty line in stream | `\n` (empty line) between messages | Read from agent stdout | Empty line skipped silently | Stream continues reading | boundary |

---

## Offline Agent Message Queuing (US-8, FR-019–FR-021)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S059 | Queue steering for offline agent | Session active but agent disconnected | Operator sends steering in Slack | Message queued, "Agent offline — message queued" posted to thread | steering_message row created with consumed=0 | happy-path |
| S060 | Deliver queued messages on reconnect | 3 queued messages for session, agent reconnects | ACP stream reconnected / MCP agent calls ping | All 3 messages delivered in chronological order | All steering_messages marked consumed=1 | happy-path |
| S061 | Stall detector triggers offline mode | Agent silent past inactivity threshold, nudges exhausted | Stall detector escalation | Session marked stalled, "Agent unresponsive — switching to queue mode" posted | Status indicator updated in Slack | happy-path |
| S062 | Agent reconnect Slack notification | Agent reconnects after offline period | Stream activity resumes | "Agent back online — delivering N queued messages" posted to thread | Status cleared | happy-path |

---

## ACP Stall Detection (US-9, FR-022, FR-023, FR-024)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S063 | ACP stream activity resets stall timer | Active ACP session | Agent sends any message on stream | last_stream_activity updated | Stall timer reset | happy-path |
| S064 | ACP inactivity triggers stall | ACP session, no stream activity for threshold duration | Timer expiry | Stall detector fires, `nudge` message written to agent stream | nudge_count incremented | happy-path |
| S065 | ACP nudge recovery | Agent resumes after nudge | Agent sends message after receiving nudge | Stall detector resets, nudge_count reset | Session active, stall cleared | happy-path |
| S066 | ACP nudge retries exhausted | Agent silent after max_retries nudges | Last nudge expires | Operator notified in Slack: "Agent unresponsive after N nudge attempts" with Terminate/Restart buttons | Session stalled, awaiting operator | happy-path |
| S067 | Operator restarts ACP session | Stalled session, operator clicks "Restart" | Slack button interaction | Old process killed, new process spawned with original prompt, same Slack thread | Session active, new process, same thread_ts | happy-path |
| S068 | Agent crash with pending clearance | Active ACP session, pending clearance `req-001` | Agent process crashes | Pending clearance resolved as timeout, operator notified of crash | Session interrupted, approval expired | error |

---

## Session Model & Persistence (FR-026, data-model.md)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S069 | Session created with protocol_mode=acp | ACP session start | `Session::new()` with protocol_mode=acp | session record has `protocol_mode='acp'` | DB row created | happy-path |
| S070 | Session created with protocol_mode=mcp (default) | MCP agent connects | `on_initialized()` auto-creates session | session record has `protocol_mode='mcp'` | DB row created | happy-path |
| S071 | Schema migration adds new columns | Existing DB without new columns | `bootstrap_schema()` on startup | `protocol_mode`, `channel_id`, `thread_ts` columns added to session table | DB schema updated | happy-path |
| S072 | Unauthorized user attempts ACP session start | Non-authorized Slack user | `/intercom session-start "..."` | Command silently ignored per authorization guard | No session created | security |

---

## Additional Scenarios (Adversarial Review Remediations)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S073 | ACP driver resolves forwarded prompt | Active ACP session, agent sent `prompt/forward` with id `prompt-001`, operator responds | Slack operator clicks "Continue" | `prompt/response` JSON with `id: "prompt-001"`, `decision: "continue"` written to agent stream | Stream contains response, prompt removed from pending | happy-path |
| S074 | ACP driver resolves standby wait | Active ACP session in standby, operator sends instruction | Slack operator sends steering message | `prompt/send` JSON with instruction text written to agent stream | Agent receives instruction, standby resolved | happy-path |
| S075 | Spawned agent process does not inherit server credentials | ACP mode, server has SLACK_BOT_TOKEN in environment | `/intercom session-start "..."` | Agent process environment does NOT contain SLACK_BOT_TOKEN, SLACK_APP_TOKEN, or other server credentials. Only safe variables (PATH, HOME, RUST_LOG) are present. | Agent cannot access Slack API | security |
| S076 | Non-owner user rejected from session modification | User A owns session, User B (also authorized) clicks "Approve" on User A's clearance | Slack button interaction by User B | Error message: "Only the session owner can perform this action" posted as ephemeral Slack message | Clearance remains pending, no state change | security |

---

## Findings Remediation Scenarios (Phases 13–16)

### Phase 13: Critical & High-Priority Fixes (HITL-003, HITL-005, HITL-006)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S077 | ACP subprocess calls check_clearance via MCP HTTP | ACP mode, agent subprocess connects to `http://localhost:3000/mcp?session_id=<sid>` | Subprocess invokes `check_clearance` tool | Tool executes successfully, clearance request posted to Slack thread for the ACP session | Clearance pending, Slack thread updated | happy-path |
| S078 | ACP subprocess with invalid session_id rejected | ACP mode, subprocess connects with `?session_id=nonexistent` | Subprocess sends any MCP tool request | HTTP 401 Unauthorized returned: "invalid or unknown session_id" | No state change, request logged | security |
| S079 | ACP subprocess with missing session_id rejected | ACP mode, subprocess connects without session_id param | Subprocess sends MCP tool request | HTTP 401 Unauthorized returned: "session_id query parameter required" | No state change | security |
| S080 | MCP tools functional end-to-end in ACP mode | ACP mode, active session, subprocess connected via HTTP | Subprocess calls auto_check → check_clearance → check_diff | Full approval workflow completes: policy checked, clearance posted to Slack, operator approves, diff applied | File written, approval recorded | happy-path |
| S081 | session-checkpoint with explicit session_id | Two active sessions (A, B) in channel | `/arc session-checkpoint <session_A_id> my-label` | Checkpoint created under session A, not session B | Checkpoint record has session_id=A, label="my-label" | happy-path |
| S082 | session-checkpoint without session_id falls back | One active session in channel | `/arc session-checkpoint my-label` | Checkpoint created under the active session | Checkpoint record has correct session_id | happy-path |
| S083 | session-stop with explicit interrupted session_id | Session X in Interrupted status | `/arc session-stop <session_X_id>` | Session X terminated successfully | Session status = Terminated | happy-path |
| S084 | session-stop implicit resolution skips Interrupted | Session X (Interrupted), Session Y (Active) in channel | `/arc session-stop` (no session ID) | Session Y stopped (implicit resolves Active only) | Session Y terminated, Session X unchanged | edge-case |
| S085 | session-cleanup terminates all interrupted sessions | 3 Interrupted sessions in channel | `/arc session-cleanup` | All 3 sessions terminated, confirmation posted | All 3 sessions status = Terminated | happy-path |
| S086 | session-cleanup in channel with no interrupted sessions | Only active sessions in channel | `/arc session-cleanup` | "No interrupted sessions in this channel" | No state change | edge-case |
| S087 | Startup posts interrupted session list | Server restarts, 2 previously-active sessions become Interrupted | Server startup | Slack message posted listing 2 interrupted sessions with "Clear All" button | Sessions in Interrupted status | happy-path |
| S088 | Startup with no interrupted sessions | Clean startup, no previous sessions | Server startup | No interrupted session notification posted | Normal startup | edge-case |

---

### Phase 14: Security Hardening (ES-004, ES-010, ES-008)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S089 | Process tree killed on session stop (Windows) | ACP session, agent spawned child processes | `/arc session-stop` | Entire process tree terminated via Job Object, not just direct child | All processes in tree killed | happy-path |
| S090 | Process tree killed on session stop (Unix) | ACP session, agent spawned child processes | `/arc session-stop` | `SIGTERM` sent to process group, all children terminated | All processes in group killed | happy-path |
| S091 | Orphan process detection on startup | Server crashed previously, orphan agent process running | Server startup | Orphan processes detected and logged at WARN level | Warning logged, no auto-kill (operator decision) | edge-case |
| S092 | host_cli in system PATH — no warning | `host_cli = "copilot"`, copilot is in PATH | Server startup | No CRITICAL warning logged | Normal startup | happy-path |
| S093 | host_cli at unusual path — CRITICAL warning | `host_cli = "/tmp/downloads/agent"` | Server startup | CRITICAL tracing event: "host_cli resolves to non-standard location: /tmp/downloads/agent" | Server starts, warning logged | security |
| S094 | host_cli nonexistent — startup error | `host_cli = "/nonexistent/binary"`, ACP mode | Server startup | Error: "host_cli binary not found: /nonexistent/binary" | Server exits, exit code 1 | error |
| S095 | Outbound ACP messages include sequence numbers | Active ACP session, 3 messages sent | Server sends clearance/response, prompt/send, session/interrupt | Messages have seq=1, seq=2, seq=3 respectively | Sequence monotonically increasing | happy-path |
| S096 | Sequence number resets per session | Session A sends 5 messages, Session B starts | Session B sends first message | Session B's first message has seq=1 (independent of Session A) | Per-session sequence counters | boundary |
| S097 | Write failure logged on broken pipe | ACP session, agent process killed externally | Server attempts to write prompt/send | WARN log: "ACP write failed: broken pipe, session=<id>, method=prompt/send, seq=N" | Session marked Interrupted | error |

---

### Phase 15: Reliability & Observability (HITL-001, HITL-007, ES-005, ES-006, ES-007, ES-009)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S098 | WebSocket drops — notification posted | Active sessions in channels X and Y | Socket Mode WebSocket disconnects (OS error 10053) | HTTP REST message posted to channels X and Y: "⚠️ Slack connection interrupted — reconnecting..." | Channels notified | happy-path |
| S099 | WebSocket recovers — notification posted | Previous disconnect notification posted | Socket Mode reconnects | HTTP REST message posted: "✅ Slack connection restored" | Channels notified, normal operation resumes | happy-path |
| S100 | WebSocket drops with no active sessions | No active sessions | WebSocket disconnects | WARN logged but no Slack message posted (no channels to notify) | Warning in logs only | edge-case |
| S101 | ACP session-start writes audit entry | ACP mode, operator starts session | `/arc session-start "build feature"` | Audit log entry: `{event: "acp_session_start", session_id, channel_id, user_id, prompt}` | Audit file contains entry | happy-path |
| S102 | ACP session-stop writes audit entry | Active ACP session | `/arc session-stop` | Audit log entry: `{event: "acp_session_stop", session_id, channel_id, user_id}` | Audit file contains entry | happy-path |
| S103 | Steering delivery writes audit entry | Active ACP session, steering message sent | Operator sends message in session thread | Audit log entry: `{event: "acp_steer_delivered", session_id, channel_id, user_id, content}` | Audit file contains entry | happy-path |
| S104 | Normal message rate — no throttle | ACP session, agent sends 5 msg/sec | Sustained 5 msg/sec for 10 seconds | All messages processed normally, no warnings | No throttle triggered | happy-path |
| S105 | Burst exceeds rate limit — warning logged | ACP session, agent sends 15 msg/sec burst | Brief burst (< 5 seconds) | WARN logged: "ACP rate limit exceeded for session <id>: 15 msg/s (limit: 10)" | Warning only, messages processed | edge-case |
| S106 | Sustained flood terminates session | ACP session, agent sends 30+ msg/sec for > 5 seconds | Sustained flood | Session terminated, ERROR logged: "ACP session <id> terminated: sustained rate limit violation" | Session terminated, process killed | error |
| S107 | Stall timer initialized from DB on restart | Session with `last_activity_at` = 4 minutes ago, threshold = 5 min | Server restart | Stall timer starts with 4 minutes elapsed (1 minute until stall) | Timer correctly initialized | happy-path |
| S108 | Stall timer triggers immediately after restart | Session with `last_activity_at` = 10 minutes ago, threshold = 5 min | Server restart | Stall detector fires immediately for this session | Nudge sent or operator notified | edge-case |
| S109 | Session committed before reader starts | ACP session start, agent immediately sends status update | `/arc session-start`, agent writes to stdout within 10ms | Status update processed correctly — session found in DB and driver map | Event processed, no errors | happy-path |
| S110 | Reader event before session committed — graceful handling | Theoretical race: reader starts before DB commit | Agent sends message on stdout | Event buffered or retried; no panic, no data loss | WARN logged, event eventually processed | edge-case |
| S111 | Config reload during session creation — consistent channel | Session being created for workspace "proj", config reload removes "proj" mapping | Concurrent config reload | Session created with the channel that was valid at resolution time (read lock held) | Session has consistent channel_id | concurrent |
| S112 | Config reload after session creation — no effect | Active session for workspace "proj", mapping updated | Config reload changes "proj" channel | Active session retains original channel, new sessions use updated mapping | Active session unaffected | happy-path |

---

### Phase 16: Usability Improvements (HITL-002, HITL-004, HITL-008)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S113 | /arc sessions --all shows all statuses | 1 Active, 1 Paused, 1 Interrupted, 1 Terminated session | `/arc sessions --all` | All 4 sessions listed with status indicators: 🟢 Active, ⏸ Paused, ⚠️ Interrupted, ⏹ Terminated | All sessions visible | happy-path |
| S114 | /arc sessions (no flag) shows active + paused | 1 Active, 1 Paused, 1 Terminated | `/arc sessions` | 2 sessions listed (Active + Paused), Terminated hidden | Only visible sessions shown | happy-path |
| S115 | Session title derived from initial prompt | Session started with "Build the authentication module for the web app" | Session listed in `/arc sessions` | Title shown: "Build the authentication module for the web app" (truncated to 80 chars) | title field populated | happy-path |
| S116 | Session title truncation at 80 chars | Session started with a 200-character prompt | Session listed | Title shows first 77 characters + "..." | title field truncated correctly | boundary |
| S117 | session-checkpoint help shows correct syntax | Operator types help command | `/arc help session-checkpoint` | Help shows: `session-checkpoint [session_id] <label>` with clear description | Correct help text | happy-path |
| S118 | session-checkpoint without session_id — clear error when no active session | No active sessions in channel | `/arc session-checkpoint my-label` | Error: "no active session in this channel" (not "session my-label not found") | No checkpoint created | error |
| S119 | /arc sessions shows paused sessions with icon | 1 Active session, 1 Paused session | `/arc sessions` | Active: "🟢 abc123 — Build auth module", Paused: "⏸ def456 — Fix login bug" | Both sessions visible with indicators | happy-path |
| S120 | /arc sessions empty channel | No sessions at all in channel | `/arc sessions` | "No active sessions in this channel." | No output | edge-case |

---

## Edge Case Coverage Checklist

- [x] Malformed inputs and invalid arguments (S006, S026, S032, S033, S052, S054)
- [x] Missing dependencies and unavailable resources (S004, S005, S014, S025, S094)
- [x] State errors and race conditions (S016, S047, S068, S110, S111)
- [x] Boundary values (empty, max-length, zero, negative) (S026, S042, S057, S058, S096, S116)
- [x] Permission and authorization failures (S072, S078, S079)
- [x] Concurrent access patterns (S017, S041, S048, S111)
- [x] Graceful degradation scenarios (S028, S031, S035, S059, S100)
- [x] Process lifecycle and cleanup (S089, S090, S091)
- [x] Audit and observability (S101, S102, S103)
- [x] Rate limiting and DoS protection (S104, S105, S106)
- [x] Server restart recovery (S087, S107, S108)

## Cross-Reference Validation

- [x] Every entity in `data-model.md` has at least one scenario covering its state transitions (Session: S018-S026, S069-S071, S081-S088; WorkspaceMapping: S027-S035; AgentEvent: S049-S055; AcpMessage: S049-S058)
- [x] Every endpoint in `contracts/` has at least one happy-path and one error scenario (AgentDriver: S008-S017, S077-S080; ACP Stream: S049-S058, S095-S097; Workspace Mapping: S027-S035)
- [x] Every user story in `spec.md` has corresponding behavioral coverage (US-1: S001-S007, US-2: S008-S017, US-3: S018-S026, US-4: S027-S035, US-5: S036-S042, US-6: S043-S048, US-7: S049-S058, US-8: S059-S062, US-9: S063-S068)
- [x] Every remediation FR has at least one scenario (FR-032/033: S077-S080, FR-034: S081-S082, FR-035/036: S083-S088, FR-037: S089-S091, FR-038/039: S092-S094, FR-040/041: S095-S097, FR-042: S098-S100, FR-043: S101-S103, FR-044: S104-S106, FR-045: S107-S108, FR-046: S109-S110, FR-047: S111-S112, FR-048/049: S113-S116, FR-050: S117-S118, FR-051: S119-S120)
- [x] No scenario has ambiguous or non-deterministic expected outcomes

## Notes

- Scenario IDs are globally sequential (S001–S120) across all components
- Categories: `happy-path`, `edge-case`, `error`, `boundary`, `concurrent`, `security`
- Each row is deterministic — exactly one expected outcome per input state
- Security scenarios are minimal because the existing authorization guard (FR-013/SC-009 from base) covers most security paths; only ACP-specific security paths are added here
- Findings remediation scenarios (S077–S120) map 1:1 to findings in `findings.json`


# Data Model: Intercom ACP Server

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## Entity Changes

### Session (Modified)

The existing `Session` entity gains three new fields to support protocol tracking, Slack threading, and channel routing.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String (UUID) | Yes | Unique record identifier (existing) |
| `owner_user_id` | String | Yes | Owning Slack user ID (existing) |
| `workspace_root` | String | Yes | Absolute path to workspace directory (existing) |
| `status` | Enum | Yes | Lifecycle status: created, active, paused, terminated, interrupted (existing) |
| `prompt` | String | No | Initial prompt/instruction (existing) |
| `mode` | Enum | Yes | Operational routing mode: remote, local, hybrid (existing) |
| `created_at` | DateTime | Yes | Creation timestamp (existing) |
| `updated_at` | DateTime | Yes | Last activity timestamp (existing) |
| `terminated_at` | DateTime | No | Termination timestamp (existing) |
| `last_tool` | String | No | Most recent tool called (existing) |
| `nudge_count` | Integer | Yes | Consecutive nudge attempts (existing) |
| `stall_paused` | Boolean | Yes | Whether stall detection is paused (existing) |
| `progress_snapshot` | JSON | No | Last-reported progress items (existing) |
| **`protocol_mode`** | **Enum** | **Yes** | **Agent communication protocol: `mcp` or `acp`. Recorded at session creation. Default: `mcp`.** |
| **`channel_id`** | **String** | **No** | **Slack channel ID where this session's messages are posted. For MCP: resolved from workspace mapping or query parameter at connection time. For ACP: derived from the Slack channel where `/intercom session-start` was issued.** |
| **`thread_ts`** | **String** | **No** | **Slack thread timestamp of the session's root message. NULL until the first message is posted. All subsequent messages use this as `thread_ts`.** |
| **`connectivity_status`** | **Enum** | **Yes** | **Agent connectivity state: `online`, `offline`, or `stalled`. Separate from lifecycle `status`. Default: `online`. Updated by stream activity monitoring and stall detector.** |
| **`last_activity_at`** | **DateTime** | **No** | **Timestamp of last agent activity (stream message, tool call, heartbeat). Used by stall detector and persisted for recovery across server restarts.** |
| **`restart_of`** | **String** | **No** | **Session ID of the predecessor session if this session was created via a restart. NULL for original sessions. Enables session lineage tracking.** |

#### State Transitions

No changes to existing state transitions. The `protocol_mode` is immutable after creation.

```
Created → Active → Paused → Active (resume)
                 → Terminated
                 → Interrupted
Paused → Terminated
       → Interrupted
Interrupted → Active (recovery)
```

**Session Restart**: When an operator restarts a stalled/interrupted session, a new session record is created with a fresh UUID. The new session inherits `thread_ts` and `channel_id` from the original. The original session remains in `terminated` state. The new session's `restart_of` field links to the original session ID.

#### Validation Rules

- `protocol_mode` must be `mcp` or `acp`
- `channel_id` is set at session creation for ACP sessions (derived from the Slack channel where `/intercom session-start` was issued) or at first tool call for MCP sessions (derived from workspace mapping or query parameter)
- `thread_ts` is immutable once set — the session's Slack thread cannot change
- `connectivity_status` must be `online`, `offline`, or `stalled`
- `restart_of` must reference an existing session ID if set

---

### WorkspaceMapping (New — Config-Derived, Not Persisted)

Workspace-to-channel mapping loaded from `config.toml` at startup and held in memory. Not persisted to SQLite.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String | Yes | Workspace namespace identifier (e.g., `agent-intercom`, `my-backend`). Must be unique across all mappings. |
| `channel_id` | String | Yes | Slack channel ID to route messages for this workspace (e.g., `C0123FRONTEND`). |

#### Validation Rules

- `id` must be non-empty and contain only alphanumeric characters, hyphens, and underscores
- `channel_id` must match Slack channel ID format (starts with `C` or `G`, followed by alphanumeric characters)
- Duplicate `id` values are rejected at config load time
- Multiple workspaces may map to the same `channel_id` (sessions disambiguated by `thread_ts`)

---

### AgentEvent (New — Runtime Only, Not Persisted)

Events emitted by the ACP driver (or MCP driver) to the shared application core via `tokio::sync::mpsc` channel.

| Variant | Fields | Description |
|---------|--------|-------------|
| `ClearanceRequested` | `request_id: String`, `session_id: String`, `title: String`, `description: Option<String>`, `diff: Option<String>`, `file_path: String`, `risk_level: String` | Agent requests operator approval for a file operation |
| `StatusUpdated` | `session_id: String`, `message: String` | Agent sends a status update or log message |
| `PromptForwarded` | `session_id: String`, `prompt_id: String`, `prompt_text: String`, `prompt_type: String` | Agent forwards a continuation prompt for operator decision |
| `HeartbeatReceived` | `session_id: String`, `progress: Option<Vec<ProgressItem>>` | Agent sends a heartbeat/ping signal |
| `SessionTerminated` | `session_id: String`, `exit_code: Option<i32>`, `reason: String` | Agent process has exited or stream has closed |

#### Notes

- `AgentEvent` is the unified event type for both MCP and ACP drivers
- The MCP driver generates these events from tool call handlers
- The ACP driver generates these events from parsed stream messages
- The core event loop consumes these events identically regardless of source

---

### AcpMessage (New — Wire Format, Not Persisted)

JSON messages exchanged over the ACP stdio stream. Two directions: agent → server (inbound) and server → agent (outbound).

#### Inbound (Agent → Server)

| Method | Fields | Maps To |
|--------|--------|---------|
| `clearance/request` | `id: String`, `title: String`, `description: String`, `diff: Option<String>`, `file_path: String`, `risk_level: String` | `AgentEvent::ClearanceRequested` |
| `status/update` | `message: String` | `AgentEvent::StatusUpdated` |
| `prompt/forward` | `id: String`, `text: String`, `type: String` | `AgentEvent::PromptForwarded` |
| `heartbeat` | `progress: Option<Vec<ProgressItem>>` | `AgentEvent::HeartbeatReceived` |

#### Outbound (Server → Agent)

| Method | Fields | Description |
|--------|--------|-------------|
| `clearance/response` | `id: String` (envelope), `status: String`, `reason: Option<String>` | Approval decision from operator. Correlation via envelope `id` matching the original `clearance/request` id. |
| `prompt/send` | `text: String` | New prompt or instruction to the agent |
| `prompt/response` | `id: String`, `decision: String`, `instruction: Option<String>` | Decision on a forwarded continuation prompt |
| `session/interrupt` | `reason: String` | Request agent to stop current work |
| `nudge` | `message: String` | Stall recovery nudge message |

---

## Schema Migration

### DDL Additions

Add to `persistence/schema.rs` `bootstrap_schema()` function:

```sql
-- New columns on session table (idempotent via PRAGMA check)
-- protocol_mode: 'mcp' (default) or 'acp'
-- channel_id: Slack channel for this session
-- thread_ts: Slack thread timestamp for session threading
-- connectivity_status: 'online' (default), 'offline', or 'stalled'
-- last_activity_at: timestamp of last agent activity (for stall recovery across restarts)
-- restart_of: predecessor session ID for restarted sessions

ALTER TABLE session ADD COLUMN protocol_mode TEXT NOT NULL DEFAULT 'mcp';
ALTER TABLE session ADD COLUMN channel_id TEXT;
ALTER TABLE session ADD COLUMN thread_ts TEXT;
ALTER TABLE session ADD COLUMN connectivity_status TEXT NOT NULL DEFAULT 'online';
ALTER TABLE session ADD COLUMN last_activity_at TEXT;
ALTER TABLE session ADD COLUMN restart_of TEXT;
```

Since SQLite does not support `ALTER TABLE ADD COLUMN IF NOT EXISTS`, the migration must check `PRAGMA table_info(session)` before each `ALTER TABLE` statement.

### New Indexes

```sql
CREATE INDEX IF NOT EXISTS idx_session_channel ON session(channel_id, status);
CREATE INDEX IF NOT EXISTS idx_session_channel_thread ON session(channel_id, thread_ts);
```

## Relationship Diagram

```
┌──────────────────┐    config.toml     ┌─────────────────────┐
│ WorkspaceMapping │◄───────────────────│    GlobalConfig      │
│ (in-memory)      │                    │ + workspace_mappings │
└──────────────────┘                    └─────────────────────┘
        │ resolves channel_id                     │
        ▼                                         │
┌──────────────────────┐                ┌─────────────────────┐
│    Session           │◄───────────────│     AppState        │
│ + protocol_mode      │                │ + agent_driver      │
│ + channel_id         │                └─────────────────────┘
│ + thread_ts          │                          │
│ + connectivity_status│                ┌─────────────────────┐
│ + last_activity_at   │                │   AgentDriver       │
│ + restart_of         │                │   (trait object)    │
└──────────────────────┘                ├─────────────────────┤
        ▲                               │ McpDriver │AcpDriver│
        │ session_id                    │           │(per-sess)│
┌──────────────────┐                    └─────────────────────┘
│   AgentEvent     │◄───────────────────        ▲
│ (mpsc channel)   │                    session_id → Sender map
└──────────────────┘
```


# Quickstart: Intercom ACP Server

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## What This Feature Does

Adds an Agent Client Protocol (ACP) mode to agent-intercom where the server actively connects to and controls headless agent processes, alongside the existing passive MCP server mode. Also introduces workspace-to-channel mapping, per-session Slack threading, and multi-session channel routing.

## Key Architectural Decisions

1. **AgentDriver trait** — protocol-agnostic abstraction; Slack handlers call trait methods regardless of MCP/ACP
2. **NDJSON over stdio** — ACP uses line-delimited JSON for stream communication with agent processes
3. **Workspace mappings in config.toml** — centralized workspace-to-channel mapping replaces per-workspace `channel_id` query parameters
4. **Session threading** — each session owns a Slack thread via `thread_ts` on the session model
5. **Channel + thread routing** — session lookup scoped by `channel_id` and `thread_ts` to prevent cross-session misrouting

## Implementation Order

### Phase 1: Foundation
- `AgentDriver` trait and `AgentEvent` enum in `src/driver/`
- `McpDriver` wrapping existing oneshot pattern
- Session model additions (`protocol_mode`, `channel_id`, `thread_ts`)
- Schema migration for new columns
- Session repo: `find_by_channel`, `find_by_channel_and_thread` queries

### Phase 2: Workspace Mapping
- `WorkspaceMapping` config parsing in `config.rs`
- `workspace_id` query parameter in SSE middleware
- Backward compatibility for `channel_id` query parameter
- Hot-reload of workspace mappings via `notify` watcher

### Phase 3: Slack Threading
- `thread_ts` propagation through `SlackService`
- Session thread root message on first Slack post
- Thread-scoped button and modal interactions
- Multi-session routing fix (RI-04)

### Phase 4: ACP Stream
- ACP codec (`LinesCodec` wrapper) in `src/acp/codec.rs`
- Stream reader task: parse inbound → `AgentEvent`
- Stream writer task: serialize outbound responses
- `AcpDriver` implementation of `AgentDriver` trait

### Phase 5: ACP Session Lifecycle
- `--mode` CLI flag in `src/main.rs`
- ACP spawner: process launch + stdio capture
- ACP session start from Slack (`/intercom session-start`)
- Process exit monitoring → session termination
- ACP stall detection adaptation

### Phase 6: Integration & Polish
- Offline message queuing (extend 004 inbox)
- End-to-end integration tests
- Config documentation updates
- Migration guide for `channel_id` → `workspace_id`

## Files to Create

| File | Purpose |
|------|---------|
| `src/driver/mod.rs` | `AgentDriver` trait, `AgentEvent` enum |
| `src/driver/mcp_driver.rs` | MCP implementation of `AgentDriver` |
| `src/driver/acp_driver.rs` | ACP implementation of `AgentDriver` |
| `src/acp/mod.rs` | ACP module root |
| `src/acp/codec.rs` | NDJSON codec for stream framing |
| `src/acp/reader.rs` | Inbound stream parser |
| `src/acp/writer.rs` | Outbound stream serializer |
| `src/acp/spawner.rs` | Agent process spawning and stdio capture |

## Files to Modify

| File | Changes |
|------|---------|
| `src/main.rs` | Add `--mode` CLI flag, ACP startup branch |
| `src/config.rs` | Add `WorkspaceMapping` config, `[[workspace]]` parsing |
| `src/errors.rs` | Add `AppError::Acp(String)` variant |
| `src/models/session.rs` | Add `protocol_mode`, `channel_id`, `thread_ts` fields |
| `src/persistence/schema.rs` | ALTER TABLE for new session columns + indexes |
| `src/persistence/session_repo.rs` | New query methods for channel/thread routing |
| `src/mcp/handler.rs` | Wire `AgentDriver` into `AppState` |
| `src/mcp/sse.rs` | Parse `workspace_id` query param, deprecation warning |
| `src/slack/client.rs` | Add `thread_ts` parameter to message posting |
| `src/slack/events.rs` | Extract `thread_ts` for routing |
| `src/slack/handlers/steer.rs` | Channel-scoped session lookup (RI-04 fix) |
| `src/orchestrator/stall_detector.rs` | Stream activity monitoring for ACP |

## Testing Strategy

- **Unit tests**: Driver trait behavior, codec parsing, workspace resolution, session routing
- **Contract tests**: Session model with new fields, driver response shapes, stream message format
- **Integration tests**: Full ACP lifecycle, multi-workspace routing, Slack threading
- **Regression**: All existing MCP tests must pass unchanged

## Dependencies on Feature 004

- Steering queue (`steering_message` table, `/intercom steer` command) — used for offline message queuing
- Task inbox (`task_inbox` table) — extended for ACP session cold-start
- Policy hot-reload (`PolicyWatcher`) — pattern reused for workspace mapping hot-reload
- Audit logging (`AuditLogger`) — ACP events emitted to audit log




---

## Checklists

# Specification Quality Checklist: Intercom ACP Server

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-02-28
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- Spec is ready for `/speckit.plan`. No clarification markers remain.
- Assumptions section documents reasonable defaults for wire protocol, host CLI interface, and Slack permissions.
- Feature 004 inbox/steering queue is listed as a dependency for offline message queuing (US-8).




---

## Contracts

# Contract: ACP Stream Protocol

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## Overview

Defines the wire format for bidirectional communication between agent-intercom (server) and a headless agent process over stdio. Uses newline-delimited JSON (NDJSON) — one JSON object per line, `\n` delimiter.

## Framing

- **Codec**: `tokio_util::codec::LinesCodec` wrapping `ChildStdout` (read) and `ChildStdin` (write)
- **Encoding**: UTF-8
- **Delimiter**: `\n` (LF)
- **Max line length**: 1 MB (configurable via `LinesCodec::new_with_max_length`)

## Message Envelope

All messages follow a JSON-RPC-like envelope:

```json
{
  "method": "clearance/request",
  "id": "optional-correlation-id",
  "params": { ... }
}
```

- `method` (string, required): Message type identifier
- `id` (string, optional): Correlation ID for request/response pairs. Present on requests that expect a response.
- `params` (object, required): Method-specific payload

## Inbound Messages (Agent → Server)

### `clearance/request`

Agent requests operator approval for a file operation.

```json
{
  "method": "clearance/request",
  "id": "req-001",
  "params": {
    "title": "Create new module",
    "description": "Adding src/driver/mod.rs",
    "diff": "--- /dev/null\n+++ b/src/driver/mod.rs\n@@ ...",
    "file_path": "src/driver/mod.rs",
    "risk_level": "low"
  }
}
```

**Response expected**: `clearance/response` with matching `id`.

### `status/update`

Agent sends a status or log message.

```json
{
  "method": "status/update",
  "params": {
    "message": "Running cargo test..."
  }
}
```

**Response expected**: None (fire-and-forget).

### `prompt/forward`

Agent forwards a continuation prompt for operator decision.

```json
{
  "method": "prompt/forward",
  "id": "prompt-001",
  "params": {
    "text": "Should I refactor the error handling?",
    "type": "continuation"
  }
}
```

**Response expected**: `prompt/response` with matching `id`.

### `heartbeat`

Agent sends a liveness signal.

```json
{
  "method": "heartbeat",
  "params": {
    "progress": [
      { "label": "Writing tests", "status": "in_progress" },
      { "label": "Implementation", "status": "pending" }
    ]
  }
}
```

**Response expected**: None. On heartbeat receipt, the server checks for pending steering messages for this session. If pending messages exist, each is sent as a separate `prompt/send` outbound message. The heartbeat itself does not receive a direct response.

## Outbound Messages (Server → Agent)

### `clearance/response`

Operator's decision on a clearance request. Correlation via envelope `id` matching the original `clearance/request` id.

```json
{
  "method": "clearance/response",
  "id": "req-001",
  "params": {
    "status": "approved",
    "reason": null
  }
}
```

> **Note**: The `id` field in the envelope is the correlation key. Do NOT duplicate it as `request_id` inside `params`.

### `prompt/send`

New prompt or instruction from the operator.

```json
{
  "method": "prompt/send",
  "params": {
    "text": "Focus on the error handling module next."
  }
}
```

### `prompt/response`

Decision on a forwarded continuation prompt.

```json
{
  "method": "prompt/response",
  "id": "prompt-001",
  "params": {
    "decision": "continue",
    "instruction": null
  }
}
```

### `session/interrupt`

Request agent to stop current work.

```json
{
  "method": "session/interrupt",
  "params": {
    "reason": "Operator requested termination"
  }
}
```

### `nudge`

Stall recovery message.

```json
{
  "method": "nudge",
  "params": {
    "message": "Continue working on the current task. Pick up where you left off."
  }
}
```

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Malformed JSON line | Log warning with raw line content, skip, continue reading |
| Unknown method | Log debug, skip, continue reading |
| Missing required field | Log warning with method name and missing field, skip message |
| Stream EOF (stdout closed) | Emit `SessionTerminated` event with reason "stream closed" |
| Write to closed stdin | Return `AppError::Acp("write failed: stream closed")` |

## Codec Configuration

```toml
[acp]
max_line_length = 1048576   # 1 MB
startup_timeout_seconds = 30
```

> **Note**: Only NDJSON framing is supported. Content-Length (LSP-style) framing may be added in a future version if needed.

## Test Contract

1. **Single message parsing** — complete JSON line → parsed `AgentEvent`
2. **Batched messages** — two messages in one read → two separate events
3. **Partial delivery** — split JSON across reads → single complete event after reassembly
4. **Malformed line** — invalid JSON → logged and skipped, stream continues
5. **Unknown method** — valid JSON, unknown method → logged and skipped
6. **Stream EOF** — stdout closes → `SessionTerminated` event emitted
7. **Write serialization** — `clearance/response` → valid NDJSON line with correct `id` correlation


---

# Contract: AgentDriver Trait

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## Overview

The `AgentDriver` trait defines the protocol-agnostic interface between the shared application core (Slack handlers, persistence, policy) and the agent communication protocol (MCP or ACP). All operator actions that affect the agent flow through this trait.

## Trait Definition

```rust
pub trait AgentDriver: Send + Sync {
    /// Resolve a pending clearance request (approve or reject).
    ///
    /// In MCP: Sends the response through the oneshot channel.
    /// In ACP: Writes a clearance/response message to the agent stream.
    fn resolve_clearance(
        &self,
        request_id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Send a new prompt or instruction to the agent.
    ///
    /// In MCP: Posts an MCP notification or is a no-op (IDE owns the prompt).
    /// In ACP: Writes a prompt/send message to the agent stream.
    fn send_prompt(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Interrupt/cancel the agent's current work.
    ///
    /// In MCP: Sends a cancellation signal via the MCP transport.
    /// In ACP: Writes a session/interrupt message and optionally kills the process.
    fn interrupt(
        &self,
        session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Resolve a pending continuation prompt.
    ///
    /// In MCP: Sends the response through the prompt oneshot channel.
    /// In ACP: Writes the decision back to the agent stream.
    fn resolve_prompt(
        &self,
        prompt_id: &str,
        decision: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Resolve a pending wait-for-instruction (standby).
    ///
    /// In MCP: Sends through the wait oneshot channel.
    /// In ACP: Writes a prompt/send message with the instruction.
    fn resolve_wait(
        &self,
        session_id: &str,
        instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}
```

## AgentEvent Enum

Events emitted by driver implementations into the shared `mpsc` channel:

```rust
pub enum AgentEvent {
    ClearanceRequested {
        request_id: String,
        session_id: String,
        title: String,
        description: String,
        diff: Option<String>,
        file_path: String,
        risk_level: String,
    },
    StatusUpdated {
        session_id: String,
        message: String,
    },
    PromptForwarded {
        session_id: String,
        prompt_id: String,
        prompt_text: String,
        prompt_type: String,
    },
    HeartbeatReceived {
        session_id: String,
        progress: Option<Vec<ProgressItem>>,
    },
    SessionTerminated {
        session_id: String,
        exit_code: Option<i32>,
        reason: String,
    },
}
```

## MCP Driver Behavior

| Method | Implementation |
|--------|---------------|
| `resolve_clearance` | Removes `oneshot::Sender<ApprovalResponse>` from `pending_approvals` map, sends response |
| `send_prompt` | Sends MCP notification `intercom/nudge` via the notification context |
| `interrupt` | Sends MCP cancellation or drops the session connection |
| `resolve_prompt` | Removes `oneshot::Sender<PromptResponse>` from `pending_prompts` map, sends response |
| `resolve_wait` | Removes `oneshot::Sender<WaitResponse>` from `pending_waits` map, sends response |

## ACP Driver Behavior

| Method | Implementation |
|--------|---------------|
| `resolve_clearance` | Looks up session_id from pending map key `(session_id, request_id)`, serializes `clearance/response` JSON, writes to the session's stream via `stream_writers[session_id]` channel |
| `send_prompt` | Looks up `stream_writers[session_id]`, serializes `prompt/send` JSON, writes to agent stream |
| `interrupt` | Looks up `stream_writers[session_id]`, serializes `session/interrupt` JSON, writes to agent stream, optionally kills process |
| `resolve_prompt` | Looks up session_id from pending map key `(session_id, prompt_id)`, serializes prompt decision JSON, writes to agent stream |
| `resolve_wait` | Looks up `stream_writers[session_id]`, serializes `prompt/send` JSON with instruction, writes to agent stream |

### AcpDriver Internal Structure

```rust
/// ACP driver managing multiple concurrent agent sessions.
pub struct AcpDriver {
    /// Per-session stream writers: session_id → sender channel
    stream_writers: Arc<Mutex<HashMap<String, mpsc::Sender<Value>>>>,
    /// Pending clearance requests keyed by (session_id, request_id)
    pending_clearances: Arc<Mutex<HashMap<(String, String), ClearanceState>>>,
    /// Pending prompt requests keyed by (session_id, prompt_id)
    pending_prompts: Arc<Mutex<HashMap<(String, String), PromptState>>>,
}
```

### Session Lifecycle Methods

```rust
impl AcpDriver {
    /// Register a new session's stream writer.
    pub fn register_session(&self, session_id: &str, writer: mpsc::Sender<Value>);
    /// Remove a session's stream writer on termination.
    pub fn deregister_session(&self, session_id: &str);
}
```

## Error Cases

| Scenario | Expected Behavior |
|----------|------------------|
| `resolve_clearance` with unknown `(session_id, request_id)` | Return `AppError::NotFound` |
| `send_prompt` to disconnected session | Return `AppError::Acp("stream closed")` |
| `send_prompt` to unknown session_id | Return `AppError::NotFound` |
| `interrupt` on already-terminated session | Return `Ok(())` (idempotent) |
| Stream write failure | Return `AppError::Acp` with the underlying I/O error |
| Action by non-owner user | Return `AppError::Unauthorized` (FR-031) |

## Test Contract

All driver implementations must pass these contract tests:

1. **resolve_clearance approved** — resolves pending request, event loop receives the approval
2. **resolve_clearance rejected** — resolves pending request, event loop receives the rejection with reason
3. **resolve_clearance unknown** — returns `NotFound` error
4. **send_prompt** — delivers prompt text to agent; for ACP, the stream contains the serialized message
5. **interrupt** — signals the agent to stop; for ACP, the stream contains the interrupt message
6. **concurrent resolution** — two clearance requests resolved concurrently, both succeed without data races
7. **session-scoped IDs** — two sessions both have `req-001`; resolving one does not affect the other
8. **owner verification** — action by non-owner user returns `Unauthorized`
9. **ACP prompt resolution** — resolve_prompt writes `prompt/response` to correct session stream
10. **ACP wait resolution** — resolve_wait writes `prompt/send` to correct session stream

## Owner Verification

All session-modifying methods (`resolve_clearance`, `send_prompt`, `interrupt`, `resolve_prompt`, `resolve_wait`) require that the caller provides the acting user's Slack ID. The driver (or the calling handler layer) MUST verify that this user matches the session's `owner_user_id` before proceeding. Non-owner actions MUST return `AppError::Unauthorized`.


---

# Contract: Workspace-to-Channel Mapping

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## Overview

Defines how workspace namespaces map to Slack channel IDs, replacing the per-workspace `channel_id` query parameter with centralized configuration.

## Configuration Format

```toml
# config.toml

[[workspace]]
id = "agent-intercom"
channel_id = "C0123FRONTEND"

[[workspace]]
id = "my-backend"
channel_id = "C0456BACKEND"

[[workspace]]
id = "shared-libs"
channel_id = "C0123FRONTEND"   # Multiple workspaces can share a channel
```

## Resolution Logic

### Query Parameter Handling

The SSE/MCP endpoint accepts these query parameters:

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace_id` | String | Workspace namespace to resolve via config mapping (new) |
| `channel_id` | String | Direct Slack channel ID (legacy, deprecated) |
| `session_id` | String | Pre-created session ID for spawned agents (existing) |

### Resolution Priority

```
1. If workspace_id is present:
   a. Look up in workspace_mappings HashMap
   b. If found → use mapped channel_id
   c. If not found → log warning, session operates without Slack channel
2. If only channel_id is present:
   a. Log deprecation warning
   b. Use channel_id directly
3. If both workspace_id and channel_id are present:
   a. workspace_id takes precedence
   b. Log deprecation warning for channel_id
4. If neither is present:
   a. Session operates without Slack channel (local-only mode)
```

### MCP.json Migration

**Before (deprecated)**:
```json
{
  "servers": {
    "intercom": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp?channel_id=C0123FRONTEND"
    }
  }
}
```

**After (preferred)**:
```json
{
  "servers": {
    "intercom": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp?workspace_id=agent-intercom"
    }
  }
}
```

## In-Memory Representation

```rust
/// Workspace-to-channel mapping loaded from config.toml.
pub struct WorkspaceMappings {
    /// Maps workspace_id → channel_id
    mappings: HashMap<String, String>,
}

impl WorkspaceMappings {
    /// Resolve a workspace_id to its configured Slack channel.
    pub fn resolve(&self, workspace_id: &str) -> Option<&str>;

    /// Check if a workspace_id has a configured mapping.
    pub fn contains(&self, workspace_id: &str) -> bool;
}
```

## Hot-Reload Behavior

- The `notify` file watcher (existing `PolicyWatcher` pattern) watches `config.toml`
- On file change, the workspace mappings section is re-parsed
- The new mappings replace the old `Arc<RwLock<WorkspaceMappings>>`
- Active sessions are **not** affected — they retain their channel_id from connection time
- Only new connections use the updated mappings

## Validation Rules

| Rule | Error |
|------|-------|
| Workspace `id` is empty | `AppError::Config("workspace id must not be empty")` |
| Workspace `id` contains invalid characters | `AppError::Config("workspace id must be alphanumeric, hyphens, or underscores")` |
| Duplicate workspace `id` | `AppError::Config("duplicate workspace id: {id}")` |
| `channel_id` is empty for a workspace entry | `AppError::Config("channel_id must not be empty for workspace: {id}")` |

## Test Contract

1. **resolve known workspace** — `workspace_id=agent-intercom` → channel `C0123FRONTEND`
2. **resolve unknown workspace** — `workspace_id=unknown` → `None`
3. **backward compat channel_id** — only `channel_id=C123` → channel `C123`
4. **precedence** — both `workspace_id` and `channel_id` → workspace mapping wins
5. **duplicate detection** — two entries with same `id` → config parse error
6. **hot-reload** — change mapping, reload → new sessions use updated mapping
7. **empty workspace section** — no `[[workspace]]` entries → all sessions local-only unless `channel_id` provided

<!-- SECTION:DESCRIPTION:END -->
