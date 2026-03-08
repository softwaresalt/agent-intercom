# Feature Specification: ACP Correctness Fixes and Mobile Operator Accessibility

**Feature Branch**: `007-acp-correctness-mobile`
**Created**: 2026-03-08
**Status**: Draft
**Input**: ACP Correctness Fixes and Mobile Input Accessibility: Fix 6 targeted ACP correctness issues identified in adversarial review, plus research and conditionally implement mobile-accessible alternatives to Slack modal dialogs for operator input on iOS.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Reliable Operator Steering Delivery (Priority: P1)

An operator is guiding an active ACP agent session by sending steering instructions through Slack. The operator expects that if a steering message fails to reach the agent, it will remain queued and be retried automatically rather than silently disappearing. Operators should never lose steering input due to a transient delivery error.

**Why this priority**: Silent loss of steering instructions is a data integrity failure. Operators directing long-running autonomous agents rely on every instruction reaching the agent — a silently dropped instruction can cause the agent to take incorrect actions with no way for the operator to know their guidance was never received.

**Independent Test**: Can be fully tested by simulating a steering delivery failure and verifying the message remains available in the queue, then succeeds on the next delivery attempt.

**Acceptance Scenarios**:

1. **Given** a queued steering message for an active session, **When** the message is successfully delivered to the agent, **Then** the message is marked as consumed and removed from the queue.
2. **Given** a queued steering message for an active session, **When** delivery fails due to a transient error, **Then** the message remains in the queue with its unconsumed status preserved and is eligible for retry on the next delivery cycle.
3. **Given** multiple queued steering messages, **When** one message fails delivery, **Then** only that message remains unconsumed; successfully delivered messages are correctly marked consumed.

---

### User Story 2 — Accurate ACP Session Capacity Enforcement (Priority: P1)

An operator attempts to start a new ACP agent session when the configured maximum number of concurrent sessions is already active or being initialized. The system must accurately count all sessions (including those that are in the process of starting) against the configured limit and reject new requests when the limit is reached, preventing resource exhaustion.

**Why this priority**: Incorrect capacity counting allows more sessions to start than the system is configured to handle, causing resource exhaustion, degraded performance, and unpredictable behavior in a server that manages multiple simultaneous autonomous agents. Each ACP session quota must apply only to ACP sessions, not to unrelated connection types.

**Independent Test**: Can be fully tested by starting sessions up to the configured maximum and verifying that attempts to start additional sessions are rejected with a clear capacity error, including when sessions are in the process of being established.

**Acceptance Scenarios**:

1. **Given** the ACP session limit is set to N, **When** exactly N active or initializing ACP sessions exist, **Then** attempts to start an additional ACP session are rejected with a capacity-exceeded message.
2. **Given** a mix of ACP and non-ACP connections, **When** the capacity check runs, **Then** only ACP sessions count toward the ACP session limit.
3. **Given** a session that is in the `created` (initializing) state, **When** the capacity check runs, **Then** the initializing session is included in the session count.
4. **Given** a session is terminated, **When** the capacity check runs afterward, **Then** the slot is available for a new session.

---

### User Story 3 — Live Workspace Routing for New ACP Sessions (Priority: P1)

When an operator starts a new ACP agent session from Slack, the system determines which workspace directory the agent should operate in based on the current channel-to-workspace mappings. These mappings are updated by the server administrator without restarting the server. The system must always use the current, live mappings rather than the snapshot loaded at startup.

**Why this priority**: Using stale workspace mappings causes sessions to operate in the wrong directory, which can cause agents to read and modify files in an unintended workspace. This is a data safety issue with potential for accidental data corruption in adjacent projects.

**Independent Test**: Can be fully tested by updating the workspace mapping configuration, starting a new ACP session without restarting the server, and verifying the session uses the updated mapping.

**Acceptance Scenarios**:

1. **Given** a channel-to-workspace mapping is updated in the configuration, **When** an operator starts a new ACP session in that channel, **Then** the session is initialized with the workspace root from the updated mapping, not the startup snapshot.
2. **Given** no mapping exists for a channel, **When** an operator starts a new ACP session in that channel, **Then** the system falls back to the default workspace root from global configuration.
3. **Given** a mapping is removed from the configuration, **When** a session start is attempted in the now-unmapped channel, **Then** the session falls back to the global default workspace.

---

### User Story 4 — Mobile Operator Approval Workflow (Priority: P2)

An operator is away from their desktop and receives a Slack notification that an ACP agent is requesting approval for a file operation or needs guidance to continue. The operator opens Slack on their iOS device and needs to approve, reject, or provide instructions. The approval and prompt response flows must work completely on mobile, including any input required from the operator.

**Why this priority**: The primary value proposition of agent-intercom is enabling remote operator control of autonomous agents. If the operator cannot respond to agent requests from a mobile device, they are effectively unable to oversee running agents while away from a desktop, which defeats the remote management scenario.

**Independent Test**: Can be fully tested by triggering an ACP clearance request and a continuation prompt, then responding to both using only the Slack iOS app — including any text input required for a "Refine" response.

**Acceptance Scenarios**:

1. **Given** an ACP agent sends a clearance request, **When** the operator views the approval message on Slack iOS, **Then** the operator can tap Accept or Reject and the agent receives the decision.
2. **Given** an ACP agent sends a continuation prompt, **When** the operator views the prompt message on Slack iOS, **Then** the operator can tap Continue or Stop and the agent receives the decision.
3. **Given** an ACP agent sends a continuation prompt requiring operator guidance text, **When** the operator taps Refine on Slack iOS, **Then** the operator can provide text input and the agent receives the guidance.
4. **Given** a text input interaction that uses Slack modal dialogs, **When** the interaction is triggered on Slack iOS and modals are not supported, **Then** the system automatically falls back to a thread-reply input mechanism that is fully functional on mobile.
5. **Given** a thread-reply fallback is active, **When** the operator replies in the session thread with their guidance, **Then** the system detects the reply, routes it to the waiting agent interaction, and confirms receipt in the thread.

---

### User Story 5 — Protocol Hygiene and Connection Safety (Priority: P2)

Server administrators and connected agents operate with confidence that the system provides clear warnings when ambiguous or conflicting configuration is detected, and that internal identifiers are reliably unique so that messages are never misrouted between concurrent agent sessions.

**Why this priority**: Silent ambiguity in connection configuration leads to unpredictable routing behavior that is difficult to diagnose. Identifier collisions cause agent messages to be delivered to the wrong session or silently lost, which is a correctness failure in a system managing concurrent autonomous agents.

**Independent Test**: Can be fully tested by providing conflicting connection parameters and verifying a deprecation warning is returned, and by running concurrent sessions and verifying prompt correlation IDs never collide.

**Acceptance Scenarios**:

1. **Given** a connection request provides both a workspace identifier and a channel identifier as parameters, **When** the connection is established, **Then** the system logs a deprecation warning indicating the ambiguous configuration and documents which parameter takes precedence.
2. **Given** multiple concurrent ACP sessions are active, **When** each session exchanges prompt messages with the server, **Then** no two sessions share a prompt correlation identifier, and responses are always delivered to the correct session.
3. **Given** the server restarts and ACP sessions reconnect, **When** prompt exchanges resume, **Then** new correlation IDs do not collide with IDs that may have been used in the previous server instance.

---

### Edge Cases

- What happens when a steering message is retried but the session has since terminated?
- What happens when the workspace mapping configuration file is temporarily unavailable during a session start?
- What happens if a thread-reply fallback is active and the operator sends multiple replies before the system processes the first?
- What happens when the mobile fallback is triggered but the session thread has been deleted or archived in Slack?
- What happens when capacity is at the limit but a session transitions from `created` to `active` concurrently with a new session start request?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST only mark a steering message as consumed after it has been successfully delivered to the target agent session.
- **FR-002**: System MUST preserve unconsumed steering messages in the queue when delivery fails, making them available for subsequent delivery attempts.
- **FR-003**: System MUST count sessions in the `created` (initializing) state against the ACP concurrent session limit.
- **FR-004**: System MUST apply the ACP session limit only to ACP protocol sessions, not to MCP or other connection types.
- **FR-005**: System MUST resolve the workspace root for a new ACP session from the current live workspace mapping configuration, not from the startup snapshot.
- **FR-006**: System MUST fall back to the global default workspace root when no channel-to-workspace mapping matches the starting session's channel.
- **FR-007**: System MUST emit a deprecation warning when a connection request supplies both workspace and channel identifier parameters simultaneously.
- **FR-008**: System MUST generate prompt correlation identifiers that are unique across all concurrent sessions and across server restarts.
- **FR-009**: System MUST research and document whether Slack modal dialogs function correctly on the iOS Slack client, specifically for the `plain_text_input` element used in operator input flows.
- **FR-010**: System MUST provide a non-modal input mechanism for all operator interactions that currently require text input, activated when modal dialogs are unavailable or when the mobile client surface is detected.
- **FR-011**: System MUST detect operator replies in the session thread and route the reply content to the waiting agent interaction when the thread-reply fallback is active.
- **FR-012**: System MUST confirm receipt of a thread-reply input by posting an acknowledgment in the session thread.
- **FR-013**: System MUST apply the thread-reply fallback for both MCP and ACP operator input flows where text input is required.

### Key Entities

- **SteeringMessage**: A queued operator instruction targeted at a specific agent session; has a consumed flag that must only be set on successful delivery.
- **SessionCapacity**: The configured maximum number of concurrent ACP sessions; enforced against all ACP sessions regardless of state (created, active).
- **WorkspaceMapping**: The live channel-to-workspace-root mapping, hot-reloaded from configuration; the authoritative source for session workspace resolution.
- **PromptCorrelationId**: A unique identifier assigned to each agent-server prompt exchange; must be globally unique across sessions and across server restarts.
- **ThreadReplyInput**: An operator's free-text reply posted in a Slack session thread in response to an input request, used as a mobile-compatible alternative to modal dialog input.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Zero steering messages are silently lost due to a delivery failure — every failed delivery leaves the message in a retryable state.
- **SC-002**: Session capacity enforcement is accurate within one session: starting sessions at or above the limit always results in rejection; starting below the limit always succeeds (absent other failures).
- **SC-003**: New ACP sessions always operate in the workspace directory that corresponds to the live configuration at session-start time, not the configuration at server-start time.
- **SC-004**: All operator approval and prompt interactions on the feature are completable end-to-end using only the Slack iOS client.
- **SC-005**: Prompt correlation identifiers are unique across 10,000 concurrent simulated prompt exchanges with zero collisions.
- **SC-006**: Ambiguous dual-parameter connections always produce a detectable deprecation warning in server logs.
- **SC-007**: All existing 996+ automated tests continue to pass after changes; new tests are added covering each corrected behavior and the mobile fallback path.

## Assumptions

- F-09 (AcpDriver deregister_session resource leak) has already been fixed in commit `b402824` and is excluded from this feature's scope.
- The mobile accessibility track (FR-009 through FR-013) is conditioned on F-15 research findings: if Slack modals fully work on iOS, only FR-009 is required; FR-010 through FR-013 are implemented only if research confirms modal input is broken on mobile.
- The thread-reply fallback (FR-010 through FR-013) uses the existing Slack event handler infrastructure; no new Slack API scopes are required beyond those already granted.
- Operator authentication for thread replies uses the existing authorized-user-ids guard already in place for all Slack event handlers.
- Prompt correlation IDs use UUIDs rather than sequential counters to guarantee uniqueness across restarts without shared state.

