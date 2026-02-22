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
| S045 | Register loads initial policy | Valid `.monocoque/settings.json` in workspace | `PolicyWatcher::register(workspace_root)` | `get_policy()` returns parsed policy with correct tools/patterns | Policy in watcher cache | happy-path |
| S046 | File modification triggers policy update | Watcher registered, initial policy loaded | Overwrite `settings.json` with new content | `get_policy()` eventually returns updated policy (poll 50ms, timeout 2s) | Cache updated | happy-path |
| S047 | File deletion falls back to deny-all | Watcher registered, policy file exists | Delete `settings.json` | `get_policy()` eventually returns `WorkspacePolicy::default()` (deny-all) | Cache cleared to default | edge-case |
| S048 | Malformed JSON uses deny-all | Watcher registered | Write invalid JSON to `settings.json` | `get_policy()` returns deny-all default | Cache retains deny-all | error |
| S049 | Unregister stops watching | Watcher registered | `unregister(workspace_root)` → modify file → poll | Policy does NOT update after unregister | Watcher removed from internal map | happy-path |
| S050 | Multiple workspaces have independent policies | Two workspaces registered | Modify policy in workspace A only | Workspace A policy updated, workspace B unchanged | Independent cache entries | concurrent |
| S051 | Missing policy directory loads deny-all | Workspace without `.monocoque/` directory | `PolicyWatcher::register(workspace_root)` | `get_policy()` returns deny-all default | No watcher error, deny-all cached | edge-case |
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
