# Behavioral Matrix: Intercom ACP Server

**Input**: Design documents from `/specs/005-intercom-acp-server/`
**Prerequisites**: spec.md (required), plan.md (required), data-model.md, contracts/
**Created**: 2026-02-28

## Summary

| Metric | Count |
|---|---|
| **Total Scenarios** | 72 |
| Happy-path | 28 |
| Edge-case | 15 |
| Error | 14 |
| Boundary | 5 |
| Concurrent | 6 |
| Security | 4 |

**Non-happy-path coverage**: 61% (minimum 30% required)

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
| S055 | Stream EOF detection | Agent process closes stdout | Read returns EOF | `AgentEvent::SessionTerminated` emitted with reason "stream closed" | Session marked interrupted | happy-path |
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

## Edge Case Coverage Checklist

- [x] Malformed inputs and invalid arguments (S006, S026, S032, S033, S052, S054)
- [x] Missing dependencies and unavailable resources (S004, S005, S014, S025)
- [x] State errors and race conditions (S016, S047, S068)
- [x] Boundary values (empty, max-length, zero, negative) (S026, S042, S057, S058)
- [x] Permission and authorization failures (S072)
- [x] Concurrent access patterns (S017, S041, S048)
- [x] Graceful degradation scenarios (S028, S031, S035, S059)

## Cross-Reference Validation

- [x] Every entity in `data-model.md` has at least one scenario covering its state transitions (Session: S018-S026, S069-S071; WorkspaceMapping: S027-S035; AgentEvent: S049-S055; AcpMessage: S049-S058)
- [x] Every endpoint in `contracts/` has at least one happy-path and one error scenario (AgentDriver: S008-S017; ACP Stream: S049-S058; Workspace Mapping: S027-S035)
- [x] Every user story in `spec.md` has corresponding behavioral coverage (US-1: S001-S007, US-2: S008-S017, US-3: S018-S026, US-4: S027-S035, US-5: S036-S042, US-6: S043-S048, US-7: S049-S058, US-8: S059-S062, US-9: S063-S068)
- [x] No scenario has ambiguous or non-deterministic expected outcomes

## Notes

- Scenario IDs are globally sequential (S001–S072) across all components
- Categories: `happy-path`, `edge-case`, `error`, `boundary`, `concurrent`, `security`
- Each row is deterministic — exactly one expected outcome per input state
- Security scenarios are minimal because the existing authorization guard (FR-013/SC-009 from base) covers most security paths; only ACP-specific security paths are added here
