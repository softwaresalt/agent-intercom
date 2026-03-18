# Behavior Scenarios: Slack UI Automated Testing

**Feature**: 008-slack-ui-testing | **Date**: 2026-03-09

## Tier 1 — Offline Structural Tests

### S-T1-001: Approval message block structure

**Given** a diff proposal with request ID "req:abc123", risk level "high", title "Add parser",
and a unified diff string,
**When** the approval message blocks are constructed via `command_approval_blocks()`,
**Then** the output contains:
- A section block with 🔐 emoji and "Terminal command approval requested" text
- A section block with the command in a code fence
- An actions block with `block_id` = `"approval_req:abc123"`
- Two buttons: `action_id` = `"approve_accept"` ("Accept") and `"approve_reject"` ("Reject"),
  both with `value` = `"req:abc123"`

**Traces to**: FR-001, FR-009

---

### S-T1-002: Prompt forwarding block structure

**Given** a prompt record with ID "prompt:xyz789", prompt text "Agent is idle", and type "continuation",
**When** the prompt message blocks are constructed,
**Then** the output contains:
- A section block with the prompt text
- A section block with the prompt type label
- An actions block with `block_id` = `"prompt_prompt:xyz789"`
- Three buttons: `"prompt_continue"` ("Continue"), `"prompt_refine"` ("Refine"),
  `"prompt_stop"` ("Stop"), each with `value` = `"prompt:xyz789"`

**Traces to**: FR-001, FR-009

---

### S-T1-003: Stall alert block structure

**Given** a stall alert with ID "stall:def456" and idle duration of 300 seconds,
**When** the stall alert blocks are constructed via `stall_alert_blocks()`,
**Then** the output contains:
- A section block with ⚠️ warning emoji and idle duration display
- An actions block with `block_id` = `"stall_stall:def456"`
- Three buttons: `"stall_nudge"` ("Nudge"), `"stall_nudge_instruct"` ("Nudge with Instructions"),
  `"stall_stop"` ("Stop"), each with `value` = `"stall:def456"`

**Traces to**: FR-001

---

### S-T1-004: Wait-for-instruction block structure

**Given** a session with ID "session:ghi012",
**When** the wait message blocks are constructed via `wait_buttons()`,
**Then** the output contains an actions block with `block_id` = `"wait_session:ghi012"` and
three buttons: `"wait_resume"` ("Resume"), `"wait_resume_instruct"` ("Resume with Instructions"),
`"wait_stop"` ("Stop Session"), each with `value` = `"session:ghi012"`

**Traces to**: FR-001

---

### S-T1-005: Session started notification structure

**Given** a session with ID "session:abc12345-...", protocol mode MCP, operational mode "remote",
workspace root "D:\\projects\\myapp", and creation timestamp 2026-03-09T10:00:00Z,
**When** `session_started_blocks()` is called,
**Then** the output contains:
- Session ID prefix ("abc12345…")
- Protocol mode "MCP"
- Operational mode "remote"
- Workspace root path
- Timestamp in "YYYY-MM-DD HH:MM UTC" format

**Traces to**: FR-001

---

### S-T1-006: Severity section emoji mapping

**Given** each severity level ("info", "success", "warning", "error"),
**When** `severity_section()` is called with each level,
**Then** the output uses the correct emoji prefix: ℹ️ (info), ✅ (success), ⚠️ (warning), ❌ (error)

**Traces to**: FR-001

---

### S-T1-007: Instruction modal structure

**Given** a callback ID "refine_prompt:xyz789", title "Refine Instructions",
and placeholder "Enter your instructions...",
**When** `instruction_modal()` is called,
**Then** the output is a modal view with:
- `callback_id` matching the input
- `title` as plain text
- A submit button labeled "Submit"
- A single input block with `block_id` = `"instruction_block"`,
  `action_id` = `"instruction_text"`, multiline enabled, and the placeholder text

**Traces to**: FR-001

---

### S-T1-008: Code snippet blocks structure

**Given** a list of snippets with labels and content,
**When** `code_snippet_blocks()` is called,
**Then** each snippet is rendered with its label as a header and content in a code block section

**Traces to**: FR-001

---

### S-T1-009: Simulated approval accept resolves blocking call

**Given** a pending approval request in the database with ID "req:test001" and a oneshot channel
registered in `AppState.pending_approvals`,
**When** a simulated "approve_accept" button action is dispatched with `value` = `"req:test001"`
from an authorized user,
**Then**:
- The oneshot channel receives `"approved"`
- The approval record in the database is updated to `status = "approved"`
- No panic or unhandled error occurs

**Traces to**: FR-002, FR-009

---

### S-T1-010: Simulated approval reject resolves blocking call

**Given** a pending approval request in the database with ID "req:test002" and a oneshot channel
registered in `AppState.pending_approvals`,
**When** a simulated "approve_reject" button action is dispatched with `value` = `"req:test002"`
from an authorized user,
**Then**:
- The oneshot channel receives `"rejected"`
- The approval record in the database is updated to `status = "rejected"`

**Traces to**: FR-002, FR-009

---

### S-T1-011: Simulated prompt continue resolves blocking call

**Given** a pending prompt in the database with ID "prompt:test003" and a oneshot channel
registered in `AppState.pending_prompts`,
**When** a simulated "prompt_continue" button action is dispatched,
**Then** the oneshot channel receives the continuation signal

**Traces to**: FR-002, FR-009

---

### S-T1-012: Simulated prompt refine triggers modal (API path)

**Given** a pending prompt with ID "prompt:test004",
**When** a simulated "prompt_refine" button action is dispatched,
**Then** the handler attempts to open a modal with `callback_id` = `"refine_prompt:test004"`.
When `state.slack = None`, the modal open is skipped and the thread-reply fallback path
is activated instead.

**Traces to**: FR-002, FR-011

---

### S-T1-013: Simulated modal submission resolves prompt with instructions

**Given** a pending prompt with ID "prompt:test005" and a registered oneshot channel,
**When** a simulated `ViewSubmission` event with `callback_id` = `"refine_prompt:test005"` and
`instruction_text` = `"Focus on error handling"` is dispatched,
**Then** the oneshot channel receives the instruction text

**Traces to**: FR-002, FR-009

---

### S-T1-014: Double-submission prevention

**Given** a pending approval request with ID "req:test006" and a registered oneshot channel,
**When** a simulated "approve_accept" action is dispatched twice for the same message,
**Then** the first dispatch resolves the approval, and the second dispatch is silently ignored
without error

**Traces to**: FR-004

---

### S-T1-015: Authorization guard rejects unauthorized user

**Given** an `AppState` with authorized user ID "U_OWNER",
**When** a simulated button action is dispatched from user ID "U_INTRUDER",
**Then** the authorization guard rejects the interaction and no state changes occur

**Traces to**: FR-003

---

### S-T1-016: Authorization guard allows authorized user

**Given** an `AppState` with authorized user ID "U_OWNER",
**When** a simulated button action is dispatched from user ID "U_OWNER",
**Then** the authorization guard allows the interaction to proceed

**Traces to**: FR-003

---

### S-T1-017: Thread-reply fallback routes to pending channel

**Given** a pending thread reply registered for thread_ts "1234567890.123456" in
`AppState.pending_thread_replies`,
**When** a thread reply message event arrives in that thread with text "Use retry logic",
**Then** the pending oneshot channel receives "Use retry logic"

**Traces to**: FR-011

---

### S-T1-018: Orphaned thread reply handled gracefully

**Given** NO pending thread reply registered for a given thread_ts,
**When** a thread reply message event arrives in that thread,
**Then** the event is ignored without error or panic

**Traces to**: FR-010, FR-012

---

### S-T1-019: Unknown action ID handled gracefully

**Given** a block action event with `action_id` = `"unknown_prefix_action"`,
**When** the event dispatcher routes the action,
**Then** the action is ignored or returns a descriptive error without panic

**Traces to**: FR-010, FR-012

---

### S-T1-020: Stale session reference in button action

**Given** a button action with `value` referencing a session ID that no longer exists in the database,
**When** the action handler is invoked,
**Then** the handler returns a graceful error (e.g., `AppError::NotFound`) without panic

**Traces to**: FR-010, FR-012

---

### S-T1-021: Malformed slash command arguments

**Given** a slash command `/acom steer` with no instruction text,
**When** the command handler is invoked,
**Then** the handler responds with a descriptive usage message without error

**Traces to**: FR-005, FR-012

---

### S-T1-022: Slash command mode gating — MCP command in MCP mode

**Given** the server is in MCP mode,
**When** a simulated `/acom steer "focus on tests"` command is dispatched by an authorized user,
**Then** the system accepts the command and the handler is invoked

**Traces to**: FR-005

---

### S-T1-023: Slash command mode gating — ACP-only command rejected in MCP mode

**Given** the server is in MCP mode,
**When** a simulated `session-start` command is dispatched,
**Then** the system responds with a mode-mismatch error message

**Traces to**: FR-005

---

### S-T1-024: Multi-session thread routing — correct session resolved

**Given** two active sessions in the same channel with different thread_ts values:
Session A (thread_ts = "111.000") and Session B (thread_ts = "222.000"),
**When** a button action arrives in Session A's thread (message thread_ts = "111.000"),
**Then** the router resolves to Session A and only Session A's state is modified

**Traces to**: FR-006

---

### S-T1-025: Nudge button action dispatches nudge to agent

**Given** a pending stall alert with ID "stall:test007" and a registered oneshot channel,
**When** a simulated "stall_nudge" button action is dispatched,
**Then** the stall alert is resolved and the nudge signal is sent to the agent

**Traces to**: FR-002, FR-009

---

### S-T1-026: Wait resume button action resolves standby

**Given** a session in standby with a registered wait channel,
**When** a simulated "wait_resume" button action is dispatched,
**Then** the standby is resolved and the session receives the resume signal

**Traces to**: FR-002, FR-009

---

### S-T1-027: Consumed oneshot channel handled gracefully

**Given** a prompt with ID "prompt:test008" whose oneshot channel has already been consumed
(e.g., by timeout),
**When** a simulated "prompt_continue" action arrives for that prompt,
**Then** the handler detects the consumed channel and returns gracefully without panic

**Traces to**: FR-010

---

## Tier 2 — Live Slack API Tests

### S-T2-001: Post and verify approval message in real Slack

**Given** a running server connected to the test Slack workspace,
**When** the test harness posts an approval request message to the test channel,
**Then** `conversations.history` returns the message with:
- Correct Block Kit structure (severity section + diff section + action buttons)
- Message timestamp is a valid Slack timestamp
- Message is in the correct channel

**Traces to**: FR-013

---

### S-T2-002: Post and verify threaded message

**Given** a session with a known thread_ts in the test channel,
**When** the server posts a broadcast message for that session,
**Then** `conversations.replies` for that thread_ts includes the new message

**Traces to**: FR-013, FR-018

---

### S-T2-003: Post messages to multiple session threads

**Given** two active sessions with distinct thread_ts values in the test channel,
**When** the server posts broadcast messages for each session,
**Then** each message appears in the correct thread (verified via `conversations.replies`),
not in the other session's thread

**Traces to**: FR-018

---

### S-T2-004: Approval accept round-trip

**Given** an approval request posted to the test channel with a pending oneshot,
**When** a synthetic "approve_accept" interaction payload is dispatched through the handler,
**Then**:
- The approval record status = "approved" in the database
- The blocking tool call resolves
- A follow-up confirmation message appears in the thread (verified via `conversations.replies`)

**Traces to**: FR-014

---

### S-T2-005: Prompt refine round-trip with modal

**Given** a prompt message posted to the test channel,
**When** a synthetic "prompt_refine" interaction payload is dispatched,
**Then**:
- `open_modal` is called with the correct callback_id
- The API returns success
- If followed by a synthetic `ViewSubmission`, the prompt resolves with the instruction text

**Traces to**: FR-014

---

### S-T2-006: Modal open for threaded button — API behavior

**Given** a prompt message posted **inside a thread** in the test channel,
**When** a synthetic "prompt_refine" interaction payload (with thread context) is dispatched,
**Then** the `views.open` API call result is captured and documented:
- Success → document that the API returned success (but this does NOT confirm rendering)
- Error → document the specific error code

**Traces to**: FR-015, FR-022

---

### S-T2-007: Modal open for top-level button — API baseline

**Given** a prompt message posted **as a top-level channel message** (not threaded),
**When** a synthetic "prompt_refine" interaction payload is dispatched,
**Then** `views.open` returns success, establishing the API-level baseline

**Traces to**: FR-016

---

### S-T2-008: Thread-reply fallback end-to-end

**Given** a prompt with "Refine" selected and modal opening skipped (simulating failure),
**When** the thread-reply fallback activates and a follow-up thread message arrives,
**Then** the prompt resolves with the text from the thread reply

**Traces to**: FR-017

---

### S-T2-009: Rate limit handling during test execution

**Given** the test harness is posting multiple messages in rapid succession,
**When** the Slack API returns a rate limit response,
**Then** the server's rate-limiting queue handles the backoff and the test eventually succeeds
without manual intervention

**Traces to**: FR-019, edge case (rate limits)

---

### S-T2-010: Stall alert → nudge round-trip in real Slack

**Given** a stall alert posted to the test channel,
**When** a synthetic "stall_nudge" interaction payload is dispatched,
**Then**:
- The stall alert is resolved
- A follow-up message confirms the nudge was sent

**Traces to**: FR-014

---

### S-T2-011: Wait-for-instruction → resume with instructions round-trip

**Given** a wait message posted to the test channel,
**When** a synthetic "wait_resume_instruct" interaction payload is dispatched,
**Then** the handler attempts to open a modal. The test documents the API result and
subsequent resolution path (modal or fallback).

**Traces to**: FR-014, FR-015

---

### S-T2-012: Slash command dispatched via test harness

**Given** the server is connected to the test workspace in MCP mode,
**When** the test harness constructs and dispatches a synthetic `/acom status` command event,
**Then** the command handler responds with a status message containing session information

**Traces to**: FR-014

---

### S-T2-013: Verify message update after button click (button replacement)

**Given** an approval message with interactive buttons posted to the test channel,
**When** the approval is accepted via synthetic interaction payload,
**Then** `conversations.history` shows the message has been updated — the action buttons
are replaced with a static status text (e.g., "✅ Approved by @user")

**Traces to**: FR-013, FR-014

---

## Tier 3 — Visual Browser-Automated Tests

### S-T3-001: Authentication and channel navigation

**Given** the Playwright browser session with stored authentication cookies,
**When** the test navigates to the Slack web client and the test channel,
**Then** a screenshot captures the channel view confirming successful login and channel navigation.
The channel name is visible in the header.

**Traces to**: FR-024, FR-032

---

### S-T3-002: Approval message visual rendering

**Given** the server has posted an approval request to the test channel,
**When** the browser displays the message,
**Then** a screenshot captures:
- The severity header with correct emoji
- The diff content in a code-fenced block (monospaced)
- Accept and Reject buttons visible and labeled
- Overall Block Kit layout renders correctly (not as raw JSON or broken markup)

**Traces to**: FR-026, FR-029

---

### S-T3-003: Prompt message visual rendering

**Given** the server has posted a prompt forwarding message,
**When** the browser displays the message,
**Then** a screenshot captures the prompt text, type indicator, and three interactive buttons
(Continue, Refine, Stop) all visible and correctly labeled

**Traces to**: FR-026

---

### S-T3-004: Stall alert visual rendering

**Given** the server has posted a stall alert,
**When** the browser displays the message,
**Then** a screenshot captures the ⚠️ warning emoji, idle duration text, and three buttons
(Nudge, Nudge with Instructions, Stop) all visible

**Traces to**: FR-026

---

### S-T3-005: Modal rendering from top-level button click (baseline)

**Given** a prompt message posted as a **top-level channel message** (not in a thread),
**When** the browser automation clicks the "Refine" button,
**Then** screenshots capture:
1. The message before clicking (showing the Refine button)
2. The modal dialog after clicking — with title, text input field, Submit button visible
3. The modal after typing test instructions into the input field

This establishes the visual baseline for correct modal behavior.

**Traces to**: FR-027, FR-028, FR-025

---

### S-T3-006: Modal rendering from threaded button click (diagnostic)

**Given** a prompt message posted **inside a thread**,
**When** the browser automation clicks the "Refine" button and waits the configurable timeout
(minimum 5 seconds),
**Then** screenshots capture:
1. The threaded message before clicking (showing the Refine button in thread view)
2. The UI state after clicking — either:
   - (a) Modal appeared → screenshot shows modal in thread context, or
   - (b) No modal → screenshot shows unchanged thread view after timeout elapsed

The test documents the result as either "modal renders in thread" or "modal silently fails in thread"
with visual evidence.

**Traces to**: FR-027, FR-028, FR-030, FR-022

---

### S-T3-007: Thread-reply fallback visual flow

**Given** modal-in-thread failure has been confirmed (S-T3-006 result (b)),
**When** the test triggers the thread-reply fallback:
1. Server posts fallback prompt in the thread
2. Browser automation types a reply in the thread composer
3. Reply is submitted

**Then** screenshots capture:
1. The fallback prompt message appearing in the thread
2. The reply being composed in the thread input
3. The thread after reply submission, showing the resolved state

**Traces to**: FR-028, FR-023

---

### S-T3-008: Button replacement visual transition

**Given** an approval message with interactive Accept/Reject buttons in the test channel,
**When** the browser automation clicks the "Accept" button,
**Then** screenshots capture:
1. The message before clicking (buttons visible)
2. The message after clicking (buttons replaced with static "✅ Approved by @user" text)

The visual transition from interactive to resolved state is documented.

**Traces to**: FR-027, FR-025

---

### S-T3-009: Session started notification visual rendering

**Given** the server posts a session-started notification,
**When** the browser displays the message,
**Then** a screenshot captures the session ID, protocol mode, operational mode, workspace root,
and timestamp — all readable and correctly formatted in the Slack web client

**Traces to**: FR-026

---

### S-T3-010: Code snippet review visual rendering

**Given** the server posts a code snippet review as a threaded reply,
**When** the browser navigates to the thread and displays the message,
**Then** a screenshot captures the code blocks rendering with monospaced font, proper indentation,
and language annotation visible

**Traces to**: FR-026

---

### S-T3-011: Wait-for-instruction modal from thread (diagnostic)

**Given** a wait message posted **inside a thread**,
**When** the browser automation clicks "Resume with Instructions" and waits the configurable timeout,
**Then** screenshots capture whether the modal renders, documenting this as a second data point
(alongside S-T3-006) for the modal-in-thread issue. If the modal fails, the test proceeds to
verify the thread-reply fallback (analogous to S-T3-007).

**Traces to**: FR-028, FR-022

---

### S-T3-012: Full scenario screenshot gallery and HTML report

**Given** a complete Tier 3 test run has executed all visual scenarios,
**When** the test report is generated,
**Then**:
- All screenshots are saved to `tests/visual/screenshots/` with naming convention
  `{scenario_id}_{step}_{description}_{timestamp}.png`
- An HTML report is generated with:
  - Chronologically ordered scenarios
  - Each scenario has pass/fail annotation
  - Screenshots inline with their step descriptions
  - Summary table of all scenarios with overall pass/fail status

**Traces to**: FR-029, SC-009

---

## Cross-Tier Verification Scenarios

### S-X-001: Modal-in-thread A/B comparison

**Given** Tier 2 (S-T2-006, S-T2-007) and Tier 3 (S-T3-005, S-T3-006) have both executed,
**When** results are compared,
**Then** the diagnostic report documents:
- API-level result: `views.open` success/error for threaded vs. non-threaded
- Visual result: modal rendered/not-rendered for threaded vs. non-threaded
- Root cause categorization: (a) platform limitation, (b) trigger_id scope, (c) timing/race,
  or (d) client version
- Recommended remediation path

**Traces to**: FR-022, SC-003

---

### S-X-002: Thread-reply fallback coverage verification

**Given** modal-in-thread failure is confirmed by S-X-001,
**When** all three modal-dependent paths are tested:
1. Refine (prompt handler)
2. Resume with Instructions (wait handler)
3. Reject with Reason (approval handler)

**Then** each path has verified fallback coverage:
- Tier 1: Simulated fallback dispatch resolves correctly (S-T1-012, S-T1-017)
- Tier 2: Live fallback round-trip works (S-T2-008)
- Tier 3: Visual evidence of fallback flow (S-T3-007)

**Traces to**: FR-023, SC-003

---

---

## Phase 11 — @-Mention Thread Reply Fix Automated Validation

### S-T3-AUTO-006: @-mention prompt text visible in seeded thread

**Given** a seeded thread anchor message with a prompt-with-Refine button posted as a
thread reply, and a seeded @-mention fallback prompt posted as a second thread reply,
**When** the Playwright browser opens the thread panel,
**Then** the @-mention prompt message is visible in the thread panel, the message text
contains the phrase `"mentioning @agent-intercom"`, and screenshots are captured at each step.

**File**: `tests/visual/scenarios/at-mention-thread-reply.spec.ts`  
**Traces to**: FR-033, FR-034, SC-011

---

### S-T3-AUTO-007: In-thread prompt has Refine button visible

**Given** a seeded thread prompt fixture with a Refine button posted inside a thread,
**When** the Playwright browser opens that thread panel,
**Then** the Refine button is visible in the thread panel (confirming the prompt Block Kit
structure renders correctly inside a thread pane), and a screenshot is captured.

**File**: `tests/visual/scenarios/at-mention-thread-reply.spec.ts`  
**Traces to**: FR-027, FR-033, SC-010

---

### S-T3-AUTO-008: @-mention prompt text contains bot mention pattern (static fixture)

**Given** a seeded @-mention thread fixture that posts the bot-mention fallback text,
**When** the Playwright browser opens the thread and locates the fallback message,
**Then** the message text contains `"@agent-intercom"` (the bot mention marker), and the
test passes as a static fixture validation that does not require a live server.

**File**: `tests/visual/scenarios/automated-harness.spec.ts`  
**Traces to**: FR-034, SC-011

---

### S-T3-HITL-001: Automated HITL — full forward_prompt cycle

**Given** the `agent-intercom` server is running and reachable at the configured health URL,
**When** the automated HITL harness posts a `forward_prompt` MCP call (or navigates to a
channel with an existing prompt message) and the Playwright browser clicks the Refine button
from inside a thread,
**Then** the @-mention fallback prompt appears in the thread within 15 seconds, and
screenshots confirm the complete operator-facing flow without manual intervention.

**File**: `scripts/run_automated_test_harness.ps1` (Suite: hitl)  
**Traces to**: FR-035, FR-036, FR-037, SC-011

---

### S-T3-HITL-002: Automated HITL — wait_for_instruction via thread reply

**Given** the `agent-intercom` server is running and a wait_for_instruction prompt has
been posted to the test channel in a thread,
**When** the automated HITL harness identifies the Resume with Instructions button in the
thread pane and clicks it,
**Then** the @-mention fallback prompt appears in the thread, and a test reply is typed
into the thread composer (visual confirmation step only; actual send is optional).

**File**: `scripts/run_automated_test_harness.ps1` (Suite: hitl)  
**Traces to**: FR-035, FR-036, FR-037, SC-011

---

| Scenario | FR Coverage | SC Coverage |
|---|---|---|
| S-T1-001 – S-T1-008 | FR-001 | SC-001 |
| S-T1-009 – S-T1-013 | FR-002, FR-009 | SC-002 |
| S-T1-014 | FR-004 | — |
| S-T1-015 – S-T1-016 | FR-003 | — |
| S-T1-017 – S-T1-018 | FR-010, FR-011 | — |
| S-T1-019 – S-T1-020 | FR-010, FR-012 | — |
| S-T1-021 | FR-005, FR-012 | SC-007 |
| S-T1-022 – S-T1-023 | FR-005 | SC-007 |
| S-T1-024 | FR-006 | — |
| S-T1-025 – S-T1-027 | FR-002, FR-009, FR-010 | SC-002 |
| S-T2-001 – S-T2-003 | FR-013, FR-018 | SC-008 |
| S-T2-004 – S-T2-005 | FR-014 | SC-002 |
| S-T2-006 – S-T2-007 | FR-015, FR-016, FR-022 | SC-003 |
| S-T2-008 | FR-017 | SC-003 |
| S-T2-009 | FR-019 | — |
| S-T2-010 – S-T2-013 | FR-013, FR-014 | SC-002, SC-006 |
| S-T3-001 | FR-024, FR-032 | — |
| S-T3-002 – S-T3-004, S-T3-009 – S-T3-010 | FR-026 | SC-010 |
| S-T3-005 – S-T3-006 | FR-027, FR-028, FR-030 | SC-003, SC-009 |
| S-T3-007 | FR-023, FR-028 | SC-003 |
| S-T3-008 | FR-025, FR-027 | SC-009 |
| S-T3-011 | FR-022, FR-028 | SC-003 |
| S-T3-012 | FR-029 | SC-009 |
| S-X-001 | FR-022 | SC-003 |
| S-X-002 | FR-023 | SC-003 |
| S-T3-AUTO-001 – S-T3-AUTO-005 | FR-026, FR-027, FR-028 | SC-009, SC-010 |
| S-T3-AUTO-006 – S-T3-AUTO-008 | FR-033, FR-034, FR-035 | SC-011 |
| S-T3-HITL-001 – S-T3-HITL-002 | FR-035, FR-036, FR-037 | SC-011 |

### Uncovered FRs

| FR | Gap | Mitigation |
|---|---|---|
| FR-007 | Implicit — all Tier 1 tests satisfy this by definition | Verified by SC-005 |
| FR-008 | Implicit — all Tier 1 tests run via `cargo test` | Verified by SC-004 |
| FR-020 | Implicit — Tier 2 test runner produces report | Verified by SC-006 |
| FR-021 | Implicit — feature gate + env vars | Verified by quickstart.md |
| FR-031 | Implicit — Tier 3 runs on demand | Verified by quickstart.md |
