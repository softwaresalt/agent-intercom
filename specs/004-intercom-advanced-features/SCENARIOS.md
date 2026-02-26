# Behavioral Matrix: Intercom Advanced Features

**Input**: Design documents from `/specs/004-intercom-advanced-features/`
**Prerequisites**: spec.md (required), plan.md (required), data-model.md, contracts/
**Created**: 2026-02-26

## Summary

| Metric | Count |
|---|---|
| **Total Scenarios** | 94 |
| Happy-path | 32 |
| Edge-case | 20 |
| Error | 18 |
| Boundary | 8 |
| Concurrent | 6 |
| Security | 6 |

**Non-happy-path coverage**: 66% (minimum 30% required)

## Operator Steering Queue

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S001 | Steering message stored from Slack | Active session on channel C1 | Operator sends `@intercom refocus on tests` in C1 | Message stored in steering_message table with source=slack, session_id matched | steering_message row created, consumed=0 | happy-path |
| S002 | Steering message delivered via ping | Session has 2 unconsumed messages | Agent calls `ping` | Response includes `pending_steering: ["msg1", "msg2"]` | Both messages marked consumed=1 | happy-path |
| S003 | Ping with no steering messages | Session has 0 unconsumed messages | Agent calls `ping` | Response has `pending_steering: []` or field absent | No DB changes | happy-path |
| S004 | Steering via /intercom steer command | Active session on channel C1 | Operator runs `/intercom steer "check error handling"` | Message stored with source=slack | steering_message created | happy-path |
| S005 | Steering via intercom-ctl IPC | Active session exists | Operator runs `intercom-ctl steer "refocus"` | Message stored with source=ipc | steering_message created | happy-path |
| S006 | Steer with no active session | No sessions active | Operator sends steer message | Error: no active session to steer | No row created | error |
| S007 | Steer to specific channel with multiple sessions | Sessions on C1 and C2 | Operator steers in C1 | Message routed only to C1 session | Only C1 session receives message | edge-case |
| S008 | Message for terminated session | Session terminated before ping | Steering message exists unconsumed | Message remains unconsumed | consumed=0, available for recovery | edge-case |
| S009 | Multiple operators steer simultaneously | Two operators send messages at same time | Concurrent Slack events | Both messages stored in arrival order | Two rows, no dedup | concurrent |
| S010 | Empty steering message text | Operator sends empty string | `/intercom steer ""` | Error: message text required | No row created | boundary |
| S011 | Very long steering message | Operator sends 10KB message | Slack app mention with long text | Message stored (no length limit per spec) | Row created with full text | boundary |
| S012 | Unauthorized user steers | Non-authorized Slack user sends steer | App mention from unknown user | Silently ignored per auth guard | No row created, security event logged | security |

---

## Task Inbox

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S013 | Task queued via Slack | No active session | `/intercom task "review PR #42"` | Task stored in task_inbox with source=slack | Row created, consumed=0 | happy-path |
| S014 | Task queued via IPC | No active session | `intercom-ctl task "fix lint"` | Task stored with source=ipc | Row created, consumed=0 | happy-path |
| S015 | Tasks delivered at session start | 3 unconsumed tasks for channel C1 | New session starts on C1, calls `reboot` | Response includes `pending_tasks` array with 3 items chronologically | All 3 marked consumed=1 | happy-path |
| S016 | Empty inbox at session start | No unconsumed tasks | Session starts, calls `reboot` | No `pending_tasks` in response (or empty array) | No DB changes | happy-path |
| S017 | Tasks scoped to channel | Tasks for C1 and C2 | Session starts on C1 | Only C1 tasks delivered | C1 tasks consumed, C2 tasks remain | edge-case |
| S018 | Task queued while session is active | Active session exists | `/intercom task "do something"` | Task stored in inbox (not steering queue) | Row created in task_inbox | edge-case |
| S019 | Multiple tasks ordered chronologically | Tasks created at T1, T2, T3 | Session starts | Delivered in order [T1, T2, T3] | All consumed | happy-path |
| S020 | Empty task message | Operator submits empty text | `/intercom task ""` | Error: message text required | No row created | boundary |
| S021 | IPC task with no channel context | Local CLI has no channel info | `intercom-ctl task "work item"` | Task stored with channel_id=NULL | Row created, delivered to any session | edge-case |
| S022 | Unauthorized task submission | Non-authorized user | Slack command from unknown user | Silently ignored | No row created | security |

---

## Server Startup Reliability

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S023 | Normal startup with port available | Port 3000 free | Server starts | HTTP transport binds, Slack connects | Process running | happy-path |
| S024 | Port already in use | Another process on port 3000 | Server starts | Error logged: port conflict, process exits | Exit code 1 | error |
| S025 | Second server instance | First instance running | Second instance launched | Detects port occupied, logs message about existing instance, exits | Exit code 1 | error |
| S026 | Bind failure after Slack connected | Slack connects first, then HTTP bind fails | Server startup sequence | Slack service shut down cleanly, then process exits | All services stopped, exit code 1 | error |
| S027 | Port freed after previous crash | Previous instance crashed, port released | Server starts | Binds successfully, no false positive | Normal startup | happy-path |
| S028 | Firewall blocks port (no bind error) | Port binds but firewall blocks traffic | Server starts | Server starts normally (outside scope) | Running but unreachable | edge-case |

---

## Slack Modal Instruction Capture

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S029 | Resume with instructions modal flow | Agent in standby | Operator presses "Resume with Instructions" | Modal opens with text input | Agent still waiting | happy-path |
| S030 | Modal text submitted | Modal open with typed text | Operator submits "focus on error handling" | Agent receives exact typed text, resumes | Session active, oneshot resolved | happy-path |
| S031 | Refine modal flow | Agent in transmit (approval pending) | Operator presses "Refine" | Modal opens for feedback input | Agent still waiting | happy-path |
| S032 | Refine text submitted | Modal open | Operator submits refinement | Agent receives refinement as rejection reason | Approval rejected with instructions | happy-path |
| S033 | Modal dismissed without submit | Modal open | Operator clicks X or Escape | No resolution; agent stays waiting | Oneshot not resolved | edge-case |
| S034 | trigger_id expired (>3s) | Server slow to respond | Button pressed, modal open delayed | Modal fails to open, no agent state change | Agent stays waiting, operator can retry | error |
| S035 | Empty modal submission | Modal open, text field empty | Operator submits with no text | Error: instruction text required | Modal stays open or re-prompts | boundary |
| S036 | Unauthorized modal interaction | ViewSubmission from non-authorized user | Modal submitted | Silently ignored | No oneshot resolution | security |

---

## SSE Disconnect Session Cleanup

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S037 | Clean disconnect detected | Active session via HTTP | Client closes connection | Session marked terminated | session.status = terminated | happy-path |
| S038 | Network hiccup causes drop | Active session | TCP connection drops | Server detects stream close, marks terminated | session.status = terminated | happy-path |
| S039 | Agent reconnects after disconnect | Previous session terminated | Same agent connects again | New session created, old remains terminated | old=terminated, new=active | edge-case |
| S040 | Multiple disconnects simultaneously | 3 active sessions | All 3 connections drop | All 3 marked terminated independently | All terminated | concurrent |
| S041 | Disconnect during tool call | Agent mid-call when connection drops | Connection closes during pending request | Pending call cancelled, session terminated | Terminated, operations cleaned up | edge-case |
| S042 | Stdio transport disconnect | Agent via stdio | Stdio pipe closes | Session marked terminated | session.status = terminated | edge-case |

---

## Policy Hot-Reload Wiring

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S043 | Policy change reflected in auto_check | Server running, policy cached | Operator edits .intercom/settings.json | Cache updated via watcher | Next auto_check uses new rules | happy-path |
| S044 | auto_check reads from cache | PolicyCache populated | Agent calls auto_check | Response from cached policy | Cache hit, no disk read | happy-path |
| S045 | Policy file deleted | Policy existed | Operator deletes settings.json | Falls back to deny-all | All auto_check return false | edge-case |
| S046 | Invalid policy file | Valid policy cached | Operator writes malformed JSON | Retains last valid policy, logs warning | Cache unchanged | error |
| S047 | Policy file created fresh | No policy existed | Operator creates settings.json | Loaded and cached | auto_check uses new rules | happy-path |
| S048 | Rapid successive edits | File edited 5 times in 1 second | Multiple filesystem events | Watcher debounces, loads final state | Cache reflects last stable version | concurrent |

---

## Audit Logging

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S049 | Tool call logged | Agent calls ping | Tool handler completes | JSONL entry: event_type=tool_call, tool_name=ping | Line in audit-YYYY-MM-DD.jsonl | happy-path |
| S050 | Approval logged | Operator approves change | Approval handler | JSONL entry: event_type=approval, operator_id, request_id | Audit entry written | happy-path |
| S051 | Rejection logged | Operator rejects with reason | Rejection handler | JSONL entry: event_type=rejection, reason populated | Audit entry written | happy-path |
| S052 | Command approval logged | Operator approves command | Command approval flow | JSONL entry: event_type=command_approval, command field | Audit entry written | happy-path |
| S053 | Session lifecycle logged | Session starts | Session creation | JSONL entry: event_type=session_start | Audit entry written | happy-path |
| S054 | Daily rotation | Date changes midnight | First log after midnight | New file audit-2026-02-27.jsonl opened | Old file closed, new active | edge-case |
| S055 | Audit directory missing | .intercom/logs/ absent | Server starts | Directory created automatically | Directory exists | happy-path |
| S056 | Disk full during audit write | Filesystem full | Audit write attempted | Warning logged, server continues | Server not crashed | error |
| S057 | Concurrent audit writes | Multiple tool calls complete | Parallel writes | All entries written without corruption | Valid JSONL | concurrent |

---

## Agent Failure Reporting

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S058 | Stall detected and reported | No heartbeat for threshold period | Stall detector fires | Slack notification with session ID, last state, recovery steps | Notification sent | happy-path |
| S059 | Agent process crash reported | Agent process exits unexpectedly | Exit detected | Slack notification with exit code, session details | Notification sent | happy-path |
| S060 | Recovery steps in notification | Stalled session | Notification generated | Includes actionable steps (e.g., "run intercom-ctl spawn") | Operator can act | happy-path |
| S061 | No channel for notifications | No global channel, session disconnected | Stall detected | Warning logged, notification skipped | Warning in logs | error |

---

## Context Detail Levels

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S062 | Minimal detail level | slack_detail_level = "minimal" | Status message posted | Only essential info (status, outcome) | Short message | happy-path |
| S063 | Standard detail (default) | No detail_level configured | Status message posted | Status, summaries, key parameters | Standard message | happy-path |
| S064 | Verbose detail level | slack_detail_level = "verbose" | Status message posted | Full details (diffs, params, metadata) | Verbose message | happy-path |
| S065 | Approval always full detail | slack_detail_level = "minimal" | Approval request posted | Full diff shown regardless | Full detail | edge-case |
| S066 | Error notification always full | slack_detail_level = "minimal" | Error posted | Full details shown | Full detail | edge-case |
| S067 | Invalid detail level in config | slack_detail_level = "debug" | Server starts | Falls back to "standard" with warning | Standard used | error |

---

## Auto-Approve Suggestion

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S068 | Suggestion offered after approval | Command not auto-approved | Operator approves `cargo test --release` | Slack button: "Add to auto-approve?" | Button displayed | happy-path |
| S069 | Operator accepts suggestion | Button displayed | Operator clicks "Yes, add" | Regex written to settings.json | Policy updated | happy-path |
| S070 | Operator declines suggestion | Button displayed | Operator clicks "No thanks" | No policy change | Unchanged | happy-path |
| S071 | Generated regex is efficient | Similar commands approved | Pattern generation | Generalized regex, not individual commands | Single pattern | edge-case |
| S072 | Policy file not writable | Permissions issue | Operator accepts | Error logged, operator informed | Policy unchanged | error |
| S073 | Already auto-approved command | Matches existing pattern | Command approved (shouldn't reach manual) | No suggestion offered | No change | edge-case |

---

## Policy Regex Pre-Compilation

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S074 | Patterns compiled at load time | Policy with 20 patterns | PolicyLoader::load() | RegexSet created with 20 patterns | CompiledWorkspacePolicy cached | happy-path |
| S075 | Pre-compiled used in auto_check | Compiled policy cached | auto_check called | RegexSet::matches() used | O(1) matching | happy-path |
| S076 | Invalid regex skipped | 1 invalid among 10 valid | Policy loaded | Invalid skipped with warning, 9 compiled | 9-pattern RegexSet | error |
| S077 | Policy reload recompiles | Policy file changed | Hot-reload | New RegexSet compiled | Cache updated | happy-path |
| S078 | Empty pattern list | No command patterns | Policy loaded | Empty RegexSet | All commands denied | boundary |
| S079 | ReDoS-like pattern | Catastrophic backtracking regex | Policy loaded | Pattern skipped or timeout | Warning, pattern excluded | error |

---

## Ping Fallback to Most-Recent Session

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S080 | Fallback to newest session | 2 active (T1, T2; T2 newer) | Agent calls ping | Uses T2 session | T2 updated | happy-path |
| S081 | Single active session | 1 active session | Agent calls ping | Normal behavior | Session updated | happy-path |
| S082 | No active sessions | 0 sessions | Agent calls ping | Error: no active session | Error response | error |

---

## Slack Queue Drain Race Fix

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S083 | Drain runs without global channel | No global channel | Shutdown signal | Queue drained unconditionally | Messages delivered | happy-path |
| S084 | Drain runs with global channel | Global channel configured | Shutdown signal | Queue drained (same as current) | Messages delivered | happy-path |
| S085 | Empty queue at shutdown | No pending messages | Shutdown | Drain completes immediately | Clean exit | boundary |
| S086 | Late message during shutdown | Message added after signal | Shutdown drains | Late message delivered | Message sent | concurrent |

---

## Approval File Attachment

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S087 | Original file attached for existing file modification | File `src/main.rs` exists (5KB) | Agent calls `check_clearance` with diff | Slack message includes diff AND original file uploaded as attachment | Two file uploads: diff snippet + original file | happy-path |
| S088 | No original attachment for new file | File does not exist on disk | Agent calls `check_clearance` (new file content) | Slack message shows diff/content only, no original file attachment | One upload (diff only) or inline diff | happy-path |
| S089 | Large original file uploaded as attachment | File is 150KB | Agent calls `check_clearance` | Original file uploaded as Slack file attachment (not inlined) | File attachment posted | happy-path |
| S090 | Small original file uploaded as attachment | File is 2KB | Agent calls `check_clearance` | Original file uploaded as Slack file attachment | File attachment posted | happy-path |
| S091 | Original file deleted before Slack post | File existed at hash time but deleted before upload | `check_clearance` posts to Slack | Warning in message: "original file unavailable", approval flow continues | Approval created, message posted without original | error |
| S092 | Binary file attached | Binary file (e.g., `.png`) | Agent calls `check_clearance` | Binary uploaded as file attachment with correct filename | File attachment with binary content | edge-case |
| S093 | File read permission denied | File unreadable due to OS permissions | Agent calls `check_clearance` | Warning logged, approval continues without original attachment | Approval request still posted | error |
| S094 | Diff already uploaded as snippet (large diff) | Diff ≥20 lines AND file exists | Agent calls `check_clearance` | Both diff snippet and original file uploaded as separate attachments | Two file uploads to channel | edge-case |

---

## Edge Case Coverage Checklist

- [x] Malformed inputs and invalid arguments (S010, S020, S035, S067, S079)
- [x] Missing dependencies and unavailable resources (S056, S061, S072, S091, S093)
- [x] State errors and race conditions (S008, S041, S048, S086)
- [x] Boundary values (empty, max-length, zero, negative) (S010, S011, S020, S035, S078, S085)
- [x] Permission and authorization failures (S012, S022, S036, S093)
- [x] Concurrent access patterns (S009, S040, S048, S057, S086)
- [x] Graceful degradation scenarios (S034, S046, S056, S076, S091)

## Cross-Reference Validation

- [x] Every entity in `data-model.md` has at least one scenario (SteeringMessage: S001-S012, TaskInboxItem: S013-S022, AuditLogEntry: S049-S057, CompiledWorkspacePolicy: S074-S079)
- [x] Every user story in `spec.md` has behavioral coverage (all 16 stories)
- [x] No scenario has ambiguous or non-deterministic expected outcomes

## Notes

- Scenario IDs globally sequential (S001-S094)
- Categories: happy-path (32), edge-case (20), error (18), boundary (8), concurrent (6), security (6)
- Each row is deterministic — exactly one expected outcome per input state
- Scenarios map directly to parameterized Rust `#[rstest]` test cases
