---
id: TASK-006
title: "ACP Event Wiring"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - feature
dependencies: []
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
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
5. **Given** a pending ACP prompt, **When** the configured prompt timeout elapses without operator action, **Then** the system treats the prompt as expired and the prompt record is updated accordingly.

---

### User Story 3 — Session Thread Continuity (Priority: P2)

When an ACP agent's first interaction with the operator is a clearance request or prompt, and no Slack thread yet exists for the session, the system creates the thread by posting the message directly (not via the background queue) and records the resulting message timestamp as the session's thread anchor. All subsequent messages for that session appear in the same thread.

**Why this priority**: Thread continuity is essential for operator usability when managing multiple concurrent sessions. Without it, approval and prompt messages scatter across the channel instead of grouping under the session thread.

**Independent Test**: Can be tested by starting an ACP session that has no prior Slack thread, triggering a clearance request, verifying the message creates a new thread, then triggering a second event and verifying it appears in the same thread.

**Acceptance Scenarios**:

1a. **Given** an ACP session with no existing Slack thread, **When** the first clearance request is posted, **Then** the message is posted directly (not queued) and the returned message timestamp is saved as the session's thread anchor.
1b. **Given** an ACP session with no existing Slack thread, **When** the first continuation prompt is posted, **Then** the message is posted directly (not queued) and the returned message timestamp is saved as the session's thread anchor.
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
- **FR-003**: System MUST post an interactive approval message to the session's Slack thread when a `ClearanceRequested` event is received, containing the file path, risk level indicator, diff content (inline up to `INLINE_DIFF_THRESHOLD` lines; larger diffs are truncated with a line-count indicator), and Accept/Reject action buttons.
- **FR-004**: System MUST register each incoming `PromptForwarded` event with the ACP driver's pending prompt map, associating the prompt ID with the session ID.
- **FR-005**: System MUST persist each incoming `PromptForwarded` event as a continuation prompt record in the database, capturing prompt text and prompt type.
- **FR-006**: System MUST post an interactive prompt message to the session's Slack thread when a `PromptForwarded` event is received, containing the prompt text, prompt type label, and Continue/Refine/Stop action buttons.
- **FR-007**: System MUST use direct message posting (not the background queue) for any event (clearance request or prompt) that is the first message in a session's Slack thread, so the returned message timestamp can be captured and stored for threading. Clearance requests MUST always use direct posting regardless of thread state to capture the `slack_ts` for the approval record.
- **FR-008**: System MUST use the session's existing Slack thread timestamp when posting messages, and if no thread exists, record the first message's timestamp as the session's thread anchor.
- **FR-009**: System MUST gracefully handle missing sessions — when the session ID from an event cannot be found in the database, the system logs a warning and discards the event.
- **FR-010**: System MUST gracefully handle Slack unavailability — when Slack is not configured, the system still persists the record and registers the request but skips posting.
- **FR-011**: System MUST parse the `risk_level` string from the event into the appropriate risk classification for display and persistence. Matching is case-sensitive: only lowercase values `"low"`, `"high"`, `"critical"` are recognized; all other values (including mixed-case variants) default to Low.
- **FR-012**: System MUST parse the `prompt_type` string from the event into the appropriate prompt type for display and persistence, defaulting to "continuation" for unrecognized values. Matching is case-sensitive: only lowercase values `"continuation"`, `"clarification"`, `"error_recovery"`, `"resource_warning"` are recognized.
- **FR-013**: System MUST validate file paths via `path_safety` before computing content hashes. Paths outside the workspace root MUST be rejected (consistent with `AppError::PathViolation`). For valid paths where the file exists, the system MUST compute a SHA-256 content hash to enable conflict detection during later diff application. For non-existent files or rejected paths, the system MUST set the hash to the `"new_file"` sentinel value.
- **FR-014**: System MUST emit structured `tracing` spans at `info` level for each event handler invocation, including the session ID, event type, and request/prompt ID. Error paths MUST emit `warn!` level spans with the error detail.
- **FR-015**: System MUST support configurable timeout periods for ACP clearance requests and continuation prompts. The timeout mechanism and its interaction with `AcpDriver` pending maps is deferred to a dedicated timeout feature; this feature documents the requirement for future implementation.
- **FR-016**: System SHOULD bound the size of `AcpDriver` pending maps. When `AcpConfig.max_sessions` concurrent sessions are active, the maximum number of pending entries per map is bounded by the session count. Explicit TTL-based eviction and capacity enforcement are deferred to the timeout feature (FR-015) but the handler MUST NOT panic or corrupt state if the map grows beyond expected bounds.

### Key Entities

- **Approval Request**: Represents a pending file operation clearance. Attributes: unique ID, session ID, request ID (ACP protocol identifier), title, description, diff content, file path, risk level, approval status, content hash (original_hash), Slack message timestamp, creation timestamp.
- **Continuation Prompt**: Represents a pending operator decision point. Attributes: unique ID, session ID, prompt text, prompt type, elapsed seconds, actions taken, operator decision, instruction text, Slack message timestamp, creation timestamp.

### Non-Functional Requirements

- **NFR-001**: Event-to-Slack-post latency SHOULD be under 2 seconds under single-session, normal Slack API conditions (p99 < 500ms). This is a target, not a hard gate.
- **NFR-002**: The system SHOULD handle events from up to 5 concurrent ACP sessions (configurable via `AcpConfig.max_sessions`) without resource contention.
- **NFR-003**: Duplicate events with the same `request_id` or `prompt_id` SHOULD be handled gracefully. The system MAY create separate records for duplicate events (idempotency enforcement is deferred to a future feature) but MUST NOT crash or corrupt state.

## Assumptions

- The existing Slack button handlers for approval responses (Accept/Reject) and prompt responses (Continue/Refine/Stop) already route decisions through the ACP driver's `resolve_clearance` and `resolve_prompt` methods. This feature only needs to wire the *inbound* side (event → registration + persistence + Slack post); the *outbound* side (Slack button → driver resolution → agent response) is already functional.
- The `AcpDriver` methods `register_clearance` and `register_prompt_request` are already implemented and tested. This feature calls them from a new location (the event consumer) rather than reimplementing them.
- The `ApprovalRepo::create` and `PromptRepo::create` database methods are already implemented and tested. This feature calls them from the event consumer.
- The `build_approval_blocks` and `build_prompt_blocks` Slack message builders are already implemented. This feature reuses them for constructing ACP event messages.
- The `slack/events.rs` authorization guard applies to all Slack interactions including ACP-originated messages. Only the session owner can interact with buttons on ACP messages.
- ACP timeout infrastructure (pending map expiry, timer scheduling) is **not** part of this feature. FR-015 documents the requirement; implementation is deferred to a dedicated timeout feature.

## Threat Model Note

Diff content posted to Slack may contain sensitive information (API keys, credentials, PII) embedded in file changes. This is a cross-cutting concern that also affects the existing MCP clearance flow (`check_clearance` tool). Secret redaction is recommended as a dedicated security feature covering both MCP and ACP paths rather than an ACP-only implementation. Until addressed, operators should review diffs in channels with restricted membership.

## Glossary

| Term | Context | Definition |
|---|---|---|
| **Clearance Request** | ACP protocol | The `ClearanceRequested` event emitted by an ACP agent when it needs operator approval for a file operation |
| **Approval Request** | Database persistence | The `ApprovalRequest` entity stored in SQLite representing a persisted clearance request |
| **Continuation Prompt** | Database persistence | The `ContinuationPrompt` entity stored in SQLite representing a persisted prompt forwarding event |
| **pending_clearances** | AcpDriver in-memory | The `HashMap` in `AcpDriver` mapping request IDs to session stream writers for clearance resolution |
| **pending_prompts_acp** | AcpDriver in-memory | The `HashMap` in `AcpDriver` mapping prompt IDs to session stream writers for prompt resolution |
| **original_hash** | ApprovalRequest field | SHA-256 hash of the target file content at request time, or `"new_file"` sentinel for non-existent/invalid paths |

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: ACP agents that emit clearance requests receive an operator decision (approved or rejected) within a bounded time period, without hanging indefinitely. *(Note: The timeout mechanism is deferred to FR-015; this criterion is satisfied when the end-to-end flow from event to operator response is functional.)*
- **SC-002**: ACP agents that emit continuation prompts receive an operator decision (continue, refine, or stop) within a bounded time period, without hanging indefinitely. *(Note: The timeout mechanism is deferred to FR-015; this criterion is satisfied when the end-to-end flow from event to operator response is functional.)*
- **SC-003**: All ACP clearance requests and continuation prompts are attempted for database persistence, creating an auditable record. In degraded conditions (DB failure), the system logs the failure at `warn` level and continues processing to avoid blocking the agent — the driver registration proceeds but the DB record may be absent.
- **SC-004**: Operators see ACP clearance and prompt messages in the correct session thread in Slack, maintaining per-session conversation grouping.
- **SC-005**: The system handles rapid successive events (two or more clearance/prompt events within 1 second) without data loss or duplicate records, with each event producing exactly one DB record and one Slack message under concurrent load.


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
| S068 | Slack post succeeds but thread_ts DB persistence fails | Active session with no thread; ClearanceRequested; post_message_direct succeeds (returns ts); set_thread_ts DB write fails | Handler attempts to save thread_ts after successful Slack post | Handler emits `warn!` with DB error; approval record has slack_ts set but session.thread_ts remains None; subsequent events will also attempt direct post (self-healing on next success) | Approval record has slack_ts; session thread_ts NOT updated; no crash; next event retries thread creation | error |

---

## Edge Case Coverage Checklist

- [x] Malformed inputs and invalid arguments — S021-S023 (risk_level), S028-S029 (prompt_type), S033-S035 (file_path), S065 (missing session_id), S066 (empty prompt_text)
- [x] Missing dependencies and unavailable resources — S005-S006, S014-S015 (session/Slack missing), S059 (workspace missing)
- [x] State errors and race conditions — S050 (registration/persistence ordering), S054 (post-termination clearance response), S063 (post-termination prompt response)
- [x] Boundary values (empty, max-length, zero, negative) — S002 (None diff), S008 (empty description), S009 (large diff), S017 (empty prompt), S022/S029 (empty strings), S032 (empty file), S061 (directory path), S066 (empty prompt_text)
- [x] Permission and authorization failures — S033-S035 (path traversal/injection), S060 (symlink outside workspace), S067 (unauthorized Slack user)
- [x] Concurrent access patterns — S047-S050 (rapid succession, multi-session, interleaved), S064 (two first-events race)
- [x] Graceful degradation scenarios — S006/S015 (Slack unavailable), S007/S016 (DB failure), S052-S053 (shutdown), S057-S058 (driver registration failure), S062 (Slack rate limit), S068 (Slack success but thread_ts DB failure)

## Cross-Reference Validation

- [x] Every entity in `data-model.md` has at least one scenario covering its state transitions — ApprovalRequest (S001, S055), ContinuationPrompt (S010, S056), AgentEvent::ClearanceRequested (S001-S009), AgentEvent::PromptForwarded (S010-S017)
- [x] Every user story in `spec.md` has corresponding behavioral coverage — US1 (S001-S009), US2 (S010-S017), US3 (S036-S041)
- [x] Every functional requirement (FR-001 through FR-015) has at least one scenario — FR-001 (S001), FR-002 (S001, S055), FR-003 (S001), FR-004 (S010), FR-005 (S010, S056), FR-006 (S010), FR-007 (S036), FR-008 (S036-S041), FR-009 (S005, S014), FR-010 (S006, S015), FR-011 (S018-S023), FR-012 (S024-S029), FR-013 (S030-S035, S059-S061), FR-014 (S001 tracing verified implicitly), FR-015 (deferred — no scenario required)
- [x] Every edge case in spec.md has corresponding scenarios — missing session (S005, S014), Slack unavailable (S006, S015), rapid succession (S047), post-termination (S054, S063), unknown prompt_type (S028)
- [x] No scenario has ambiguous or non-deterministic expected outcomes

## Notes

- Scenario IDs are globally sequential (S001–S068) across all components
- Categories: `happy-path`, `edge-case`, `error`, `boundary`, `concurrent`, `security`
- S057–S067 added during adversarial review remediation
- Each row is deterministic — exactly one expected outcome per input state
- Tables are grouped by component/subsystem under level-2 headings
- Scenarios map directly to parameterized test cases (Rust `#[rstest]` blocks for unit/contract tests, async integration tests for full pipeline)
- Outbound button handling (Accept/Reject/Continue/Refine/Stop Slack actions) is out of scope per spec assumptions — already implemented via polymorphic `state.driver` dispatch
- S054 (post-termination response) is included for completeness as it validates existing driver behavior when exercised by ACP-originated requests


# Data Model: ACP Event Handler Wiring

**Feature**: 006-acp-event-wiring
**Date**: 2026-03-07

## Entities

This feature uses **existing** entities and database tables. No new tables or schema changes are required.

### ApprovalRequest (existing)

Represents a pending file operation clearance request.

| Field | Type | Source (ACP handler) | Notes |
|-------|------|---------------------|-------|
| id | String (UUID) | Auto-generated by `ApprovalRequest::new()` | Primary key |
| session_id | String | From `AgentEvent::ClearanceRequested.session_id` | FK to sessions table |
| title | String | From event `.title` | Short description |
| description | Option<String> | From event `.description` | Detailed context |
| diff_content | String | From event `.diff` (or empty if None) | Unified diff |
| file_path | String | From event `.file_path` | Target file |
| risk_level | RiskLevel enum | Parsed from event `.risk_level` string | Low/High/Critical |
| status | ApprovalStatus enum | `Pending` (initial) | State machine |
| original_hash | String | Computed from file at `file_path` | SHA-256 or "new_file" |
| slack_ts | Option<String> | From `post_message_direct()` response | Thread anchor |
| created_at | DateTime | Auto-set by `new()` | Audit trail |
| consumed_at | Option<DateTime> | None (initial) | Set on resolution |

**State transitions**: Pending → Approved | Rejected | Expired | Consumed | Interrupted

### ContinuationPrompt (existing)

Represents a pending operator decision point.

| Field | Type | Source (ACP handler) | Notes |
|-------|------|---------------------|-------|
| id | String (UUID) | Auto-generated by `ContinuationPrompt::new()` | Primary key |
| session_id | String | From `AgentEvent::PromptForwarded.session_id` | FK to sessions table |
| prompt_text | String | From event `.prompt_text` | Display text |
| prompt_type | PromptType enum | Parsed from event `.prompt_type` string | Continuation/Clarification/ErrorRecovery/ResourceWarning |
| elapsed_seconds | Option<i64> | None (not in ACP event) | MCP-only field |
| actions_taken | Option<i64> | None (not in ACP event) | MCP-only field |
| decision | Option<PromptDecision> | None (initial) | Continue/Refine/Stop |
| instruction | Option<String> | None (initial) | Operator text |
| slack_ts | Option<String> | None (queued, not direct posted) | Thread reference |
| created_at | DateTime | Auto-set by `new()` | Audit trail |

**State transitions**: Created (no decision) → Continue | Refine | Stop

### AgentEvent::ClearanceRequested (existing)

Event variant emitted by ACP reader when agent requests clearance.

| Field | Type | Notes |
|-------|------|-------|
| request_id | String | Unique per request |
| session_id | String | Owning ACP session |
| title | String | Short description |
| description | String | Detailed context |
| diff | Option<String> | Unified diff (may be None for non-diff operations) |
| file_path | String | Target file path |
| risk_level | String | "low", "high", or "critical" |

### AgentEvent::PromptForwarded (existing)

Event variant emitted by ACP reader when agent forwards a prompt.

| Field | Type | Notes |
|-------|------|-------|
| session_id | String | Owning ACP session |
| prompt_id | String | Unique per prompt |
| prompt_text | String | Display text |
| prompt_type | String | "continuation", "clarification", "error_recovery", "resource_warning" |

## Relationships

```text
Session (1) ──── (N) ApprovalRequest
Session (1) ──── (N) ContinuationPrompt
AgentEvent  ───maps to──→  ApprovalRequest | ContinuationPrompt
```

## Field Mapping: Event → Record

### ClearanceRequested → ApprovalRequest

| Event Field | Record Field | Transformation |
|-------------|-------------|----------------|
| session_id | session_id | Direct copy |
| request_id | request_id | Direct copy (ACP protocol-level identifier, preserved for correlation) |
| title | title | Direct copy |
| description | description | Some(description) |
| diff | diff_content | unwrap_or_default() |
| file_path | file_path | Direct copy |
| risk_level | risk_level | parse_risk_level_or_default() → RiskLevel enum (case-sensitive, defaults to Low) |
| (computed) | original_hash | compute_file_hash(file_path) via path_safety; "new_file" sentinel on rejection/not-found |
| (generated) | id | UUID::new_v4() — **used as driver registration key and Slack button action_id** |
| (set) | status | ApprovalStatus::Pending |

> **Note**: The AcpDriver is registered with `approval.id` (the DB-generated UUID), not `event.request_id`. This ensures Slack button action_ids match the driver's pending map keys. The `request_id` is preserved on the record for protocol-level correlation and logging.

### PromptForwarded → ContinuationPrompt

| Event Field | Record Field | Transformation |
|-------------|-------------|----------------|
| session_id | session_id | Direct copy |
| prompt_text | prompt_text | Direct copy |
| prompt_type | prompt_type | parse_prompt_type() → PromptType enum |
| (generated) | id | UUID::new_v4() |
| (set) | elapsed_seconds | None |
| (set) | actions_taken | None |


# Quickstart: ACP Event Handler Wiring

**Feature**: 006-acp-event-wiring
**Date**: 2026-03-07

## What This Feature Does

Wires the ACP event consumer's `ClearanceRequested` and `PromptForwarded` handlers so that ACP agents can request operator approval for file operations and forward continuation prompts via Slack. Previously these handlers only logged events — now they persist records, register with the ACP driver, and post interactive Slack messages.

## Testing the Feature

### Prerequisites

1. Server running in ACP mode with Slack configured
2. A workspace mapping configured for a Slack channel
3. An ACP agent session started in the configured channel

### Test 1: Clearance Request Flow

1. Start an ACP session via `/arc session-start`
2. In the ACP agent, trigger a file operation that requires approval (e.g., file deletion or modification in a non-auto-approved path)
3. **Observe**: An approval message appears in the session's Slack thread with:
   - File path and risk level indicator (🟢/🟡/🔴)
   - Diff content (inline for small diffs, file upload for large)
   - Accept and Reject buttons
4. Click **Accept**
5. **Observe**: The agent receives the approval and proceeds with the file operation
6. **Verify**: Check the database — `approval_request` table has a record with `status = 'approved'`

### Test 2: Prompt Forwarding Flow

1. Start an ACP session via `/arc session-start`
2. In the ACP agent, trigger a continuation prompt (e.g., agent asks for clarification)
3. **Observe**: A prompt message appears in the session's Slack thread with:
   - Prompt type icon and label
   - Prompt text
   - Continue, Refine, and Stop buttons
4. Click **Continue** (or Refine with instructions, or Stop)
5. **Observe**: The agent receives the decision and acts accordingly
6. **Verify**: Check the database — `continuation_prompt` table has a record with the operator's decision

### Test 3: Thread Continuity

1. Start an ACP session (no prior messages in the channel thread)
2. Trigger a clearance request
3. **Observe**: The approval message creates a new Slack thread
4. Trigger a second clearance request or prompt
5. **Observe**: The second message appears as a reply in the same thread

## Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Event consumer with wired handlers |
| `src/slack/blocks.rs` | Shared block builders (extracted from MCP tools) |
| `src/mcp/tools/ask_approval.rs` | MCP tool (now imports shared builders) |
| `src/mcp/tools/forward_prompt.rs` | MCP tool (now imports shared builders) |
| `src/driver/acp_driver.rs` | ACP driver registration and resolution |
| `src/persistence/approval_repo.rs` | Approval request persistence |
| `src/persistence/prompt_repo.rs` | Prompt persistence |




---

## Checklists

# Specification Quality Checklist: ACP Event Handler Wiring

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-03-07
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

- Spec contains an Assumptions section documenting that the outbound side (Slack button → driver → agent) is already functional. This feature only wires the inbound side (event → registration + persistence + Slack post).
- No [NEEDS CLARIFICATION] markers — the feature scope is well-defined from the backlog and codebase analysis.
- All items pass validation. Spec is ready for `/speckit.clarify` or `/speckit.plan`.

<!-- SECTION:DESCRIPTION:END -->
