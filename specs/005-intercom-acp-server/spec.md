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
2. **Given** the server is started with `--mode acp`, **When** the startup sequence completes, **Then** the server initiates an outbound connection to the configured agent endpoint instead of listening for inbound MCP connections.
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
- What happens when a Slack thread exceeds Slack's reply limit? The system posts a continuation message as a new top-level message in the channel, linking back to the original thread, and updates the session's `thread_ts`.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support a `--mode` startup flag accepting `mcp` (default) and `acp` values to select the agent communication protocol.
- **FR-002**: In MCP mode, the system MUST behave identically to the current implementation — no regressions in existing MCP functionality.
- **FR-003**: In ACP mode, the system MUST initiate an outbound connection to a configured agent endpoint and send an initial prompt.
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
