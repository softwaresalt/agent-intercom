# Feature Specification: Integration Test Full Coverage

**Feature Branch**: `001-002-integration-test`  
**Created**: 2026-02-22  
**Status**: Draft  
**Input**: User description: "Spec out the integration test plan for this server to achieve full coverage of the current functionality and implement all tests, then execute all tests to achieve a full test pass for insight into actual functionality being production ready."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Call Tool End-to-End Dispatch (Priority: P1)

A developer invokes any of the 9 MCP tools through the `call_tool()` handler entry point and receives a well-formed JSON response. This validates the full dispatch path: JSON argument parsing, session resolution, stall timer reset, tool-router matching, handler execution, and response construction.

**Why this priority**: `call_tool()` is the single entry point for all agent interactions. Every other feature depends on it working correctly end-to-end.

**Independent Test**: Can be tested by constructing a `CallToolRequestParam` with valid arguments, invoking `call_tool()` on an `AgentRcServer` with an active in-memory session, and asserting the response JSON matches expected shape and content.

**Acceptance Scenarios**:

1. **Given** an active session exists, **When** `call_tool("heartbeat", {status_message: "working"})` is dispatched, **Then** the response contains `{acknowledged: true}` and `last_activity` is updated.
2. **Given** an active session exists, **When** `call_tool("set_operational_mode", {mode: "hybrid"})` is dispatched, **Then** the session mode is updated in the database and the response confirms the mode change.
3. **Given** an active session exists, **When** `call_tool("remote_log", {message: "test", level: "info"})` is dispatched without Slack, **Then** the response gracefully indicates no Slack channel and `last_tool` is updated.
4. **Given** an active session exists, **When** `call_tool("recover_state", {})` is dispatched, **Then** a well-formed recovery payload is returned.
5. **Given** an active session and workspace policy file, **When** `call_tool("check_auto_approve", {tool_name: "echo"})` is dispatched, **Then** the auto-approve evaluation result is returned.
6. **Given** no active session exists, **When** `call_tool("heartbeat", {})` is dispatched, **Then** an appropriate error is returned indicating no active session.
7. **Given** an active session exists, **When** `call_tool("unknown_tool", {})` is dispatched, **Then** the router returns a "tool not implemented" error.

---

### User Story 2 - HTTP/SSE Transport Health & Routing (Priority: P1)

A client connects to the HTTP/SSE server and verifies the transport layer is functioning: the health endpoint responds, SSE connections are accepted with query-parameter extraction, and the server can handle concurrent connection attempts.

**Why this priority**: The SSE transport is the primary production interface. If it doesn't start, bind, route, or accept connections correctly, no agent can communicate.

**Independent Test**: Can be tested by spawning the axum server on an ephemeral port, making HTTP requests, and verifying responses.

**Acceptance Scenarios**:

1. **Given** the SSE server is running, **When** `GET /health` is requested, **Then** a 200 OK response with body "ok" is returned.
2. **Given** the SSE server is running, **When** a request is made to a non-existent path, **Then** a 404 response is returned.
3. **Given** the SSE server is running, **When** `GET /sse?channel_id=C_WORKSPACE&session_id=S_123` is requested, **Then** the channel and session overrides are extracted and applied to the created `AgentRcServer`.

---

### User Story 3 - Session Manager Orchestration (Priority: P1)

A session progresses through its full lifecycle via the orchestrator: creation, activation, pause, resume, and termination. The orchestrator enforces state-machine transitions and concurrent-session limits.

**Why this priority**: Session management is the backbone of multi-agent orchestration. Incorrect transitions or missing enforcement would corrupt server state.

**Independent Test**: Can be tested by calling `session_manager` functions against an in-memory database and verifying state transitions, error cases, and boundary conditions.

**Acceptance Scenarios**:

1. **Given** an active session, **When** `pause_session()` is called, **Then** session status becomes Paused.
2. **Given** a paused session, **When** `resume_session()` is called, **Then** session status becomes Active.
3. **Given** an active session, **When** `terminate_session()` is called, **Then** session status becomes Terminated.
4. **Given** the maximum concurrent sessions limit is reached, **When** a new session is spawned, **Then** the spawn fails with an appropriate error.
5. **Given** a terminated session, **When** `resume_session()` is called, **Then** an invalid-transition error is returned.

---

### User Story 4 - Checkpoint Create & Restore with Divergence Detection (Priority: P2)

A developer creates a checkpoint that captures file hashes of the workspace, then mutates files, and restores the checkpoint. The system detects modified, deleted, and added files.

**Why this priority**: Checkpoints enable safe rollback. Divergence detection prevents silent data loss. This is critical for operator confidence.

**Independent Test**: Can be tested with a temp directory containing known files, creating a checkpoint, mutating the filesystem, and verifying divergence entries.

**Acceptance Scenarios**:

1. **Given** a workspace with files A and B, **When** a checkpoint is created, **Then** the checkpoint contains SHA-256 hashes for both files.
2. **Given** a checkpoint exists, **When** file A is modified, **Then** `restore_checkpoint()` reports a `Modified` divergence for file A.
3. **Given** a checkpoint exists, **When** file B is deleted, **Then** `restore_checkpoint()` reports a `Deleted` divergence for file B.
4. **Given** a checkpoint exists, **When** a new file C is added, **Then** `restore_checkpoint()` reports an `Added` divergence for file C.
5. **Given** a checkpoint exists and no files changed, **When** restoring, **Then** zero divergence entries are returned.

---

### User Story 5 - Stall Detector Escalation Flow (Priority: P2)

The stall detector monitors session inactivity. When no tool calls arrive within the threshold, it fires events through Stalled → AutoNudge → Escalated. Reset events interrupt the escalation chain.

**Why this priority**: Stall detection prevents abandoned agents from consuming resources. The escalation chain is the primary automated recovery mechanism.

**Independent Test**: Can be tested by configuring short thresholds, monitoring the event channel, and verifying the event sequence.

**Acceptance Scenarios**:

1. **Given** stall detection is enabled with a 1-second threshold, **When** no activity occurs for 1 second, **Then** a `Stalled` event is emitted.
2. **Given** a stall has occurred, **When** the auto-nudge fires, **Then** an `AutoNudge` event is emitted and `nudge_count` increments.
3. **Given** max retries (3) are exhausted, **When** another nudge would fire, **Then** an `Escalated` event is emitted.
4. **Given** a stall has been detected, **When** `handle.reset()` is called, **Then** a `SelfRecovered` event is emitted and the timer restarts.
5. **Given** stall detection is active, **When** `handle.pause()` is called, **Then** no stall events fire until `handle.resume()`.

---

### User Story 6 - Policy Hot-Reload via File Watcher (Priority: P2)

Workspace auto-approve policies in `.agentrc/settings.json` are loaded and hot-reloaded when the file changes. The evaluator applies the latest policy to tool calls.

**Why this priority**: Hot-reload enables operators to adjust policy without restarting the server. This is important for iterative trust calibration.

**Independent Test**: Can be tested by writing a policy file, registering a watcher, modifying the file, and verifying the evaluator reflects the new policy.

**Acceptance Scenarios**:

1. **Given** a valid policy file exists, **When** `PolicyLoader::load()` is called, **Then** the policy is parsed correctly.
2. **Given** a policy watcher is registered, **When** the policy file is modified, **Then** the evaluator applies the updated policy.
3. **Given** a policy watcher is registered, **When** the policy file is deleted, **Then** the evaluator falls back to deny-all.
4. **Given** a malformed policy file, **When** it is loaded, **Then** the evaluator uses deny-all default.

---

### User Story 7 - IPC Server Command Dispatch (Priority: P3)

The `monocoque-ctl` CLI connects via IPC (named pipe) and executes commands: list sessions, approve/reject requests, resume sessions, and change modes. Auth token validation is enforced.

**Why this priority**: IPC is a secondary interface for local operator control. Important but not on the critical path for agent communication.

**Independent Test**: Can be tested by starting the IPC server, connecting a client, sending JSON-line commands, and verifying responses and auth enforcement.

**Acceptance Scenarios**:

1. **Given** the IPC server is running with an auth token, **When** a command with a valid token is sent, **Then** the command executes successfully.
2. **Given** the IPC server is running with an auth token, **When** a command with an invalid token is sent, **Then** an unauthorized error is returned.
3. **Given** the IPC server is running, **When** a `list` command is sent, **Then** active sessions are returned.
4. **Given** an active session with a pending approval, **When** an `approve` command is sent via IPC, **Then** the approval is resolved.

---

### User Story 8 - Graceful Shutdown Sequence (Priority: P3)

When the server receives a shutdown signal, it marks all pending approvals, prompts, and waits as Interrupted, posts a Slack recovery summary (if available), and terminates within the timeout.

**Why this priority**: Clean shutdown prevents data corruption and enables recovery on restart. Important for production reliability.

**Independent Test**: Can be tested by creating pending approvals/prompts/sessions, triggering the shutdown sequence, and verifying all entities are marked Interrupted.

**Acceptance Scenarios**:

1. **Given** pending approvals and prompts exist, **When** shutdown is triggered, **Then** all pending entities are marked as Interrupted in the database.
2. **Given** active sessions exist, **When** shutdown is triggered, **Then** sessions are marked as Interrupted.
3. **Given** the shutdown sequence completes, **When** the server restarts, **Then** `check_interrupted_on_startup()` finds the interrupted entities.

---

### User Story 9 - Startup Recovery Flow (Priority: P3)

On server startup, `check_interrupted_on_startup()` scans for sessions that were interrupted by a previous crash or shutdown, and posts a recovery summary.

**Why this priority**: Recovery ensures operators are aware of interrupted work and can resume. This is the complement to graceful shutdown.

**Independent Test**: Can be tested by pre-populating the database with interrupted sessions and invoking the recovery check.

**Acceptance Scenarios**:

1. **Given** interrupted sessions exist in the database, **When** `check_interrupted_on_startup()` runs, **Then** the interrupted sessions are found and reported.
2. **Given** no interrupted sessions exist, **When** `check_interrupted_on_startup()` runs, **Then** no recovery action is taken.

---

### Edge Cases

- What happens when `call_tool` receives malformed JSON arguments?
- How does the system handle a tool call when the database connection is lost?
- What happens when two agents attempt concurrent tool calls on the same session?
- How does `accept_diff` behave when the target file is locked by another process?
- What happens when the IPC named pipe path already exists from a previous run?
- How does the stall detector handle rapid reset/pause/resume cycles?
- What happens when `PolicyWatcher` cannot read the policy directory (permissions)?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Test suite MUST exercise `call_tool()` dispatch for all 9 tools (heartbeat, set_operational_mode, remote_log, recover_state, check_auto_approve, ask_approval, accept_diff, forward_prompt, wait_for_instruction) through the `ServerHandler` trait.
- **FR-002**: Test suite MUST verify the HTTP/SSE health endpoint returns 200 OK.
- **FR-003**: Test suite MUST exercise the session manager orchestrator functions (`pause_session`, `resume_session`, `terminate_session`) including invalid transition rejection.
- **FR-004**: Test suite MUST verify checkpoint creation captures file hashes and restore detects Modified, Deleted, and Added divergences.
- **FR-005**: Test suite MUST verify stall detector escalation: Stalled → AutoNudge → Escalated event sequence.
- **FR-006**: Test suite MUST verify stall detector reset, pause, and resume behaviours.
- **FR-007**: Test suite MUST verify policy hot-reload updates the evaluator when `.agentrc/settings.json` changes.
- **FR-008**: Test suite MUST verify IPC server auth token enforcement (valid/invalid/missing).
- **FR-009**: Test suite MUST verify graceful shutdown marks pending entities as Interrupted.
- **FR-010**: Test suite MUST verify startup recovery scan finds interrupted sessions.
- **FR-011**: All new integration tests MUST use in-memory SQLite and not require external services (no real Slack, no real IPC connections unless testing IPC specifically).
- **FR-012**: Test suite MUST pass `cargo test` with zero test failures.
- **FR-013**: Test suite MUST pass `cargo clippy -- -D warnings` with zero warnings.

### Key Entities

- **Session**: Core lifecycle entity with state machine (Created → Active → Paused/Terminated/Interrupted)
- **ApprovalRequest**: Pending code change with status transitions (Pending → Approved/Rejected/Expired → Consumed)
- **ContinuationPrompt**: Operator decision point with Continue/Refine/Stop outcomes
- **Checkpoint**: Workspace snapshot with file hashes for divergence detection
- **StallAlert**: Escalation tracking with nudge counts
- **WorkspacePolicy**: Auto-approve rules with tool/command/file-pattern matching

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All existing tests (unit, contract, integration) continue to pass — zero regressions.
- **SC-002**: New integration tests cover `call_tool()` dispatch for all 9 MCP tools.
- **SC-003**: New integration tests cover HTTP health endpoint verification.
- **SC-004**: New integration tests cover session manager orchestrator functions.
- **SC-005**: New integration tests cover checkpoint create/restore with divergence detection.
- **SC-006**: New integration tests cover stall detector escalation chain.
- **SC-007**: New integration tests cover policy hot-reload.
- **SC-008**: Full `cargo test` passes with zero failures.
- **SC-009**: Full `cargo clippy -- -D warnings` passes with zero warnings.

## Assumptions

- Tests will use in-memory SQLite for isolation and speed, consistent with existing test infrastructure.
- Slack interactions will be tested at the database and oneshot-channel level, not with a real Slack API, since the existing test suite already omits Slack client.
- IPC tests may be scoped to unit-level if named-pipe creation is unreliable in CI environments.
- The stall detector escalation tests will use short timeouts (100-500ms) to keep test execution fast.
- Policy hot-reload tests will use `tempfile` directories and short polling intervals.
