# Behavioral Matrix: ACP Event Handler Wiring

**Input**: Design documents from `/specs/006-acp-event-wiring/`
**Prerequisites**: spec.md (required), plan.md (required), data-model.md, research.md
**Created**: 2026-03-07

## Summary

| Metric | Count |
|---|---|
| **Total Scenarios** | 56 |
| Happy-path | 25 |
| Edge-case | 7 |
| Error | 7 |
| Boundary | 10 |
| Concurrent | 4 |
| Security | 3 |

**Non-happy-path coverage**: 55.4% (minimum 30% required) ✅

---

## ClearanceRequested Event Handler

Covers FR-001 (register with AcpDriver), FR-002 (persist ApprovalRequest), FR-003 (post Slack approval message), FR-007 (direct post), FR-009 (missing session), FR-010 (Slack unavailable). Handler location: `src/main.rs` `run_acp_event_consumer` match arm.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S001 | Standard clearance request with all fields and low risk | Active session in DB with Slack thread; event: `request_id="req-1"`, `session_id="s1"`, `title="Edit config"`, `description="Update timeout"`, `diff=Some("+timeout=30")`, `file_path="config.toml"`, `risk_level="low"` | `AgentEvent::ClearanceRequested` received on mpsc channel | AcpDriver::register_clearance called with ("s1", "req-1"); ApprovalRequest persisted with status=Pending, risk_level=Low; Slack approval message posted via post_message_direct with Accept/Reject buttons in session thread | ApprovalRequest row in DB; entry in AcpDriver pending_clearances map; Slack message posted as thread reply | happy-path |
| S002 | Clearance request with None diff field | Active session; event: `diff=None`, all other fields valid | `AgentEvent::ClearanceRequested` received | ApprovalRequest persisted with `diff_content=""` (empty string via `unwrap_or_default()`); Slack blocks built with empty diff section | ApprovalRequest.diff_content = "" in DB | happy-path |
| S003 | Clearance request with high risk level | Active session; event: `risk_level="high"`, all other fields valid | `AgentEvent::ClearanceRequested` received | ApprovalRequest persisted with `risk_level=High`; Slack blocks display high-risk indicator | ApprovalRequest.risk_level = "high" in DB | happy-path |
| S004 | Clearance request with critical risk level | Active session; event: `risk_level="critical"`, all other fields valid | `AgentEvent::ClearanceRequested` received | ApprovalRequest persisted with `risk_level=Critical`; Slack blocks display critical-risk indicator | ApprovalRequest.risk_level = "critical" in DB | happy-path |
| S005 | Session referenced by event does not exist in DB | No session row for `session_id="gone"`; event: `session_id="gone"` | `AgentEvent::ClearanceRequested` received | `warn!` tracing event emitted with session_id; event discarded; no AcpDriver registration; no DB insert; no Slack post | No side effects; event consumer continues processing next event | error |
| S006 | Slack service not configured or temporarily unavailable | Active session; Slack client returns `AppError::Slack` on post_message_direct | `AgentEvent::ClearanceRequested` received | AcpDriver::register_clearance called; ApprovalRequest persisted to DB; Slack post attempted and fails; `warn!` tracing event emitted; handler continues | ApprovalRequest in DB (slack_ts=None); entry in pending_clearances map; no Slack message | error |
| S007 | Database persistence failure during ApprovalRepo::create | Active session; DB returns error on INSERT (e.g., disk full) | `AgentEvent::ClearanceRequested` received | AcpDriver::register_clearance called; ApprovalRepo::create fails; `warn!` tracing event emitted; handler continues | Entry in pending_clearances map; no ApprovalRequest row in DB; event consumer continues | error |
| S008 | Clearance request with empty description string | Active session; event: `description=""` (empty, not None) | `AgentEvent::ClearanceRequested` received | ApprovalRequest persisted with `description=Some("")`; Slack blocks omit or show empty description section | ApprovalRequest.description = "" in DB | edge-case |
| S009 | Clearance request with very large diff content (>100 KB) | Active session; event: `diff=Some(large_string)` where len > 100,000 chars | `AgentEvent::ClearanceRequested` received | ApprovalRequest persisted with full diff_content; Slack block builder truncates display per INLINE_DIFF_THRESHOLD; no panic or allocation failure | Full diff stored in DB; Slack message displays truncated diff | boundary |

---

## PromptForwarded Event Handler

Covers FR-004 (register with AcpDriver), FR-005 (persist ContinuationPrompt), FR-006 (post Slack prompt message), FR-009 (missing session), FR-010 (Slack unavailable), FR-012 (prompt_type parsing). Handler location: `src/main.rs` `run_acp_event_consumer` match arm.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S010 | Standard prompt with continuation type | Active session with Slack thread; event: `session_id="s1"`, `prompt_id="p-1"`, `prompt_text="Should I continue?"`, `prompt_type="continuation"` | `AgentEvent::PromptForwarded` received on mpsc channel | AcpDriver::register_prompt_request called with ("s1", "p-1"); ContinuationPrompt persisted with prompt_type=Continuation; Slack prompt message enqueued with Continue/Refine/Stop buttons | ContinuationPrompt row in DB; entry in AcpDriver pending_prompts_acp map; message in Slack queue | happy-path |
| S011 | Prompt with clarification type | Active session; event: `prompt_type="clarification"` | `AgentEvent::PromptForwarded` received | ContinuationPrompt persisted with `prompt_type=Clarification`; Slack blocks display clarification icon and label | ContinuationPrompt.prompt_type = "clarification" in DB | happy-path |
| S012 | Prompt with error_recovery type | Active session; event: `prompt_type="error_recovery"` | `AgentEvent::PromptForwarded` received | ContinuationPrompt persisted with `prompt_type=ErrorRecovery`; Slack blocks display error recovery icon and label | ContinuationPrompt.prompt_type = "error_recovery" in DB | happy-path |
| S013 | Prompt with resource_warning type | Active session; event: `prompt_type="resource_warning"` | `AgentEvent::PromptForwarded` received | ContinuationPrompt persisted with `prompt_type=ResourceWarning`; Slack blocks display resource warning icon and label | ContinuationPrompt.prompt_type = "resource_warning" in DB | happy-path |
| S014 | Session referenced by prompt event does not exist in DB | No session row for `session_id="gone"`; event: `session_id="gone"` | `AgentEvent::PromptForwarded` received | `warn!` tracing event emitted with session_id; event discarded; no AcpDriver registration; no DB insert; no Slack post | No side effects; event consumer continues processing next event | error |
| S015 | Slack service not configured during prompt handling | Active session; Slack service is None or returns error on enqueue | `AgentEvent::PromptForwarded` received | AcpDriver::register_prompt_request called; ContinuationPrompt persisted to DB; Slack enqueue skipped; `warn!` tracing event emitted | ContinuationPrompt in DB (slack_ts=None); entry in pending_prompts_acp; no Slack message | error |
| S016 | Database persistence failure during PromptRepo::create | Active session; DB returns error on INSERT | `AgentEvent::PromptForwarded` received | AcpDriver::register_prompt_request called; PromptRepo::create fails; `warn!` tracing event emitted; handler continues | Entry in pending_prompts_acp map; no ContinuationPrompt row in DB; event consumer continues | error |
| S017 | Prompt with empty prompt_text | Active session; event: `prompt_text=""` | `AgentEvent::PromptForwarded` received | ContinuationPrompt persisted with empty prompt_text; Slack blocks display empty/minimal prompt section | ContinuationPrompt.prompt_text = "" in DB | boundary |

---

## Risk Level Parsing

Covers FR-011. Handler-level parsing defaults unknown values to `RiskLevel::Low` (per research RQ-4), distinct from the DB-level `parse_risk_level()` which returns `Err` for unknowns.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S018 | Parse "low" risk level string | Event field: `risk_level="low"` | Handler parses risk_level | Returns `RiskLevel::Low` | ApprovalRequest.risk_level = Low | happy-path |
| S019 | Parse "high" risk level string | Event field: `risk_level="high"` | Handler parses risk_level | Returns `RiskLevel::High` | ApprovalRequest.risk_level = High | happy-path |
| S020 | Parse "critical" risk level string | Event field: `risk_level="critical"` | Handler parses risk_level | Returns `RiskLevel::Critical` | ApprovalRequest.risk_level = Critical | happy-path |
| S021 | Unknown risk level string defaults to Low | Event field: `risk_level="extreme"` | Handler parses risk_level | Returns `RiskLevel::Low` (default); no error emitted | ApprovalRequest.risk_level = Low | boundary |
| S022 | Empty risk level string defaults to Low | Event field: `risk_level=""` | Handler parses risk_level | Returns `RiskLevel::Low` (default); no error emitted | ApprovalRequest.risk_level = Low | boundary |
| S023 | Mixed-case risk level not recognized (case-sensitive) | Event field: `risk_level="High"` or `risk_level="LOW"` | Handler parses risk_level | Returns `RiskLevel::Low` (default); matching is lowercase-only | ApprovalRequest.risk_level = Low | boundary |

---

## Prompt Type Parsing

Covers FR-012. Handler-level parsing defaults unknown values to `PromptType::Continuation` (per research RQ-5), distinct from the DB-level `parse_prompt_type()` which returns `Err` for unknowns.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S024 | Parse "continuation" prompt type | Event field: `prompt_type="continuation"` | Handler parses prompt_type | Returns `PromptType::Continuation` | ContinuationPrompt.prompt_type = Continuation | happy-path |
| S025 | Parse "clarification" prompt type | Event field: `prompt_type="clarification"` | Handler parses prompt_type | Returns `PromptType::Clarification` | ContinuationPrompt.prompt_type = Clarification | happy-path |
| S026 | Parse "error_recovery" prompt type | Event field: `prompt_type="error_recovery"` | Handler parses prompt_type | Returns `PromptType::ErrorRecovery` | ContinuationPrompt.prompt_type = ErrorRecovery | happy-path |
| S027 | Parse "resource_warning" prompt type | Event field: `prompt_type="resource_warning"` | Handler parses prompt_type | Returns `PromptType::ResourceWarning` | ContinuationPrompt.prompt_type = ResourceWarning | happy-path |
| S028 | Unknown prompt type defaults to Continuation | Event field: `prompt_type="custom_agent_query"` | Handler parses prompt_type | Returns `PromptType::Continuation` (default); Slack label shows "Continuation" | ContinuationPrompt.prompt_type = Continuation | boundary |
| S029 | Empty prompt type defaults to Continuation | Event field: `prompt_type=""` | Handler parses prompt_type | Returns `PromptType::Continuation` (default); no error emitted | ContinuationPrompt.prompt_type = Continuation | boundary |

---

## Content Hash Computation

Covers FR-013. The handler computes SHA-256 of the target file (resolved within session workspace via `path_safety.rs`) and stores it in `ApprovalRequest.original_hash` for conflict detection during later diff application.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S030 | File exists and has content | Session workspace contains `config.toml` with known content; event: `file_path="config.toml"` | Handler computes file hash | SHA-256 hex digest of file content computed and stored | ApprovalRequest.original_hash = hex SHA-256 string (64 chars) | happy-path |
| S031 | Target file does not exist (new file creation) | Session workspace has no file at `file_path="new_module.rs"` | Handler computes file hash | Hash set to `"new_file"` sentinel value; no error | ApprovalRequest.original_hash = "new_file" | edge-case |
| S032 | Target file is empty (0 bytes) | Session workspace contains empty file at `file_path="empty.txt"` | Handler computes file hash | SHA-256 of empty content: `"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"` | ApprovalRequest.original_hash = SHA-256 of empty bytes | boundary |
| S033 | Path traversal attempt in file_path | Event: `file_path="../../etc/passwd"` | Handler resolves path via path_safety | path_safety rejects traversal; handler emits `warn!` with rejected path; hash set to `"new_file"` sentinel | Path is NOT resolved outside workspace boundary; ApprovalRequest.original_hash = "new_file" | security |
| S034 | Absolute path outside workspace in file_path | Event: `file_path="/etc/shadow"` (Unix) or `file_path="C:\\Windows\\System32\\config"` (Windows) | Handler resolves path via path_safety | path_safety rejects absolute path; handler emits `warn!`; hash set to `"new_file"` sentinel | No file read outside workspace; ApprovalRequest.original_hash = "new_file" | security |
| S035 | Null bytes or control characters in file_path | Event: `file_path="config\x00.toml"` | Handler resolves path via path_safety | path_safety rejects malformed path; handler emits `warn!`; hash set to `"new_file"` sentinel | No file system access with malformed path; ApprovalRequest.original_hash = "new_file" | security |

---

## Thread Management

Covers FR-007 (direct post for clearance), FR-008 (thread_ts management). User Story 3 specifies that the first message for any session creates the thread via direct post. `SessionRepo::set_thread_ts` is a write-once operation (UPDATE WHERE thread_ts IS NULL).

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S036 | Clearance is first event — no existing Slack thread | Active session with `thread_ts=None` in DB | `AgentEvent::ClearanceRequested` received | Message posted via `post_message_direct()` (not enqueued); returned Slack `ts` saved via `SessionRepo::set_thread_ts()` | Session.thread_ts updated to Slack ts; ApprovalRequest.slack_ts = Slack ts | happy-path |
| S037 | Clearance with existing Slack thread | Active session with `thread_ts=Some("1234.5678")` in DB | `AgentEvent::ClearanceRequested` received | Message posted via `post_message_direct()` as reply in existing thread (thread_ts="1234.5678"); `set_thread_ts` is no-op (already set) | Session.thread_ts unchanged; message appears in thread | happy-path |
| S038 | Prompt is first event — no existing Slack thread | Active session with `thread_ts=None`; first event is PromptForwarded | `AgentEvent::PromptForwarded` received | Message posted via `post_message_direct()` (not enqueued) to create thread; returned ts saved via `set_thread_ts()` | Session.thread_ts updated; thread created | edge-case |
| S039 | Prompt with existing Slack thread | Active session with `thread_ts=Some("1234.5678")` | `AgentEvent::PromptForwarded` received | Message enqueued via `SlackService::enqueue()` with thread_ts for in-thread reply | Message queued for async delivery in existing thread | happy-path |
| S040 | set_thread_ts is idempotent when already set | Session with `thread_ts=Some("1234.5678")`; second clearance arrives | Handler calls `set_thread_ts("s1", "9999.0000")` | SQL UPDATE has `WHERE thread_ts IS NULL` predicate — no rows updated; original thread_ts preserved | Session.thread_ts remains "1234.5678" (unchanged) | edge-case |
| S041 | Clearance creates thread then prompt uses same thread | Session starts with no thread; clearance event arrives first, then prompt event | ClearanceRequested → PromptForwarded (sequential) | First event: direct post, saves thread_ts="ts-from-clearance"; Second event: enqueue with thread_ts="ts-from-clearance" | Both messages appear in same Slack thread anchored by clearance | edge-case |

---

## Shared Block Builders

Covers design decision D1 (shared block extraction to `src/slack/blocks.rs`). Both MCP tools and ACP event handlers must produce identical Slack Block Kit output for the same inputs. Functions: `build_approval_blocks()`, `build_prompt_blocks()`, `prompt_type_label()`, `prompt_type_icon()`, `truncate_text()`.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S042 | Approval blocks contain required structure | Input: `title="Edit config"`, `description=Some("Update timeout")`, `diff="+timeout=30"`, `file_path="config.toml"`, `risk_level=Low` | `build_approval_blocks()` called | Returns Vec<SlackBlock> containing: header with title, description section, risk level badge, diff code block, Accept/Reject action buttons with request_id in action_id | Pure function; no side effects | happy-path |
| S043 | Prompt blocks contain required structure | Input: `prompt_text="Should I continue?"`, `prompt_type=Continuation`, `elapsed_seconds=None`, `actions_taken=None`, `prompt_id="p-1"` | `build_prompt_blocks()` called | Returns Vec<SlackBlock> containing: header with prompt_type icon + label, prompt text section, Continue/Refine/Stop action buttons with prompt_id in action_id | Pure function; no side effects | happy-path |
| S044 | Diff exceeding INLINE_DIFF_THRESHOLD is truncated | Input: diff with 25 lines (threshold = 20); `file_path="large.rs"` | `build_approval_blocks()` called | Diff display truncated or summarized; full diff NOT shown inline in Slack blocks | Blocks contain truncation indicator | boundary |
| S045 | Prompt text exceeding max length is truncated | Input: `prompt_text` with 5000 chars (exceeds Slack block text limit) | `build_prompt_blocks()` called | `truncate_text()` applied; text ends with ellipsis `…` at max_chars boundary | Truncated text in output blocks | boundary |
| S046 | MCP and ACP produce identical blocks for same inputs | MCP tool call with `title="T"`, `diff="D"`, `file_path="F"`, `risk_level=High`; ACP event with same field values | Both codepaths call `build_approval_blocks()` from `slack/blocks.rs` | Identical Vec<SlackBlock> output from both call sites (same import, same function) | No divergence between MCP and ACP Slack message rendering | happy-path |

---

## Concurrent Event Processing

Covers SC-005 (rapid successive events without data loss). Tests that independent events are tracked, persisted, and displayed separately even under concurrent load.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S047 | Two clearance requests for same session in rapid succession | Active session "s1"; two ClearanceRequested events with different `request_id` values arrive within <100ms | Both events received on mpsc channel sequentially | Each event independently: registers with AcpDriver (separate map entries), persists separate ApprovalRequest rows, posts separate Slack messages | Two distinct ApprovalRequest rows; two entries in pending_clearances; two Slack messages in thread | concurrent |
| S048 | Clearance and prompt for same session interleaved | Active session "s1"; ClearanceRequested then PromptForwarded arrive back-to-back | Both events received sequentially on mpsc | Clearance: register + persist + direct post; Prompt: register + persist + enqueue; no cross-contamination between handlers | One ApprovalRequest + one ContinuationPrompt in DB; separate AcpDriver map entries | concurrent |
| S049 | Events from multiple sessions processed independently | Two active sessions "s1" and "s2"; each emits a ClearanceRequested event | Both events received on shared mpsc channel | Each event processed with its own session context; no shared state leakage between sessions | Separate ApprovalRequest rows per session; separate Slack threads per session | concurrent |
| S050 | AcpDriver registration and DB persistence ordering | Active session; ClearanceRequested arrives; DB write attempted before driver registration | Event processed by handler | DB persistence is attempted first; on success, driver registration follows using the DB-generated approval ID. If DB write fails, handler emits `warn!` and skips driver registration to avoid unaudited pending state | On DB success: both DB record and driver entry exist with matching ID. On DB failure: neither record exists; agent does not hang (no pending entry to wait on) | concurrent |

---

## Event Consumer Lifecycle

Covers the `run_acp_event_consumer` task loop behavior for startup, shutdown, and edge conditions. Design decision D3 (log-and-continue) applies to all error handling within the loop.

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S051 | Normal event processing loop receives and dispatches events | Event consumer running; valid events on mpsc channel | Multiple AgentEvent variants received | Each event dispatched to correct match arm; ClearanceRequested and PromptForwarded handlers execute full pipeline; other variants (StatusUpdated, HeartbeatReceived, etc.) handled by existing arms | Consumer remains alive and responsive between events | happy-path |
| S052 | Cancellation token fires — consumer exits gracefully | Event consumer running; `cancel.cancelled()` becomes ready | CancellationToken cancelled (server shutdown) | Consumer exits select! loop; any in-progress event handler completes or is interrupted; no panic; `info!` log emitted | Consumer task terminates cleanly; no dangling resources | edge-case |
| S053 | mpsc channel sender dropped — consumer exits loop | All ACP session AcpReader tasks terminate; mpsc sender is dropped | `rx.recv()` returns `None` | Consumer detects closed channel; exits loop gracefully | Consumer task terminates; no panic | edge-case |
| S054 | Operator responds to clearance after ACP session terminated | ApprovalRequest persisted and Slack message posted; ACP session later terminates; operator clicks Accept in Slack | Slack button handler calls `state.driver.resolve_clearance(request_id)` | AcpDriver returns error (session writer gone / unknown session); Slack handler logs warning; button handler responds with error ephemeral message to operator | ApprovalRequest remains in Pending state; no agent response sent | error |

---

## Field Mapping Correctness

Validates the event-to-record field transformations defined in `data-model.md`. These scenarios confirm constructor arguments and default values for records created by ACP event handlers (vs. MCP tool handlers which set additional fields).

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S055 | ClearanceRequested maps to ApprovalRequest with correct defaults | Event: all fields populated; `diff=Some("...")`, `risk_level="high"` | Handler constructs ApprovalRequest via `ApprovalRequest::new()` | Fields: `id` = auto UUID, `session_id` = event.session_id, `title` = event.title, `description` = Some(event.description), `diff_content` = event.diff.unwrap_or_default(), `file_path` = event.file_path, `risk_level` = High, `original_hash` = computed SHA-256, `status` = Pending, `consumed_at` = None, `created_at` = Utc::now() | ApprovalRequest record matches data-model.md field mapping table | happy-path |
| S056 | PromptForwarded maps to ContinuationPrompt with ACP-specific defaults | Event: `prompt_type="clarification"`, `prompt_text="Need input"` | Handler constructs ContinuationPrompt via `ContinuationPrompt::new()` | Fields: `id` = auto UUID, `session_id` = event.session_id, `prompt_text` = event.prompt_text, `prompt_type` = Clarification, `elapsed_seconds` = None, `actions_taken` = None, `decision` = None, `instruction` = None, `slack_ts` = None (queued path), `created_at` = Utc::now() | ContinuationPrompt record matches data-model.md; ACP-only fields (elapsed_seconds, actions_taken) are None | happy-path |

### Adversarial Review Additions (S057–S067)

| ID | Scenario | Input State | Trigger | Expected Behavior | Verifiable Outcome | Category |
|---|---|---|---|---|---|---|
| S057 | AcpDriver::register_clearance returns error | Active session; ClearanceRequested arrives; driver rejects registration (e.g., duplicate request_id) | Handler calls register_clearance() | Handler emits `warn!` with error detail; continues without Slack post; DB record still persisted | warn! log emitted; no Slack message; ApprovalRequest row exists in DB | error |
| S058 | AcpDriver::register_prompt_request returns error | Active session; PromptForwarded arrives; driver rejects registration | Handler calls register_prompt_request() | Handler emits `warn!`; continues without Slack post; DB record still persisted | warn! log emitted; no Slack message; ContinuationPrompt row exists in DB | error |
| S059 | Workspace resolution failure during hash computation | Active session; ClearanceRequested with valid file_path; session has no workspace_id or workspace not in config | Handler attempts to resolve workspace root | Handler emits `warn!`; sets original_hash to `"new_file"` sentinel; continues with persistence and Slack post | ApprovalRequest.original_hash = "new_file"; warn! log includes session_id and workspace context | edge-case |
| S060 | File path is a symlink resolving outside workspace | Active session; ClearanceRequested with `file_path="link_to_outside"` where symlink target is outside workspace | path_safety resolves canonical path | path_safety rejects after canonicalization; handler emits `warn!`; hash set to `"new_file"` | Path NOT followed outside workspace; ApprovalRequest.original_hash = "new_file" | security |
| S061 | File path points to a directory instead of a file | Active session; ClearanceRequested with `file_path="src/"` (a directory) | Handler attempts hash computation | Hash computation fails gracefully (not a regular file); hash set to `"new_file"` sentinel | ApprovalRequest.original_hash = "new_file"; no panic | boundary |
| S062 | Slack API returns 429 (rate limit) on post_message_direct | Active session; ClearanceRequested; Slack API returns 429 with Retry-After header | Handler calls post_message_direct() | Handler emits `warn!` with rate-limit detail; approval record persisted without slack_ts; driver registration intact | ApprovalRequest.slack_ts = None; driver pending entry exists; warn! log emitted | error |
| S063 | Post-termination prompt response (symmetric with S054) | Session terminated; operator clicks Continue on a previously-posted ACP prompt | Slack button handler calls resolve_prompt() | AcpDriver returns error (session writer gone); Slack handler posts ephemeral error to operator | No agent receives the response; ephemeral message shown to operator | edge-case |
| S064 | Two first-events for same session both see thread_ts=None | Active session with no thread; ClearanceRequested and PromptForwarded arrive nearly simultaneously | Both handlers read thread_ts=None and call post_message_direct() | Both messages posted successfully; first set_thread_ts() wins (idempotent write); second set_thread_ts() is no-op or ignored | Two Slack messages exist; session.thread_ts set to first message's ts; no data corruption | concurrent |
| S065 | ClearanceRequested with missing required fields | Event arrives with null/empty session_id | Handler validates event fields | Handler emits `warn!` with field validation detail; event discarded without DB persist or driver registration | No ApprovalRequest row; no driver pending entry; warn! log | boundary |
| S066 | PromptForwarded with empty prompt_text | Event arrives with `prompt_text=""` (empty string, not null) | Handler processes event | Handler persists record with empty prompt_text; Slack message shows empty prompt body; no error | ContinuationPrompt.prompt_text = ""; Slack blocks posted with empty text section | boundary |
| S067 | Unauthorized Slack user clicks Accept on ACP clearance message | ACP clearance posted to Slack; non-owner user clicks Accept | Slack authorization guard in events.rs | Guard rejects action (user not session owner); no state mutation; no driver resolution | Approval status unchanged (Pending); no AcpDriver resolution; guard logged | security |

---

## Edge Case Coverage Checklist

- [x] Malformed inputs and invalid arguments — S021-S023 (risk_level), S028-S029 (prompt_type), S033-S035 (file_path), S065 (missing session_id), S066 (empty prompt_text)
- [x] Missing dependencies and unavailable resources — S005-S006, S014-S015 (session/Slack missing), S059 (workspace missing)
- [x] State errors and race conditions — S050 (registration/persistence ordering), S054 (post-termination clearance response), S063 (post-termination prompt response)
- [x] Boundary values (empty, max-length, zero, negative) — S002 (None diff), S008 (empty description), S009 (large diff), S017 (empty prompt), S022/S029 (empty strings), S032 (empty file), S061 (directory path), S066 (empty prompt_text)
- [x] Permission and authorization failures — S033-S035 (path traversal/injection), S060 (symlink outside workspace), S067 (unauthorized Slack user)
- [x] Concurrent access patterns — S047-S050 (rapid succession, multi-session, interleaved), S064 (two first-events race)
- [x] Graceful degradation scenarios — S006/S015 (Slack unavailable), S007/S016 (DB failure), S052-S053 (shutdown), S057-S058 (driver registration failure), S062 (Slack rate limit)

## Cross-Reference Validation

- [x] Every entity in `data-model.md` has at least one scenario covering its state transitions — ApprovalRequest (S001, S055), ContinuationPrompt (S010, S056), AgentEvent::ClearanceRequested (S001-S009), AgentEvent::PromptForwarded (S010-S017)
- [x] Every user story in `spec.md` has corresponding behavioral coverage — US1 (S001-S009), US2 (S010-S017), US3 (S036-S041)
- [x] Every functional requirement (FR-001 through FR-015) has at least one scenario — FR-001 (S001), FR-002 (S001, S055), FR-003 (S001), FR-004 (S010), FR-005 (S010, S056), FR-006 (S010), FR-007 (S036), FR-008 (S036-S041), FR-009 (S005, S014), FR-010 (S006, S015), FR-011 (S018-S023), FR-012 (S024-S029), FR-013 (S030-S035, S059-S061), FR-014 (S001 tracing verified implicitly), FR-015 (deferred — no scenario required)
- [x] Every edge case in spec.md has corresponding scenarios — missing session (S005, S014), Slack unavailable (S006, S015), rapid succession (S047), post-termination (S054, S063), unknown prompt_type (S028)
- [x] No scenario has ambiguous or non-deterministic expected outcomes

## Notes

- Scenario IDs are globally sequential (S001–S067) across all components
- Categories: `happy-path`, `edge-case`, `error`, `boundary`, `concurrent`, `security`
- S057–S067 added during adversarial review remediation
- Each row is deterministic — exactly one expected outcome per input state
- Tables are grouped by component/subsystem under level-2 headings
- Scenarios map directly to parameterized test cases (Rust `#[rstest]` blocks for unit/contract tests, async integration tests for full pipeline)
- Outbound button handling (Accept/Reject/Continue/Refine/Stop Slack actions) is out of scope per spec assumptions — already implemented via polymorphic `state.driver` dispatch
- S054 (post-termination response) is included for completeness as it validates existing driver behavior when exercised by ACP-originated requests
