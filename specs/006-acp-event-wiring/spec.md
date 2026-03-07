# Feature Specification: ACP Event Handler Wiring

**Feature Branch**: `006-acp-event-wiring`
**Created**: 2026-03-07
**Status**: Draft
**Input**: Wire the ACP event consumer's ClearanceRequested and PromptForwarded handlers to register with AcpDriver, persist to the DB, and post Slack interactive messages.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Operator Approves ACP File Operation (Priority: P1)

An ACP agent session is running and the agent needs operator approval before modifying a file. The agent emits a clearance request. The operator sees an approval message appear in the session's Slack thread containing the file path, risk level, and a diff of the proposed change. The operator reviews the details and taps "Accept" or "Reject." The agent receives the decision and either applies the change or abandons it.

**Why this priority**: Without this, ACP agents requesting approval hang indefinitely. This is the primary blocker for ACP-based file operations that require operator oversight — the core value proposition of agent-intercom in ACP mode.

**Independent Test**: Can be fully tested by starting an ACP session, triggering a clearance request from the agent, observing the Slack message, clicking Accept, and verifying the agent receives the approval response.

**Acceptance Scenarios**:

1. **Given** an active ACP session with a connected Slack channel, **When** the agent emits a `ClearanceRequested` event, **Then** an approval request is persisted to the database, registered with the ACP driver, and an interactive approval message is posted to the session's Slack thread.
2. **Given** a pending ACP clearance request displayed in Slack, **When** the operator taps "Accept," **Then** the system resolves the clearance through the ACP driver, the agent receives an "approved" response, and the approval record is updated in the database.
3. **Given** a pending ACP clearance request displayed in Slack, **When** the operator taps "Reject," **Then** the system resolves the clearance through the ACP driver, the agent receives a "rejected" response, and the approval record is updated in the database.
4. **Given** a pending ACP clearance request, **When** the configured approval timeout elapses without operator action, **Then** the system treats the request as expired and the approval record is updated accordingly.

---

### User Story 2 — Operator Responds to ACP Continuation Prompt (Priority: P1)

An ACP agent session is running and the agent needs operator input to continue — for example, to clarify requirements, recover from an error, or decide on next steps. The agent emits a prompt forwarding event. The operator sees a prompt message in the session's Slack thread describing the agent's question and offering response options (Continue, Refine, Stop). The operator selects an option and optionally provides additional instructions. The agent receives the decision and acts accordingly.

**Why this priority**: Continuation prompts are the second half of the ACP human-in-the-loop interaction model. Without this, agents that need operator guidance hang indefinitely, making interactive ACP workflows non-functional.

**Independent Test**: Can be fully tested by starting an ACP session, triggering a prompt forwarding event, observing the Slack message, clicking a response button, and verifying the agent receives the operator's decision.

**Acceptance Scenarios**:

1. **Given** an active ACP session with a connected Slack channel, **When** the agent emits a `PromptForwarded` event, **Then** a prompt record is persisted to the database, registered with the ACP driver, and an interactive prompt message is posted to the session's Slack thread.
2. **Given** a pending ACP prompt displayed in Slack, **When** the operator taps "Continue," **Then** the system resolves the prompt through the ACP driver and the agent receives a "continue" decision.
3. **Given** a pending ACP prompt displayed in Slack, **When** the operator taps "Refine" and provides additional instructions, **Then** the system resolves the prompt with the operator's instructions and the agent receives both the decision and the instruction text.
4. **Given** a pending ACP prompt displayed in Slack, **When** the operator taps "Stop," **Then** the system resolves the prompt through the ACP driver and the agent receives a "stop" decision.

---

### User Story 3 — Session Thread Continuity (Priority: P2)

When an ACP agent's first interaction with the operator is a clearance request or prompt, and no Slack thread yet exists for the session, the system creates the thread by posting the message directly (not via the background queue) and records the resulting message timestamp as the session's thread anchor. All subsequent messages for that session appear in the same thread.

**Why this priority**: Thread continuity is essential for operator usability when managing multiple concurrent sessions. Without it, approval and prompt messages scatter across the channel instead of grouping under the session thread.

**Independent Test**: Can be tested by starting an ACP session that has no prior Slack thread, triggering a clearance request, verifying the message creates a new thread, then triggering a second event and verifying it appears in the same thread.

**Acceptance Scenarios**:

1. **Given** an ACP session with no existing Slack thread, **When** the first clearance request or prompt is posted, **Then** the message is posted directly (not queued) and the returned message timestamp is saved as the session's thread anchor.
2. **Given** an ACP session with an existing Slack thread, **When** a clearance request or prompt is posted, **Then** the message appears as a reply in the existing session thread.

---

### Edge Cases

- What happens when the session referenced in a `ClearanceRequested` event no longer exists in the database? The system must log a warning and discard the event without crashing.
- What happens when Slack is not configured or temporarily unavailable? The system must still persist the request to the database and register it with the driver, but skip the Slack notification (logging a warning).
- What happens when two clearance requests arrive for the same ACP session in rapid succession? Each must be independently tracked, persisted, and displayed as separate Slack messages.
- What happens when the operator responds to a clearance request after the ACP session has already terminated? The resolution attempt must handle the missing session writer gracefully (the driver already returns an error for unknown sessions).
- What happens when a `PromptForwarded` event arrives with an unknown `prompt_type` value? The system must treat it as a generic continuation prompt and display it with a default label.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST register each incoming `ClearanceRequested` event with the ACP driver's pending clearance map, associating the request ID with the session ID.
- **FR-002**: System MUST persist each incoming `ClearanceRequested` event as an approval request record in the database, capturing title, description, diff, file path, and risk level.
- **FR-003**: System MUST post an interactive approval message to the session's Slack thread when a `ClearanceRequested` event is received, containing the file path, risk level indicator, diff content, and Accept/Reject action buttons.
- **FR-004**: System MUST register each incoming `PromptForwarded` event with the ACP driver's pending prompt map, associating the prompt ID with the session ID.
- **FR-005**: System MUST persist each incoming `PromptForwarded` event as a continuation prompt record in the database, capturing prompt text and prompt type.
- **FR-006**: System MUST post an interactive prompt message to the session's Slack thread when a `PromptForwarded` event is received, containing the prompt text, prompt type label, and Continue/Refine/Stop action buttons.
- **FR-007**: System MUST use direct message posting (not the background queue) for clearance requests so the returned Slack message timestamp can be captured and stored for threading.
- **FR-008**: System MUST use the session's existing Slack thread timestamp when posting messages, and if no thread exists, record the first message's timestamp as the session's thread anchor.
- **FR-009**: System MUST gracefully handle missing sessions — when the session ID from an event cannot be found in the database, the system logs a warning and discards the event.
- **FR-010**: System MUST gracefully handle Slack unavailability — when Slack is not configured, the system still persists the record and registers the request but skips posting.
- **FR-011**: System MUST parse the `risk_level` string from the event into the appropriate risk classification for display and persistence.
- **FR-012**: System MUST parse the `prompt_type` string from the event into the appropriate prompt type for display and persistence, defaulting to "continuation" for unrecognized values.
- **FR-013**: System MUST compute a content hash of the target file for clearance requests (when the file exists within the workspace) to enable conflict detection during later diff application.

### Key Entities

- **Approval Request**: Represents a pending file operation clearance. Attributes: unique ID, session ID, title, description, diff content, file path, risk level, approval status, content hash, Slack message timestamp, creation timestamp.
- **Continuation Prompt**: Represents a pending operator decision point. Attributes: unique ID, session ID, prompt text, prompt type, elapsed seconds, actions taken, operator decision, instruction text, Slack message timestamp, creation timestamp.

## Assumptions

- The existing Slack button handlers for approval responses (Accept/Reject) and prompt responses (Continue/Refine/Stop) already route decisions through the ACP driver's `resolve_clearance` and `resolve_prompt` methods. This feature only needs to wire the *inbound* side (event → registration + persistence + Slack post); the *outbound* side (Slack button → driver resolution → agent response) is already functional.
- The `AcpDriver` methods `register_clearance` and `register_prompt_request` are already implemented and tested. This feature calls them from a new location (the event consumer) rather than reimplementing them.
- The `ApprovalRepo::create` and `PromptRepo::create` database methods are already implemented and tested. This feature calls them from the event consumer.
- The `build_approval_blocks` and `build_prompt_blocks` Slack message builders are already implemented. This feature reuses them for constructing ACP event messages.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: ACP agents that emit clearance requests receive an operator decision (approved or rejected) within the configured approval timeout period, without hanging indefinitely.
- **SC-002**: ACP agents that emit continuation prompts receive an operator decision (continue, refine, or stop) within the configured prompt timeout period, without hanging indefinitely.
- **SC-003**: 100% of ACP clearance requests and continuation prompts are persisted to the database, creating an auditable record of all human-in-the-loop interactions.
- **SC-004**: Operators see ACP clearance and prompt messages in the correct session thread in Slack, maintaining per-session conversation grouping.
- **SC-005**: The system handles rapid successive events (two or more clearance/prompt events within 1 second) without data loss, race conditions, or duplicate records.
