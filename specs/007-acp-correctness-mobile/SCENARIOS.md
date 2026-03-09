# Behavioral Matrix: ACP Correctness Fixes and Mobile Operator Accessibility

**Input**: Design documents from `/specs/007-acp-correctness-mobile/`
**Prerequisites**: spec.md (required), plan.md (required), data-model.md
**Created**: 2026-03-08

## Summary

| Metric | Count |
|---|---|
| **Total Scenarios** | 36 |
| Happy-path | 17 |
| Edge-case | 10 |
| Error | 3 |
| Boundary | 3 |
| Concurrent | 2 |
| Security | 1 |

**Non-happy-path coverage**: 52.8% (minimum 30% required) ✅

---

## Steering Message Delivery (F-06 / FR-001, FR-002)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S001 | Successful steering delivery marks consumed | 1 unconsumed steering message queued for active session; driver `send_prompt` succeeds | `flush_queued_messages` called on reconnect | Message delivered to agent via `send_prompt`; `mark_consumed` called | `consumed = 1` in DB; `StreamActivity` event emitted | happy-path |
| S002 | Failed delivery preserves unconsumed status | 1 unconsumed steering message queued; driver `send_prompt` returns error | `flush_queued_messages` called on reconnect | Warning logged with session_id and message_id; `mark_consumed` NOT called | `consumed = 0` in DB; message eligible for retry on next reconnect | error |
| S003 | Partial failure leaves only failed messages unconsumed | 3 unconsumed messages; message #2 `send_prompt` fails; #1 and #3 succeed | `flush_queued_messages` called on reconnect | Messages #1 and #3 delivered and marked consumed; #2 logged as failed | Messages #1,#3: `consumed = 1`; Message #2: `consumed = 0` | edge-case |
| S004 | Retry on next reconnect delivers previously failed message | 1 message failed on first flush; session reconnects again | Second `flush_queued_messages` invocation | Message delivered on retry; `mark_consumed` called after success | `consumed = 1`; message fully delivered | happy-path |
| S005 | Retry when session has since terminated | 1 unconsumed message; session status = `terminated` | `flush_queued_messages` called | `send_prompt` fails (session gone); message remains unconsumed; no crash | `consumed = 0`; function returns without panic | edge-case |
| S006 | Empty queue is a no-op | No unconsumed messages for the session | `flush_queued_messages` called | Function returns immediately after query; no `send_prompt` calls | No DB changes; no events emitted | boundary |
| S007 | mark_consumed fails after successful send | `send_prompt` succeeds; `mark_consumed` returns DB error | `flush_queued_messages` delivers message then marks | Warning logged for failed mark; message may be re-delivered on next flush | `consumed = 0` in DB; agent received message (potential duplicate) | edge-case |

---

## ACP Session Capacity Enforcement (F-07 / FR-003, FR-004)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S008 | Reject ACP start at capacity with active sessions | max_sessions = 2; 2 ACP sessions in `active` state | `/arc session-start <prompt>` | Error returned: "max concurrent ACP sessions reached (2/2)" | No new session created; existing sessions unaffected | happy-path |
| S009 | Allow ACP start below capacity | max_sessions = 3; 1 ACP session in `active` state | `/arc session-start <prompt>` | Session created successfully; Slack thread posted | New session in `created` state; total ACP count = 2 | happy-path |
| S010 | Created-state sessions counted against limit | max_sessions = 2; 1 `active` + 1 `created` ACP session | `/arc session-start <prompt>` | Error returned: capacity exceeded (2/2) | No new session created; `created` session counted | happy-path |
| S011 | MCP sessions excluded from ACP count | max_sessions = 2; 3 MCP sessions active; 0 ACP sessions | `/arc session-start <prompt>` | Session created successfully | MCP sessions do not affect ACP capacity | happy-path |
| S012 | Terminated session frees capacity slot | max_sessions = 2; 2 ACP sessions; 1 transitions to `terminated` | `/arc session-start <prompt>` | Session created successfully; capacity = 2/2 after create | New session created; terminated session not counted | happy-path |
| S013 | Concurrent session starts at capacity boundary | max_sessions = 2; 1 ACP session active; 2 concurrent `/arc session-start` requests | Two simultaneous `handle_acp_session_start` calls | At most one succeeds; the other gets capacity-exceeded error | At most 2 ACP sessions exist (created + active) | concurrent |
| S014 | max_sessions = 0 rejects all ACP starts | max_sessions = 0; no existing sessions | `/arc session-start <prompt>` | Error returned: capacity exceeded (0/0) | No session created | boundary |
| S015 | Active, created, and paused states counted (LC-06) | max_sessions = 3; 1 `active` + 1 `paused` + 1 `terminated` ACP session | `/arc session-start <prompt>` | Session created; `active`, `created`, and `paused` ACP sessions counted; only `terminated`/`interrupted` excluded | New session in `created`; capacity used = 3/3 | edge-case |

---

## MCP Query Parameter Cleanup (F-10 / FR-007 — scope: remove channel_id)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S016 | workspace_id resolves channel from mapping | `workspace_id=my-repo` in URL; workspace mapping `my-repo → C_WORKSPACE` configured | MCP connection to `/mcp?workspace_id=my-repo` | Channel resolved to `C_WORKSPACE` via workspace mapping | IntercomServer created with `effective_channel = Some("C_WORKSPACE")` | happy-path |
| S017 | No workspace_id — session runs without channel | No query parameters on `/mcp` URL | MCP connection to `/mcp` | Session created without Slack channel routing | IntercomServer created with `effective_channel = None` | happy-path |
| S018 | channel_id param is silently ignored | `channel_id=C_DIRECT` in URL; no `workspace_id` | MCP connection to `/mcp?channel_id=C_DIRECT` | `channel_id` not extracted; session runs without channel | IntercomServer created with `effective_channel = None` (not `C_DIRECT`) | edge-case |
| S019 | Both params — workspace_id used, channel_id ignored | `workspace_id=my-repo&channel_id=C_IGNORED` in URL | MCP connection to `/mcp?workspace_id=my-repo&channel_id=C_IGNORED` | Channel resolved from `workspace_id` only; `channel_id` not read | IntercomServer with `effective_channel` from workspace mapping | edge-case |
| S020 | Unknown workspace_id — no channel, warning logged | `workspace_id=unknown-repo`; no matching mapping | MCP connection to `/mcp?workspace_id=unknown-repo` | Warning logged: "workspace_id not found in config"; session runs without channel | IntercomServer with `effective_channel = None` | error |
| S021 | session_id extracted from query parameters | `session_id=sess-123&workspace_id=my-repo` in URL | MCP connection to `/mcp?session_id=sess-123&workspace_id=my-repo` | Both `session_id` and `workspace_id` extracted; channel resolved | IntercomServer linked to pre-created session | happy-path |

---

## Prompt Correlation ID Uniqueness (F-13 / FR-008)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S022 | Handshake IDs use UUID format | ACP session spawn | `send_initialize()`, `send_session_new()`, `send_prompt()` in handshake | IDs match pattern `"intercom-{purpose}-{uuid}"` (e.g., `intercom-init-550e8400-...`) | Handshake completes with unique IDs | happy-path |
| S023 | Runtime prompt IDs use UUID format | Active ACP session; operator resolves clearance | `AcpDriver::resolve_clearance()` or `resolve_prompt()` | JSON-RPC `id` field is `"intercom-prompt-{uuid}"` | Response correctly routed to agent via session writer | happy-path |
| S024 | 10,000 IDs with zero collisions | Generate 10,000 correlation IDs in a loop | Call ID generation function 10,000 times | All IDs are unique; set.len() == 10,000 | Zero duplicates detected | boundary |
| S025 | Post-restart IDs don't collide with pre-restart | Generate IDs before simulated restart; generate IDs after | Two batches of ID generation with fresh UUID state | No overlap between pre- and post-restart ID sets | Zero collisions across restart boundary | edge-case |
| S026 | Two concurrent sessions have distinct IDs | 2 ACP sessions spawned simultaneously | Both sessions perform handshake concurrently | Each session's handshake IDs are globally unique | No shared IDs between sessions; both handshakes complete | concurrent |

---

## Mobile Modal Research (F-15 / FR-009)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S027 | Research document produced with findings | Slack API docs, Block Kit reference, community reports | Desk research phase | Document at `research-f15-mobile-modals.md` with one of: (a) modals work, (b) input broken, (c) modals swallowed | Research findings gate F-16/F-17 implementation decision | happy-path |

---

## Thread-Reply Input Fallback (F-16 / FR-010–FR-012 — Conditional on F-15)

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S028 | Modal opens normally on desktop | Operator on desktop client; Refine button pressed | `handle_prompt_action` with `prompt_refine` action | `views.open` succeeds; modal displayed; oneshot resolved on submission | Modal context cached; pending resolution via `handle_view_submission` | happy-path |
| S029 | Modal failure triggers thread-reply fallback | `views.open` returns error (mobile client or Slack API issue) | `handle_prompt_action` with `prompt_refine` action; `open_modal` fails | Fallback message posted in session thread: "Reply with your instructions" | `pending_thread_replies` entry created; modal context cleaned up | edge-case |
| S030 | Thread reply captured and routed to waiting interaction | Pending thread-reply for `prompt_refine:{prompt_id}`; authorized user replies | `message` event in session thread from authorized user | Reply text extracted; oneshot resolved with operator's text | Prompt record updated with `Refine` decision and instruction text | happy-path |
| S031 | Acknowledgment posted after thread reply | Operator reply captured and routed successfully | Thread-reply handler completes | "✅ Received your instructions" posted in session thread | Thread shows reply + acknowledgment; agent unblocked | happy-path |
| S032 | Multiple replies — first captured, rest ignored | Pending thread-reply; operator sends 3 replies rapidly | Three `message` events in quick succession | First reply resolves the oneshot; subsequent replies are no-ops (oneshot already consumed) | Only first reply's text delivered to agent | edge-case |
| S033 | Reply from unauthorized user rejected | Pending thread-reply; non-authorized user replies in thread | `message` event from user not in `authorized_user_ids` | Reply silently ignored (per FR-013/SC-009 authorization guard) | Pending thread-reply remains unresolved; authorized user can still reply | security |
| S034 | Archived thread — graceful error | Pending thread-reply; session thread deleted/archived | Attempt to post fallback message in thread | Slack API returns error; warning logged; oneshot resolved with timeout/error | Agent receives timeout/error response; no crash | error |
| S035 | Fallback works for both MCP and ACP prompts | MCP agent sends `transmit` prompt; ACP agent sends `PromptForwarded` | Both trigger `prompt_refine` action; both experience modal failure | Both fall back to thread-reply mechanism; both resolve correctly | Both agents receive operator instructions via thread reply | happy-path |
| S036 | Modal timeout triggers fallback | `views.open` succeeds but no submission received within timeout | Modal submission timeout (e.g., 5 minutes) | Fallback message posted in thread; modal considered abandoned | `pending_thread_replies` entry created; modal context cleaned up | edge-case |

---

## Edge Case Coverage Checklist

- [x] Malformed inputs and invalid arguments — S018 (ignored channel_id), S020 (unknown workspace_id)
- [x] Missing dependencies and unavailable resources — S005 (terminated session), S034 (archived thread)
- [x] State errors and race conditions — S013 (concurrent session starts), S007 (mark_consumed failure)
- [x] Boundary values (empty, max-length, zero, negative) — S006 (empty queue), S014 (max_sessions=0), S024 (10,000 IDs)
- [x] Permission and authorization failures — S033 (unauthorized thread reply)
- [x] Concurrent access patterns — S013 (concurrent capacity), S026 (concurrent handshakes)
- [x] Graceful degradation scenarios — S029 (modal fallback), S036 (modal timeout)

## Cross-Reference Validation

- [x] Every entity in `data-model.md` has at least one scenario covering its state transitions
  - SteeringMessage: S001 (consumed=0→1), S002 (stays 0), S003 (partial)
  - Session: S008–S015 (status + protocol_mode filtering)
  - PendingParams: S016–S021 (workspace_id only)
  - PromptCorrelationId: S022–S026 (UUID generation and uniqueness)
  - ThreadReplyInput: S028–S036 (conditional entity, all states covered)
- [x] No API contracts defined (internal changes only) — N/A
- [x] Every user story in `spec.md` has corresponding behavioral coverage
  - US1 (Steering): S001–S007
  - US2 (Capacity): S008–S015
  - US3 (Workspace): Already fixed (F-08 excluded) — no scenarios needed
  - US4 (Mobile): S027–S036
  - US5 (Protocol): S016–S026
- [x] No scenario has ambiguous or non-deterministic expected outcomes

## Notes

- Scenario IDs are globally sequential (S001–S036) across all components
- S028–S036 (Thread-Reply Fallback) are conditional on F-15 research outcome — if modals work on iOS, these scenarios are deferred
- S013 (concurrent capacity) tests inherent SQLite serialization — the actual race window is narrow but the scenario validates correctness under contention
- US3 scenarios omitted because F-08 (workspace resolution) is already fixed in the current codebase; the spec's FR-005/FR-006 are satisfied by existing code
