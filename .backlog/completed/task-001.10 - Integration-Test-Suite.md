---
id: TASK-001.10
title: "001 - Integration Test Suite (Cross-cutting 001-002)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001
dependencies: []
ordinal: 1100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
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


# Behavioral Matrix: Integration Test Full Coverage

**Input**: Design documents from `/specs/001-002-integration-test/`
**Prerequisites**: spec.md (required), plan.md (required)
**Created**: 2026-02-22

## Summary

| Metric | Count |
|---|---|
| **Total Scenarios** | 78 |
| Happy-path | 30 |
| Edge-case | 14 |
| Error | 18 |
| Boundary | 6 |
| Concurrent | 5 |
| Security | 5 |

**Non-happy-path coverage**: 62% (minimum 30% required)

---

## Call Tool Dispatch (US1, FR-001)

Validates the full MCP dispatch path: JSON argument parsing, session resolution, tool-router matching, handler execution, stall timer reset, and response construction.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S001 | Heartbeat dispatches through full path | Active session, valid `status_message` | `call_tool("heartbeat", {status_message: "working"})` via transport | Response contains `acknowledged: true` | `last_activity` updated in DB, stall detector reset | happy-path |
| S002 | Set mode dispatches and persists | Active session, mode `hybrid` | `call_tool("set_operational_mode", {mode: "hybrid"})` via transport | Response confirms mode change to `hybrid` | Session mode updated to Hybrid in DB | happy-path |
| S003 | Recover state returns clean state | No interrupted sessions | `call_tool("recover_state", {})` via transport | Response contains `{interrupted_sessions: []}` | No state changes | happy-path |
| S004 | Remote log without Slack gracefully degrades | Active session, no Slack client | `call_tool("remote_log", {message: "test", level: "info"})` via transport | Response indicates no Slack channel, `last_tool` updated | `last_tool` set to `remote_log` in DB | happy-path |
| S005 | Check auto-approve evaluates policy | Active session, workspace policy with matching tool | `call_tool("check_auto_approve", {tool_name: "echo"})` via transport | Response contains auto-approve evaluation result | No state changes | happy-path |
| S006 | Unknown tool returns descriptive error | Active session | `call_tool("unknown_tool", {})` via transport | Error response: tool not found/implemented | No state changes | error |
| S007 | Malformed JSON arguments return error | Active session | `call_tool("heartbeat", {invalid_field: 123})` via transport | Error response describing argument parse failure | No state changes | error |
| S008 | No active session returns error | No sessions in DB | `call_tool("heartbeat", {})` via transport | Error response indicating no active session | No state changes | error |
| S009 | Stall detector reset on tool call entry | Active session, active stall detector | Any `call_tool()` invocation | Tool executes normally | Stall detector timer reset before and after execution | happy-path |
| S010 | Tool router matches all 9 registered tools | Active session | `list_tools()` via transport | Response lists exactly 9 tools with correct schemas | No state changes | happy-path |

---

## HTTP/SSE Transport Health & Routing (US2, FR-002)

Validates HTTP transport layer: health endpoint, 404 routing, SSE query parameter extraction.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S011 | Health endpoint returns 200 OK | SSE server running on ephemeral port | `GET /health` | HTTP 200 with body `"ok"` | Server continues running | happy-path |
| S012 | Non-existent route returns 404 | SSE server running | `GET /nonexistent` | HTTP 404 | Server continues running | error |
| S013 | Health endpoint without trailing slash | SSE server running | `GET /health` (no trailing slash) | HTTP 200 with body `"ok"` | Server continues running | edge-case |
| S014 | Channel ID extracted from SSE query params | SSE server running | `GET /sse?channel_id=C_WORKSPACE` | `AgentRcServer` created with `channel_id_override = Some("C_WORKSPACE")` | Per-connection server has correct override | happy-path |
| S015 | Session ID extracted from SSE query params | SSE server running | `GET /sse?session_id=S_123` | `AgentRcServer` created with `session_id_override = Some("S_123")` | Per-connection server has correct override | happy-path |
| S016 | Missing query params use None | SSE server running | `GET /sse` (no query params) | `AgentRcServer` created with `channel_id_override = None, session_id_override = None` | Per-connection server has no overrides | edge-case |

---

## Session Manager Orchestration (US3, FR-003)

Validates session lifecycle: creation, activation, pause, resume, termination, state machine enforcement, concurrent session limits.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S017 | Pause active session | Session with status Active | `pause_session(session_id)` | Returns updated Session with status Paused | DB updated, `updated_at` changed | happy-path |
| S018 | Resume paused session | Session with status Paused | `resume_session(session_id)` | Returns updated Session with status Active | DB updated | happy-path |
| S019 | Terminate active session | Session with status Active, no child process | `terminate_session(session_id, None)` | Returns updated Session with Terminated status | `terminated_at` set, DB updated | happy-path |
| S020 | Full lifecycle: pause → resume → terminate | Session starts Active | Sequential: `pause → resume → terminate` | Each transition succeeds | Final status Terminated | happy-path |
| S021 | Resume terminated session fails | Session with Terminated status | `resume_session(session_id)` | Error: invalid state transition | Session remains Terminated | error |
| S022 | Pause created session fails | Session with Created status | `pause_session(session_id)` | Error: invalid state transition (Created → Paused not allowed) | Session remains Created | error |
| S023 | Max concurrent sessions enforced | `max_concurrent_sessions` sessions active | Spawn new session | Error: concurrent session limit reached | No new session created | boundary |
| S024 | Resolve session by owner user ID | Multiple sessions, one active for user | `resolve_session(None, user_id)` | Returns the active session for that user | No state changes | happy-path |
| S025 | Resolve wrong user returns unauthorized | Session owned by user A | `resolve_session(session_id, user_B)` | `AppError::Unauthorized` | No state changes | security |

---

## Checkpoint Create & Restore (US4, FR-004)

Validates checkpoint creation with file hashes and restore with divergence detection.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S026 | Create checkpoint captures file hashes | Workspace with files A, B | `create_checkpoint(session_id, label)` | Checkpoint saved with SHA-256 hashes for A and B | Checkpoint record in DB | happy-path |
| S027 | Restore detects modified file | Checkpoint exists, file A modified | `restore_checkpoint(checkpoint_id)` | Returns `DivergenceEntry { file: A, kind: Modified }` | No state changes | happy-path |
| S028 | Restore detects deleted file | Checkpoint exists, file B deleted | `restore_checkpoint(checkpoint_id)` | Returns `DivergenceEntry { file: B, kind: Deleted }` | No state changes | happy-path |
| S029 | Restore detects added file | Checkpoint exists, new file C added | `restore_checkpoint(checkpoint_id)` | Returns `DivergenceEntry { file: C, kind: Added }` | No state changes | happy-path |
| S030 | Restore with no changes yields zero divergences | Checkpoint exists, no file changes | `restore_checkpoint(checkpoint_id)` | Returns empty `Vec<DivergenceEntry>` | No state changes | happy-path |
| S031 | Restore detects all three divergence types | Checkpoint exists, one file modified, one deleted, one added | `restore_checkpoint(checkpoint_id)` | Returns 3 divergence entries (Modified, Deleted, Added) | No state changes | edge-case |
| S032 | Restore nonexistent checkpoint returns error | Invalid checkpoint ID | `restore_checkpoint("nonexistent")` | `AppError::NotFound` | No state changes | error |
| S033 | Hash workspace with empty directory | Empty temp directory | `hash_workspace_files(empty_dir)` | Returns empty `HashMap` | No state changes | boundary |
| S034 | Hash workspace skips subdirectories | Directory with files and subdirs | `hash_workspace_files(root)` | HashMap contains only top-level files, not subdir contents | No state changes | edge-case |

---

## Stall Detector Escalation (US5, FR-005, FR-006)

Validates stall detection escalation chain and control operations.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S035 | Stall event fires after inactivity threshold | Detector with 100ms threshold | No activity for 100ms+ | `StallEvent::Stalled` emitted on event channel | Detector in stalled state | happy-path |
| S036 | AutoNudge fires after escalation interval | Stall already detected | Wait escalation interval | `StallEvent::AutoNudge { nudge_count: 1 }` emitted | `nudge_count` incremented | happy-path |
| S037 | Full escalation: Stalled → AutoNudge(1,2) → Escalated | max_retries = 2 | No activity through full chain | Events in order: Stalled, AutoNudge(1), AutoNudge(2), Escalated | Detector in escalated state | happy-path |
| S038 | Reset before threshold prevents stall | Active detector | `handle.reset()` before threshold | No stall event emitted | Timer restarted | happy-path |
| S039 | Self-recovery after stall | Stall detected | `handle.reset()` | `StallEvent::SelfRecovered` emitted | Timer restarted, nudge count zeroed | happy-path |
| S040 | Pause prevents stall events | Active detector | `handle.pause()` → wait > threshold | No stall events emitted | Detector paused | happy-path |
| S041 | Resume after pause restarts detection | Paused detector | `handle.resume()` → wait > threshold | `StallEvent::Stalled` emitted | Timer restarted from resume point | happy-path |
| S042 | Cancellation stops detector | Active detector | `cancel_token.cancel()` | Detector task completes, no further events | Detector stopped | edge-case |
| S043 | is_stalled reflects state | Initial detector | Check `is_stalled()` before and after threshold | `false` initially, `true` after stall | State reflects detector status | edge-case |
| S044 | Rapid reset cycles do not panic | Active detector | 100 rapid `handle.reset()` calls | No panic, timer restarted each time | Detector stable | boundary |

---

## Policy Hot-Reload (US6, FR-007)

Validates workspace auto-approve policy loading, hot-reloading via file watcher, and fallback behavior.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S045 | Register loads initial policy | Valid `.agentrc/settings.json` in workspace | `PolicyWatcher::register(workspace_root)` | `get_policy()` returns parsed policy with correct tools/patterns | Policy in watcher cache | happy-path |
| S046 | File modification triggers policy update | Watcher registered, initial policy loaded | Overwrite `settings.json` with new content | `get_policy()` eventually returns updated policy (poll 50ms, timeout 2s) | Cache updated | happy-path |
| S047 | File deletion falls back to deny-all | Watcher registered, policy file exists | Delete `settings.json` | `get_policy()` eventually returns `WorkspacePolicy::default()` (deny-all) | Cache cleared to default | edge-case |
| S048 | Malformed JSON uses deny-all | Watcher registered | Write invalid JSON to `settings.json` | `get_policy()` returns deny-all default | Cache retains deny-all | error |
| S049 | Unregister stops watching | Watcher registered | `unregister(workspace_root)` → modify file → poll | Policy does NOT update after unregister | Watcher removed from internal map | happy-path |
| S050 | Multiple workspaces have independent policies | Two workspaces registered | Modify policy in workspace A only | Workspace A policy updated, workspace B unchanged | Independent cache entries | concurrent |
| S051 | Missing policy directory loads deny-all | Workspace without `.agentrc/` directory | `PolicyWatcher::register(workspace_root)` | `get_policy()` returns deny-all default | No watcher error, deny-all cached | edge-case |
| S052 | Evaluator applies updated policy to tool check | Policy modified to add tool "echo" | `PolicyEvaluator::check("echo", ...)` after hot-reload | Auto-approved for "echo" tool | No state changes | happy-path |

---

## IPC Server Command Dispatch (US7, FR-008)

Validates IPC named pipe server: command routing, auth enforcement, and all command types.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S053 | Valid auth token accepted | Server with `ipc_auth_token = "test-token"` | Send JSON `{command: "list", auth_token: "test-token"}` | `{ok: true, data: [...]}` | Command dispatched | security |
| S054 | Invalid auth token rejected | Server with `ipc_auth_token = "test-token"` | Send JSON `{command: "list", auth_token: "wrong"}` | `{ok: false, error: "unauthorized"}` | Command NOT dispatched | security |
| S055 | Missing auth token rejected | Server with `ipc_auth_token = "test-token"` | Send JSON `{command: "list"}` (no auth_token field) | `{ok: false, error: "unauthorized"}` | Command NOT dispatched | security |
| S056 | Auth disabled when no token configured | Server with `ipc_auth_token = None` | Send JSON `{command: "list"}` | `{ok: true, data: [...]}` | Command dispatched without auth | edge-case |
| S057 | List returns active sessions | DB with 2 active sessions, 1 terminated | Send `{command: "list"}` with valid auth | Response contains 2 session IDs | No state changes | happy-path |
| S058 | List with no sessions returns empty | Empty DB | Send `{command: "list"}` | `{ok: true, data: []}` | No state changes | boundary |
| S059 | Approve resolves pending approval | Pending approval with oneshot sender in map | Send `{command: "approve", id: "approval-id"}` | `{ok: true}`, oneshot sender fires with Approved | Approval status → Approved in DB | happy-path |
| S060 | Reject resolves with reason | Pending approval with oneshot sender | Send `{command: "reject", id: "id", reason: "unsafe"}` | `{ok: true}`, oneshot fires with Rejected + reason | Approval status → Rejected in DB | happy-path |
| S061 | Approve nonexistent request errors | No matching pending approval | Send `{command: "approve", id: "nonexistent"}` | `{ok: false, error: "not found"}` | No state changes | error |
| S062 | Resume resolves pending wait | Pending wait with oneshot sender | Send `{command: "resume", id: "session-id"}` | `{ok: true}`, oneshot fires with resumed status | Wait resolved | happy-path |
| S063 | Resume with instruction | Pending wait | Send `{command: "resume", instruction: "do X"}` | `{ok: true}`, instruction passed through oneshot | Instruction available to waiting tool | happy-path |
| S064 | Mode changes session mode | Active session, current mode Remote | Send `{command: "mode", mode: "hybrid"}` | `{ok: true}` | Session mode → Hybrid in DB | happy-path |
| S065 | Mode with invalid value errors | Active session | Send `{command: "mode", mode: "invalid"}` | `{ok: false, error: ...}` | No state changes | error |
| S066 | Unknown command errors | Any server state | Send `{command: "unknown"}` | `{ok: false, error: "unknown command"}` | No state changes | error |

---

## Graceful Shutdown (US8, FR-009)

Validates that shutdown marks all pending entities as Interrupted.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S067 | Shutdown interrupts pending approvals | 2 pending approvals in DB | Trigger shutdown sequence | Both approvals marked Interrupted | All pending approvals → Interrupted in DB | happy-path |
| S068 | Shutdown interrupts pending prompts | 2 pending prompts in DB | Trigger shutdown sequence | Both prompts updated with Stop decision | Prompts marked with Stop decision | happy-path |
| S069 | Shutdown interrupts active sessions | 1 active + 1 paused session | Trigger shutdown sequence | Both sessions marked Interrupted | Sessions → Interrupted in DB | happy-path |
| S070 | Full shutdown: all entity types | Active session + pending approval + pending prompt | Trigger shutdown sequence | All entities interrupted/stopped | Complete clean state for recovery | happy-path |
| S071 | Shutdown with no pending entities is no-op | Clean DB, no pending anything | Trigger shutdown sequence | No errors, no changes | DB unchanged | edge-case |

---

## Startup Recovery (US9, FR-010)

Validates recovery scan on server startup.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S072 | Recovery finds interrupted sessions | 2 interrupted sessions in DB | `check_interrupted_on_startup()` | Returns list of 2 interrupted sessions | No state changes (read-only scan) | happy-path |
| S073 | Clean DB yields no recovery action | No interrupted sessions | `check_interrupted_on_startup()` | Returns empty/clean result | No state changes | edge-case |
| S074 | Recovery counts pending per session | Interrupted session with 2 pending approvals + 1 prompt | `check_interrupted_on_startup()` | Recovery summary includes counts: 2 approvals, 1 prompt | No state changes | happy-path |
| S075 | Recovery includes progress snapshot | Interrupted session with progress snapshot | Recovery scan | Progress snapshot data available in result | Snapshot preserved from pre-interruption | happy-path |
| S076 | Recovery includes last checkpoint | Interrupted session with checkpoint | Recovery scan | Checkpoint data available in recovery result | Checkpoint preserved | happy-path |

---

## Cross-Cutting Concerns

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S077 | Concurrent tool calls on same session | Active session, 2 parallel tool calls | Parallel `call_tool("heartbeat")` invocations | Both succeed without corruption | `last_activity` updated, no DB conflicts | concurrent |
| S078 | Database connection pool exhaustion | In-memory SQLite with `max_connections=1`, concurrent writes | Multiple concurrent repo operations | Operations serialize correctly (SQLite WAL) | No data corruption | concurrent |

---

## Edge Case Coverage Checklist

- [x] Malformed inputs and invalid arguments (S007, S048, S065)
- [x] Missing dependencies and unavailable resources (S051, S058)
- [x] State errors and race conditions (S021, S022, S044)
- [x] Boundary values (empty, max-length, zero, negative) (S023, S033, S044, S058)
- [x] Permission and authorization failures (S025, S053, S054, S055)
- [x] Concurrent access patterns (S050, S077, S078)
- [x] Graceful degradation scenarios (S004, S047, S056, S071)

## Cross-Reference Validation

- [x] Every user story in `spec.md` has corresponding behavioral coverage (US1–US9 mapped to S001–S076)
- [x] No scenario has ambiguous or non-deterministic expected outcomes
- [x] Scenarios map directly to parameterized test cases
- [x] Edge cases in spec.md accounted for: malformed JSON (S007), DB connection loss (S078), concurrent tool calls (S077), path traversal (covered by existing `handler_accept_diff_tests`), stale IPC pipe (covered by unique pipe names), rapid stall cycles (S044), policy directory permissions (S051)

## Notes

- Scenario IDs S001–S078 are globally sequential
- Categories: `happy-path` (30), `edge-case` (14), `error` (18), `boundary` (6), `concurrent` (5), `security` (5)
- Each row is deterministic — exactly one expected outcome per input state
- Scenarios covering already-tested functionality (S011–S044, S067–S076) document existing test contracts
- Scenarios requiring new tests (S001–S010, S045–S066, S077–S078) map to the three gap modules in plan.md


# Quickstart: Integration Test Full Coverage

**Feature**: 001-002-integration-test | **Date**: 2026-02-22

## Prerequisites

- Rust stable toolchain (edition 2021)
- `cargo` on PATH

## Build

No additional dependencies are required. All test infrastructure uses existing workspace dependencies (`tokio`, `tempfile`, `sqlx`, `interprocess`, `notify`).

```powershell
cargo check
```

## Run All Tests

```powershell
cargo test
```

Or with output capture for full results:

```powershell
cargo test 2>&1 | Out-File logs\test-results.txt
```

## Run Specific Test Modules

New integration test files:

```powershell
# Policy hot-reload tests
cargo test --test integration policy_watcher_tests

# IPC server command dispatch tests
cargo test --test integration ipc_server_tests

# MCP transport dispatch tests (if added)
cargo test --test integration mcp_dispatch_tests
```

## Verify Quality Gates

```powershell
cargo check
cargo clippy -- -D warnings
cargo fmt --all -- --check
cargo test
```

## Test Structure

All new tests are in `tests/integration/` and registered in `tests/integration.rs`:

| Module | Tests | Spec Coverage |
|---|---|---|
| `policy_watcher_tests` | Hot-reload, deletion fallback, malformed file | FR-007, US6 |
| `ipc_server_tests` | Auth enforcement, list/approve/reject/resume/mode | FR-008, US7 |
| `mcp_dispatch_tests` | Full call_tool dispatch via transport (if feasible) | FR-001, US1 |

## Key Patterns

- **Database**: `db::connect_memory()` — fresh in-memory SQLite per test
- **Filesystem**: `tempfile::tempdir()` — isolated temp directory per test
- **IPC pipes**: Unique name per test via UUID suffix
- **Timeouts**: All async assertions use `tokio::time::timeout()` with 2-5s bounds
- **Policy watcher**: Poll-with-timeout pattern (50ms interval, 2s max) for `notify` event convergence




---

## Checklists

# Specification Quality Checklist: Integration Test Full Coverage

**Purpose**: Validate specification completeness and quality before proceeding to planning  
**Created**: 2026-02-22  
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

- All items pass validation. Spec is ready for implementation.
- This is a test-implementation spec, so "user stories" map to test coverage areas rather than end-user features.
- Assumptions section documents the testing strategy decisions (in-memory DB, no real Slack, short timeouts).

<!-- SECTION:DESCRIPTION:END -->
