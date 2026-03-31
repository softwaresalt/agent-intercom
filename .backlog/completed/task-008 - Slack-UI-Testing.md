---
id: TASK-008
title: "Slack UI Testing"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - feature
dependencies: []
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
# Feature Specification: Slack UI Automated Testing

**Feature Branch**: `008-slack-ui-testing`  
**Created**: 2026-03-09  
**Status**: Draft  
**Input**: User description: "Add automated testing framework for Slack channel interactions. Three-tier approach: (1) offline structural tests for CI, (2) live Slack API integration tests for message posting/threading/interaction verification, and (3) browser-automated visual tests with screenshots to capture actual Slack UI rendering, modal dialog behavior, and interactive component appearance — especially diagnosing the known modal-in-thread failure."

## Clarifications

### Session 2026-03-09

- Q: Should the feature include live Slack testing or only offline API-level tests? → A: Three-tier model: Tier 1 (offline structural tests, CI-safe), Tier 2 (live Slack API integration tests against a real test workspace), and Tier 3 (browser-automated visual tests with screenshots capturing actual Slack UI rendering).
- Q: Are there known issues with modal dialogs in threads? → A: Yes. Modal dialogs triggered by buttons (Refine, Resume with Instructions) inside Slack threads do not reliably render or accept input. The server's `open_modal` API call may succeed without error, but the modal silently fails to appear in the Slack client. The existing thread-reply fallback (F-16/F-17) only activates on API failure, not on silent client-side swallowing. This must be tested and diagnosed as part of this feature.
- Q: Should the test suite capture screenshots during interactions? → A: Yes. Screenshot capture during automated browser interactions is becoming a best practice for this kind of development workflow. Visual evidence is required to fully understand Slack UI behavior — especially for diagnosing issues like modals silently failing in threads, where API responses alone are misleading.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Verify Interactive Components Work in Real Slack (Priority: P1)

An operator triggers interactive components in a real Slack workspace — buttons that open modals (Refine, Resume with Instructions, Reject with Reason), buttons that resolve blocking calls (Approve, Continue, Stop, Nudge), and slash commands — and the test suite automatically verifies that each component behaves as intended: modals actually render and accept input, button presses resolve correctly, and follow-up messages appear in the right threads. Browser automation captures screenshots at each interaction step, providing visual evidence of what the operator actually sees.

**Why this priority**: There is a known issue where modal dialogs triggered from buttons inside Slack threads silently fail to render. The server returns success on `views.open`, but the operator never sees the modal. This class of bug is invisible to API-level testing — only browser-level visual verification can definitively prove whether the modal appeared. Verifying actual UI behavior is the highest-priority gap because it directly blocks operator workflows.

**Independent Test**: Requires a test Slack workspace with the app installed and a browser automation environment. Can be run independently by executing the visual test suite against a running server instance. Delivers immediate value by capturing screenshots that document the modal-in-thread issue and verifying all interactive components visually.

**Acceptance Scenarios**:

1. **Given** a prompt message with Refine button posted **in a thread**, **When** the browser automation clicks the Refine button, **Then** a screenshot is captured showing whether the modal dialog appeared, and the test documents the visual result (modal visible with input field, or no modal rendered).
2. **Given** a prompt message with Refine button posted **as a top-level channel message** (not in a thread), **When** the browser automation clicks the Refine button, **Then** a screenshot confirms the modal dialog renders correctly with the text input field visible.
3. **Given** an approval message posted in a session thread, **When** the browser automation clicks the Accept button, **Then** screenshots capture the button replacement (from interactive buttons to static status line) and the follow-up confirmation in the same thread.
4. **Given** a wait-for-instruction message in a thread, **When** the browser automation clicks "Resume with Instructions", **Then** a screenshot documents whether the modal appears. If it does not, a subsequent screenshot shows the thread-reply fallback prompt and the fallback response flow.
5. **Given** the modal-in-thread failure is confirmed visually, **When** the thread-reply fallback is triggered, **Then** screenshots capture the fallback prompt appearing in the thread and the operator's reply resolving the pending operation.

---

### User Story 2 — Validate Message Posting, Formatting, and Threading in Real Slack (Priority: P2)

An operator views messages posted by the server in a real Slack channel and the test suite automatically verifies that messages appear correctly: Block Kit formatting renders as intended, messages land in the correct threads, severity indicators display the right emoji, code blocks render as monospaced text, and buttons are interactive.

**Why this priority**: The server constructs Block Kit payloads that may be structurally valid but render incorrectly in the Slack client — truncated text, broken markdown, invisible code blocks, or misthreaded messages. Only live Slack verification catches these rendering issues. This is the second-highest priority because message readability is essential for operator situational awareness.

**Independent Test**: Requires a test Slack workspace. Can be run independently by posting a representative set of messages and verifying them via Slack's conversation history API.

**Acceptance Scenarios**:

1. **Given** an approval request is posted to the test channel, **When** the test queries the channel history, **Then** the message contains the expected diff content, severity header, and two interactive buttons (Accept/Reject).
2. **Given** multiple sessions are active with distinct threads, **When** broadcast messages are sent for each session, **Then** each message appears in the correct thread (verified via conversation replies API), not as a top-level message or in the wrong thread.
3. **Given** a stall alert is posted, **When** the test queries the message, **Then** the warning emoji, idle duration text, and three buttons (Nudge/Nudge with Instructions/Stop) are all present and interactive.
4. **Given** a code snippet review is posted as a threaded reply, **When** the test retrieves the thread, **Then** code blocks render with proper formatting (backtick fences intact, language annotation present).
5. **Given** a session-started notification, **When** the test retrieves the message, **Then** it contains the session ID prefix, protocol mode, operational mode, workspace root, and timestamp — all readable and correctly formatted.

---

### User Story 3 — Visual Screenshot Capture During Automated Interactions (Priority: P3)

The test suite automates a browser session against the Slack web client, navigates to the test channel, and captures timestamped screenshots at each step of an operator interaction flow — before a button is clicked, after the click, during modal rendering (or absence), after modal submission, and after message updates. Screenshots are saved to a test artifacts directory and included in the test report.

**Why this priority**: Screenshots provide the definitive visual record of what an operator actually sees. API-level tests (Tier 2) can confirm a message was posted and a `views.open` call succeeded, but only a screenshot proves whether the modal actually rendered in the browser, whether the Block Kit formatting looks correct, and whether button replacements visually updated. This is an emerging best practice for HITL-adjacent development workflows where the gap between "API says success" and "user sees the right thing" is the primary source of bugs.

**Independent Test**: Requires a browser automation runtime and a test Slack workspace. Can be run independently to produce a visual test report. Delivers value as both a diagnostic tool (modal-in-thread investigation) and an ongoing visual regression baseline.

**Acceptance Scenarios**:

1. **Given** the visual test suite is invoked, **When** the browser navigates to the test Slack channel, **Then** a screenshot captures the channel view confirming successful login and navigation.
2. **Given** the server posts an approval request message, **When** the browser displays the message, **Then** a screenshot captures the rendered Block Kit layout including the diff section, severity header, and Accept/Reject buttons — verifying they appear as intended (not as raw JSON or broken formatting).
3. **Given** a Refine button is clicked in a threaded message, **When** the browser waits for the modal to appear, **Then** a screenshot is captured after a configurable timeout documenting whether the modal rendered. If no modal appeared, the screenshot shows the unchanged thread view (providing visual evidence of the silent failure).
4. **Given** a Refine button is clicked in a top-level (non-threaded) message, **When** the modal appears, **Then** a screenshot captures the modal with its title, text input field, and Submit button — establishing the visual baseline for correct modal behavior.
5. **Given** the operator submits text via a modal (or thread-reply fallback), **When** the interaction completes, **Then** a screenshot captures the updated message showing the static status replacement (e.g., "✏️ Refine selected by @user"), confirming the visual transition from interactive buttons to resolved state.
6. **Given** a complete test run, **When** all scenarios have executed, **Then** the test report includes a chronologically ordered gallery of all captured screenshots with timestamps, scenario labels, and pass/fail annotations.

---

### User Story 4 — Offline Structural Test Suite for Block Kit Payloads (Priority: P4)

A developer runs `cargo test` and the test suite automatically verifies that all Block Kit message types produce correctly structured payloads with the expected blocks, text content, buttons, severity indicators, and threading metadata. No real Slack workspace or credentials are required.

**Why this priority**: Offline tests run in CI on every pull request, catching structural regressions immediately. While they cannot verify rendering behavior (Tiers 2–3), they provide fast feedback on code changes that break message construction. This is the foundation that prevents obvious errors from reaching live testing.

**Independent Test**: Runs as part of `cargo test` with no external dependencies. Delivers value by covering every Block Kit builder function with regression tests.

**Acceptance Scenarios**:

1. **Given** the test suite is invoked, **When** approval message blocks are generated for a diff proposal, **Then** the output contains a severity-formatted header, a code-fenced diff section, and Accept/Reject action buttons with the correct `action_id` prefixes and `request_id` values.
2. **Given** a prompt forwarding event, **When** the message blocks are generated, **Then** the output contains the prompt text, prompt type indicator, and Continue/Refine/Stop action buttons.
3. **Given** a stall alert event, **When** the alert blocks are generated, **Then** the output contains a warning-severity section with idle duration and Nudge/Nudge with Instructions/Stop action buttons.
4. **Given** a session started event, **When** the session notification blocks are generated, **Then** the output contains the session ID prefix, protocol mode (MCP/ACP), operational mode, workspace root, and creation timestamp.
5. **Given** a log broadcast at each severity level (info, success, warning, error), **When** the message blocks are generated, **Then** each uses the correct emoji prefix and formatting per the severity level.

---

### User Story 5 — Offline Simulation of Operator Interactions (Priority: P5)

A developer runs `cargo test` and the test suite simulates the complete operator interaction cycle using synthetic payloads: the server receives a simulated button press or modal submission, and the system processes the response correctly — updating state, resolving blocking tool calls, and queueing follow-up messages.

**Why this priority**: Simulated interaction tests run offline in CI and catch regressions in the dispatch pipeline, authorization guard, double-submission prevention, and state transitions. While they don't verify actual Slack rendering (Tier 2), they ensure the server-side logic is correct.

**Independent Test**: Runs as part of `cargo test` with no external dependencies. Tests construct synthetic Slack event payloads and dispatch them through the handler pipeline.

**Acceptance Scenarios**:

1. **Given** a pending approval request, **When** a simulated "Accept" button action is dispatched, **Then** the approval status is updated to "approved", the blocking tool call resolves with `status: "approved"`, and a confirmation message is queued.
2. **Given** a pending approval request, **When** a simulated "Reject" button action is dispatched, **Then** the approval status is updated to "rejected" and the blocking tool call resolves with `status: "rejected"`.
3. **Given** a pending prompt, **When** a simulated "Continue" button action is dispatched, **Then** the prompt resolves with the continuation signal.
4. **Given** a pending prompt with "Refine" selected, **When** a simulated modal submission event is dispatched, **Then** the prompt resolves with the operator's instruction text.
5. **Given** a button already processed (double-submission scenario), **When** a second identical button action arrives, **Then** the system ignores the duplicate without error.
6. **Given** an unauthorized user, **When** any button action is dispatched, **Then** the authorization guard rejects the interaction.

---

### User Story 6 — Validate Slash Command Routing and Responses (Priority: P6)

A developer runs `cargo test` and the test suite verifies that all slash commands (`/acom` for MCP, `/arc` for ACP) are correctly routed, authorized, and produce the expected response messages — including session management commands, steering, task submission, and file browsing.

**Why this priority**: Slash commands are the operator's primary text-based interface. Routing errors, authorization failures, or incorrect mode gating can silently break operator workflows.

**Independent Test**: Can be tested by constructing slash command event payloads and invoking the command handler, then asserting the response content and format.

**Acceptance Scenarios**:

1. **Given** the server is in MCP mode, **When** a simulated `/acom steer "focus on tests"` command is dispatched by an authorized user, **Then** the system accepts the steering instruction and responds with a confirmation message.
2. **Given** the server is in ACP mode, **When** a simulated `/arc session-start` command is dispatched, **Then** the system initiates a new ACP session and responds with session details.
3. **Given** an unauthorized Slack user, **When** any slash command is dispatched, **Then** the system rejects the command with an appropriate unauthorized response.
4. **Given** the server is in MCP mode, **When** an ACP-only command (`session-start`) is dispatched, **Then** the system responds with a mode-mismatch error.

---

### User Story 7 — Verify Multi-Session and Threading Behavior (Priority: P7)

A developer runs `cargo test` (offline) and the live test suite (online) to verify that messages are correctly threaded to the right session when multiple sessions are active, that session-scoped interactions only affect the intended session, and that orphaned threads are handled gracefully.

**Why this priority**: Multi-session support is a core ACP feature. Incorrect threading can route operator responses to the wrong session. This is nearly impossible to test manually with precision.

**Independent Test**: Offline tests verify routing logic with synthetic thread timestamps. Live tests verify actual Slack thread behavior.

**Acceptance Scenarios**:

1. **Given** two active sessions in the same channel with different thread timestamps, **When** an approval button is pressed in Session A's thread, **Then** only Session A's approval is resolved and Session B remains unaffected.
2. **Given** a session with a thread timestamp, **When** a broadcast message is sent for that session, **Then** the message is posted as a reply in the correct thread.
3. **Given** a session that has been terminated, **When** a button action arrives for that session's thread, **Then** the system handles the stale interaction gracefully.

---

### User Story 8 — Automated Regression Suite Runs in CI (Priority: P8)

The offline test suite (Tier 1) runs as part of the project's CI pipeline alongside existing tests. The live API test suite (Tier 2) and visual browser test suite (Tier 3) run on-demand against a configured test Slack workspace.

**Why this priority**: CI-gated tests prevent regressions from merging. Live and visual tests provide deeper validation but require credentials and a browser environment — they are run on-demand before releases or after significant Slack-facing changes.

**Independent Test**: Tier 1 validated by confirming `cargo test` passes in a clean CI environment. Tiers 2–3 validated by running the live/visual suites with test workspace credentials and a browser runtime.

**Acceptance Scenarios**:

1. **Given** a clean CI environment with no Slack credentials, **When** `cargo test` is executed, **Then** all Tier 1 (offline) Slack UI tests pass without errors or skips.
2. **Given** a test Slack workspace is configured, **When** the Tier 2 live test suite is invoked, **Then** all API-level scenarios execute and produce a pass/fail report without requiring human intervention.
3. **Given** a test Slack workspace and browser runtime are configured, **When** the Tier 3 visual test suite is invoked, **Then** all visual scenarios execute, screenshots are captured, and a report is produced with pass/fail annotations alongside screenshot evidence.
4. **Given** a change that breaks a Block Kit message format, **When** `cargo test` is executed in CI, **Then** the relevant Tier 1 test fails with a clear error message.
5. **Given** the full Tier 1 test suite, **When** `cargo test` is executed, **Then** total test execution time does not increase by more than 30 seconds compared to the baseline.

---

### User Story 9 — Automated @-Mention Thread Reply Validation (Priority: P1)

The automated test harness validates the @-mention thread reply fix (commit `480aaab`) end-to-end
using Playwright browser automation. When a Refine or Resume with Instructions button is clicked
inside a Slack thread, the server now proactively skips `views.open` and instead posts an
`@agent-intercom` mention prompt in the thread. The operator replies with `@agent-intercom
<instructions>` and the server routes the stripped text to the pending prompt waiter. This entire
path is exercised automatically — without manual operator clicks — by the Playwright harness using
self-seeded fixture messages.

**Why this priority**: The @-mention fix resolves the highest-severity UX blocker (silent modal
suppression in threads, confirmed in Phase 9). Without automated coverage, regressions to the fix
would be invisible until a manual HITL pass. The self-seeding fixture approach means this test runs
without a prior HITL session — it seeds its own @-mention prompt message and verifies it visually.

**Independent Test**: Requires the same env configuration as the existing automated harness
(`SLACK_BOT_TOKEN`, `SLACK_WORKSPACE_URL`, `SLACK_TEST_CHANNEL`, `SLACK_TEST_CHANNEL_ID`).
Runs as part of `npm run test:at-mention` or `npm run test:automated`. A `-Suite hitl` mode in
`run_automated_test_harness.ps1` orchestrates the live end-to-end path when the server is running.

**Acceptance Scenarios**:

1. **Given** a seeded thread with an @-mention prompt (text contains `"mentioning @agent-intercom"`),
   **When** the Playwright harness opens the thread panel,
   **Then** the @-mention prompt text is visible in the thread, screenshots are captured, and the
   test passes (S-T3-AUTO-006).
2. **Given** a seeded in-thread prompt with a Refine button,
   **When** the Playwright harness opens the thread panel,
   **Then** the Refine button is visible inside the thread pane (S-T3-AUTO-007).
3. **Given** a seeded @-mention fixture in the automated harness,
   **When** the Playwright harness locates the @-mention prompt message,
   **Then** the text contains `"@agent-intercom"` as a static fixture assertion (S-T3-AUTO-008).
4. **Given** the `agent-intercom` server is running and reachable,
   **When** the `-Suite hitl` automated harness runs,
   **Then** the @-mention Playwright spec executes and reports PASS or SKIP (not FAIL) depending
   on server availability (S-T3-HITL-001, S-T3-HITL-002).

---

### Edge Cases

- What happens when a Slack interaction payload references a session ID that no longer exists in the database?
- How does the system handle a modal submission event when the corresponding oneshot channel has already been consumed (timeout or duplicate)?
- What happens when a button action payload contains an unrecognized `action_id` prefix?
- How does the system behave when the Block Kit payload exceeds Slack's 50-block or 3,000-character-per-block limits?
- What happens when a slash command is dispatched with malformed or missing arguments?
- How does the system handle a thread reply fallback response when no pending fallback is registered for that thread?
- What happens when `views.open` returns success but the modal never renders in the Slack client (silent swallow)?
- How does the system detect and recover when a modal is silently swallowed in a threaded context?
- What happens when the test Slack workspace has rate limits that slow down live test execution?
- What happens when the Slack web client DOM structure changes between versions, breaking browser automation selectors?
- How does the visual test suite handle Slack login flow (including potential 2FA or SSO requirements)?
- What happens when browser automation detects a modal did not render within the timeout — how is this distinguished from slow rendering vs. silent failure?
- How does the test suite handle Slack client updates that change the visual appearance of Block Kit components (visual regression without functional regression)?

## Requirements *(mandatory)*

### Functional Requirements

**Tier 1 — Offline Structural Tests (CI-safe)**

- **FR-001**: The test suite MUST validate the structure and content of every Block Kit message type produced by the server (approval requests, prompt forwarding, stall alerts, session lifecycle notifications, log broadcasts, diff applied/conflict notifications, code snippet reviews, command approval blocks, auto-approve suggestion buttons, session summary blocks, and instruction modals).
- **FR-002**: The test suite MUST simulate Slack interactive payloads (button presses and modal submissions) and verify that the event dispatcher routes them to the correct handler.
- **FR-003**: The test suite MUST verify that the authorization guard rejects interactions from unauthorized users and allows interactions from authorized users.
- **FR-004**: The test suite MUST verify double-submission prevention: when a button action is dispatched, subsequent identical actions for the same message MUST be ignored.
- **FR-005**: The test suite MUST validate slash command routing for both MCP (`/acom`) and ACP (`/arc`) prefixes, including mode gating.
- **FR-006**: The test suite MUST verify that messages are threaded correctly when multiple sessions are active in the same channel.
- **FR-007**: Tier 1 tests MUST run without any external Slack connectivity, credentials, or human operator interaction — all Slack API behavior MUST be simulated within the test harness.
- **FR-008**: Tier 1 tests MUST integrate with the existing test infrastructure (`cargo test`) and execute as part of the standard test run.
- **FR-009**: The test suite MUST verify that blocking tool calls (approval, prompt, wait-for-instruction) resolve correctly when simulated operator interactions are dispatched.
- **FR-010**: The test suite MUST validate that stale or orphaned interactions are handled gracefully without panics or unhandled errors.
- **FR-011**: The test suite MUST verify that the thread-reply fallback mechanism correctly routes in-thread text responses to the appropriate pending channel when modal submission is unavailable.
- **FR-012**: The test suite MUST cover error paths: malformed payloads, unknown action IDs, missing session references, and command parsing failures.

**Tier 2 — Live Slack Integration Tests**

- **FR-013**: The live test suite MUST post messages to a real Slack test channel and verify their presence and structure via the Slack conversation history API.
- **FR-014**: The live test suite MUST verify that interactive buttons (Approve, Reject, Continue, Refine, Stop, Nudge, Resume with Instructions) produce the correct server-side behavior when triggered in the Slack client.
- **FR-015**: The live test suite MUST specifically test modal dialogs triggered from buttons posted inside Slack threads and document whether the modal renders, fails silently, or produces an error.
- **FR-016**: The live test suite MUST test modal dialogs triggered from buttons posted as top-level channel messages (non-threaded) to establish a behavioral baseline.
- **FR-017**: The live test suite MUST verify that the thread-reply fallback mechanism works end-to-end in real Slack when modal dialogs fail — including posting the fallback prompt and capturing the operator's in-thread reply.
- **FR-018**: The live test suite MUST verify message threading in real Slack: messages for a given session appear in the correct thread, not as top-level messages or in other sessions' threads.
- **FR-019**: The live test suite MUST execute without human intervention — all operator actions (button presses, modal submissions, thread replies) are automated by the test harness.
- **FR-020**: The live test suite MUST produce a structured pass/fail report for each test scenario, including screenshots or message content snapshots for failed scenarios.
- **FR-021**: The live test suite MUST be runnable on-demand (not part of standard `cargo test`) and gated behind a configuration flag or environment variable that provides test workspace credentials.

**Modal Behavior Diagnostics**

- **FR-022**: The test suite MUST diagnose and document the specific failure mode when `views.open` succeeds but the modal does not render in the Slack client for threaded messages. This includes identifying whether the issue is: (a) a Slack platform limitation, (b) a `trigger_id` scope issue with threaded block_action events, (c) a timing/race condition, or (d) a specific Slack client version issue (desktop vs. mobile vs. web).
- **FR-023**: If modal-in-thread failure is confirmed, the test suite MUST verify that the existing thread-reply fallback (F-16/F-17) activates reliably as an alternative input method and that all modal-dependent interactions have a working fallback path.

**Tier 3 — Visual Browser-Automated Tests**

- **FR-024**: The visual test suite MUST automate a browser session against the Slack web client (`app.slack.com`), authenticate as a test operator, and navigate to the test channel.
- **FR-025**: The visual test suite MUST capture timestamped screenshots at each significant interaction step — before button clicks, after button clicks, during modal rendering (or absence), after modal submission or fallback, and after message updates.
- **FR-026**: The visual test suite MUST visually verify that Block Kit messages render as intended in the Slack web client — correct emoji display, monospaced code blocks, properly formatted markdown, interactive buttons with correct labels.
- **FR-027**: The visual test suite MUST click interactive buttons in the Slack web client (not via API simulation) and capture the resulting UI state, including modal appearance or absence, button replacement animations, and follow-up message rendering.
- **FR-028**: The visual test suite MUST compare modal rendering behavior between threaded messages and top-level messages by capturing screenshots of both scenarios, providing definitive visual evidence of the modal-in-thread issue.
- **FR-029**: The visual test suite MUST save all screenshots to a test artifacts directory with a consistent naming convention (scenario ID, step number, timestamp) and generate a browsable HTML report linking screenshots to their scenarios.
- **FR-030**: The visual test suite MUST support a configurable wait timeout for modal detection — waiting long enough to distinguish "slow rendering" from "silent failure" with a recommended minimum of 5 seconds.
- **FR-031**: The visual test suite MUST be runnable on-demand (not part of standard `cargo test`) and gated behind environment configuration that provides test workspace credentials and browser automation runtime availability.
- **FR-032**: The visual test suite MUST handle Slack web client authentication, including session persistence across test runs to avoid repeated login overhead.

- **FR-033**: When a block_action event originates from a Slack thread context (the `trigger_id` is associated with a threaded message), the server MUST skip `views.open` entirely and instead invoke the thread-reply fallback path, posting an @-mention prompt directly in the thread.
- **FR-034**: The @-mention fallback message posted in the thread MUST include the bot's Slack @-mention so the operator knows to use it in their reply. The message MUST instruct the operator to reply using the mention syntax (e.g., `"🤖 Please type your instructions as a reply mentioning @agent-intercom"`).
- **FR-035**: When an `AppMention` event arrives for a message that is a thread reply (the event has a `thread_ts` field), the server MUST check `pending_thread_replies` before routing to steering. If a pending entry exists for the `(channel_id, thread_ts)` pair and the sender is the authorized user, the server MUST strip the `<@BOTID>` mention prefix and deliver the remaining text to the pending waiter's oneshot channel.
- **FR-036**: The mention stripping operation MUST produce text that exactly equals the operator's input with only the leading `<@BOTID>` and any surrounding whitespace removed — no further modification of the instruction text is permitted.
- **FR-037**: The automated test harness MUST provide a `-Suite hitl` mode in `scripts/run_automated_test_harness.ps1` that orchestrates the @-mention thread reply flow end-to-end using Playwright browser automation, verifying the flow without manual operator intervention.

### Key Entities

- **Test Scenario**: A named, self-contained test case that exercises a specific Slack UI interaction path. Attributes: scenario ID, tier (1, 2, or 3), description, preconditions, simulated/live/visual actions, expected outcomes.
- **Simulated Interaction**: A synthetic Slack event payload (block_action, view_submission, or command event) constructed by the test harness. Used in Tier 1 tests. Attributes: event type, user ID, action ID, payload data, target message/thread.
- **Live Interaction**: A real Slack API call or webhook-simulated action performed against a test workspace. Used in Tier 2 tests. Attributes: API method, channel, thread_ts, trigger mechanism, expected Slack response.
- **Visual Interaction**: A browser-automated click, text input, or navigation action performed against the Slack web client. Used in Tier 3 tests. Attributes: DOM selector strategy, action type, pre/post screenshots, visual assertion criteria.
- **Expected Message**: An assertion about a message posted to Slack, covering structure (block types, action IDs), content (text patterns, emoji), and placement (channel, thread). Used in all tiers.
- **Screenshot Artifact**: A timestamped image captured during a Tier 3 visual test step. Attributes: scenario ID, step number, timestamp, file path, pass/fail annotation, description of what is being verified.
- **Modal Diagnostic Result**: The documented outcome of a modal-in-thread test, categorizing the failure mode and recommending a remediation path. Includes both API-level evidence (Tier 2) and visual evidence (Tier 3 screenshots).

## Assumptions

- A dedicated Slack test workspace (or test channel in the existing workspace) is available for Tier 2 and Tier 3 tests, with the agent-intercom Slack app installed and configured.
- The existing `blocks.rs` functions are the single source of truth for all Slack Block Kit construction and can be tested in isolation (Tier 1) by passing domain model inputs and asserting the returned structures.
- The Slack Web API (`conversations.history`, `conversations.replies`) provides sufficient inspection capability to verify message posting, threading, and content for Tier 2 API-level tests.
- The existing `test_helpers.rs` infrastructure (in-memory SQLite, mock `AppState`) is sufficient as the foundation for Tier 1 offline tests.
- Tier 2 live tests simulate "operator" actions by constructing interaction payloads that mimic what the Slack client sends to the server's Socket Mode handler — not by automating the Slack client UI itself.
- Tier 3 visual tests automate the Slack web client (`app.slack.com`) via browser automation. The Slack web client is a standard web application whose interactive elements (buttons, modals, text inputs) can be targeted by browser automation, though DOM selectors may need periodic maintenance as Slack updates its client.
- Browser automation can authenticate with the Slack web client and persist sessions to avoid repeated login overhead. Authentication may require a dedicated test Slack account without 2FA, or a session token–based approach.
- The modal-in-thread issue may be a Slack platform limitation rather than an agent-intercom bug, in which case the remediation is to ensure the thread-reply fallback reliably covers all modal-dependent interactions. Tier 3 screenshots will provide the definitive evidence.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every Block Kit message builder function in `blocks.rs` (currently 15+ functions) has at least one corresponding Tier 1 automated test that validates its output structure.
- **SC-002**: All six operator interaction types (approve, reject, continue, refine, stop, nudge) have Tier 1 simulated tests, Tier 2 live tests, and Tier 3 visual tests verifying the complete round-trip at each level.
- **SC-003**: The modal-in-thread issue is diagnosed with a documented root cause, categorized failure mode, and visual evidence (Tier 3 screenshots comparing threaded vs. non-threaded modal behavior). If confirmed as a platform limitation, all three modal-dependent paths (Refine, Resume with Instructions, Reject with Reason) have verified fallback coverage.
- **SC-004**: The Tier 1 (offline) test suite executes in under 30 seconds as part of `cargo test` with no external dependencies.
- **SC-005**: The Tier 1 test suite runs successfully in CI without Slack credentials or network access.
- **SC-006**: The Tier 2 (live API) test suite executes all scenarios without human intervention and produces a structured report with pass/fail status for each scenario.
- **SC-007**: Every slash command subcommand (steer, task, list-files, show-file, session-start, session-stop, session-restart, sessions, checkpoint, status) has at least one Tier 1 routing and response format test.
- **SC-008**: Tier 2 tests verify that messages posted to threaded conversations actually appear in the correct thread when queried via the Slack API.
- **SC-009**: The Tier 3 (visual) test suite captures screenshots for every interaction scenario and produces a browsable HTML report with scenario labels, pass/fail annotations, and chronologically ordered screenshot evidence.
- **SC-010**: Tier 3 screenshots visually confirm that Block Kit messages render correctly in the Slack web client — buttons are visible and labeled, code blocks are monospaced, emoji indicators display correctly, and severity formatting is distinguishable.
- **SC-011**: The automated harness validates the @-mention thread reply fix end-to-end: the seeded @-mention prompt fixture is visually confirmed in the Playwright browser (S-T3-AUTO-006, S-T3-AUTO-008), the Refine button is visible in the seeded thread prompt (S-T3-AUTO-007), and the `-Suite hitl` orchestration script executes without error (PASS or SKIP per server availability).


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


# Data Model: Slack UI Automated Testing

**Feature**: 008-slack-ui-testing | **Date**: 2026-03-09

## Overview

This feature adds test infrastructure — no new persistent entities or schema changes. The data model describes the test-time structures used across the three tiers.

## Test Infrastructure Entities

### TestScenario (conceptual — no Rust struct needed)

Represented as individual `#[test]` or `#[tokio::test]` functions in Rust (Tiers 1–2) and as `.spec.ts` files in Playwright (Tier 3).

| Attribute | Type | Description |
|---|---|---|
| scenario_id | string | Unique identifier (e.g., `T1_blocks_approval`, `T2_live_modal_thread`) |
| tier | 1 \| 2 \| 3 | Which testing tier |
| preconditions | text | Required state before execution |
| expected_outcome | text | What constitutes pass/fail |

### BlockKitAssertion (Tier 1 test helper)

A reusable assertion utility for verifying Block Kit JSON structure.

| Attribute | Type | Description |
|---|---|---|
| blocks | `Vec<SlackBlock>` | The Block Kit payload to verify |
| expected_block_types | `Vec<&str>` | Expected sequence of block types (section, actions, divider) |
| expected_action_ids | `Vec<&str>` | Expected `action_id` values in actions blocks |
| expected_text_patterns | `Vec<&str>` | Substring patterns that must appear in text content |

### LiveTestConfig (Tier 2 runtime configuration)

Read from environment variables at test startup.

| Attribute | Source | Description |
|---|---|---|
| bot_token | `SLACK_TEST_BOT_TOKEN` | Bot token for the test workspace |
| app_token | `SLACK_TEST_APP_TOKEN` | App token for Socket Mode |
| channel_id | `SLACK_TEST_CHANNEL_ID` | Dedicated test channel |
| authorized_user_id | `SLACK_TEST_USER_ID` | User ID for authorized interaction tests |

### VisualTestConfig (Tier 3 — in `playwright.config.ts`)

| Attribute | Source | Description |
|---|---|---|
| slack_workspace_url | env `SLACK_TEST_WORKSPACE_URL` | Slack workspace URL (e.g., `https://myworkspace.slack.com`) |
| slack_email | env `SLACK_TEST_EMAIL` | Test account email |
| slack_password | env `SLACK_TEST_PASSWORD` | Test account password |
| channel_name | env `SLACK_TEST_CHANNEL_NAME` | Channel name for navigation |
| screenshot_dir | config | Output directory for screenshots |
| modal_wait_timeout | config | Seconds to wait for modal detection (default: 5) |

## Existing Entities Used (no changes)

These existing domain entities are test targets — not modified by this feature:

- **ApprovalRequest** (`models/approval.rs`) — tested via approval flow scenarios
- **PromptRecord** (`models/prompt.rs`) — tested via prompt interaction scenarios
- **Session** (`models/session.rs`) — tested via session lifecycle and threading scenarios
- **StallAlert** (`models/stall.rs`) — tested via stall alert scenarios
- **AppState** (`mcp/handler.rs`) — constructed in test helpers with in-memory DB

## State Transitions Tested

The test suite verifies these existing state transitions (no new transitions added):

```
ApprovalRequest: pending → approved | rejected
PromptRecord: pending → continue | refine | stop
StallAlert: active → nudged | resolved
Session: created → active → paused → terminated
```

## No Schema Changes

This feature adds no database tables, columns, or migrations. All test data is created in-memory and discarded after each test.


# Quickstart: Slack UI Automated Testing

**Feature**: 008-slack-ui-testing

## Tier 1 — Offline Tests (CI-safe)

```powershell
# Runs automatically with the standard test suite
cargo test
```

No configuration needed. All Tier 1 tests use in-memory SQLite and mock AppState.

## Tier 2 — Live Slack API Tests

### Prerequisites

1. A Slack workspace with the agent-intercom app installed
2. A dedicated test channel (e.g., `#intercom-test`)
3. Bot and app tokens with appropriate scopes

### Configuration

Set environment variables:

```powershell
$env:SLACK_TEST_BOT_TOKEN = "xoxb-..."
$env:SLACK_TEST_CHANNEL_ID = "C_TEST_CHANNEL"
```

### Run

```powershell
cargo test --features live-slack-tests
```

## Tier 3 — Visual Browser Tests

### Prerequisites

1. Node.js 18+ installed
2. A dedicated Slack test account (email/password login, no 2FA)
3. The agent-intercom server running and connected to the test workspace

### Setup

```powershell
cd tests/visual
npm install
npx playwright install chromium
```

### Configuration

Copy `tests/visual/.env.example` to `tests/visual/.env` and fill in the values,
or export the variables directly:

```powershell
$env:SLACK_WORKSPACE_URL = "https://myworkspace.slack.com"
$env:SLACK_EMAIL = "test@example.com"
$env:SLACK_PASSWORD = "..."
$env:SLACK_TEST_CHANNEL = "agent-intercom-test"
```

### First Run (authenticates and persists session)

```powershell
npx playwright test --project=setup
```

### Run Visual Tests

```powershell
npx playwright test
```

### View Report

```powershell
npx playwright show-report reports/
```

Screenshots are saved to `tests/visual/screenshots/`.




---

## Checklists

# Specification Quality Checklist: Slack UI Automated Testing

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-03-09
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

- Spec references existing domain concepts (Block Kit, Socket Mode, action_id patterns) for precision — these are product domain terms, not implementation choices.
- The Assumptions section documents reasonable defaults about testability approach without prescribing specific frameworks or tools.
- All items pass validation. Ready for `/speckit.clarify` or `/speckit.plan`.

---

## Final Phase 10 Pass/Fail Status

*Updated: 2026-03-09 — Phase 10 (Report Generation & CI Integration) complete.*

| Success Criterion | Status | Evidence |
|---|---|---|
| **SC-001** Every Block Kit builder has ≥ 1 Tier 1 test | ✅ PASS | Phase 1: 6 new test files; all 15+ builders covered |
| **SC-002** All 6 interaction types have Tier 1 + Tier 2 + Tier 3 tests | ✅ PASS | Phases 2–3 (Tier 1), Phase 5 (Tier 2), Phases 8–9 (Tier 3) |
| **SC-003** Modal-in-thread diagnosed; fallback coverage verified | ✅ PASS | Phases 6 + 9; final report: `modal-in-thread-final-report.md` |
| **SC-004** Tier 1 tests run < 30 s in `cargo test` | ✅ PASS | Phase 10 run: unit 6.07s, integration 6.31s, contract 0.02s ≈ 12.4s total |
| **SC-005** Tier 1 runs in CI without credentials | ✅ PASS | Phase 10 run: 1,190 tests passed, Tier 2 feature-gated, no credentials needed |
| **SC-006** Tier 2 suite runs without human intervention; produces structured results | ✅ PASS | Phase 5–6: live test suite runs headlessly; skips when no credentials |
| **SC-007** All slash command subcommands have ≥ 1 Tier 1 routing test | ✅ PASS | Phase 2: `command_routing_tests.rs` covers all subcommands |
| **SC-008** Tier 2 tests verify messages land in correct threads | ✅ PASS | Phase 5: `live_threading_tests.rs` verifies via `conversations.replies` |
| **SC-009** Tier 3 captures screenshots for every scenario; HTML report with annotations | ✅ PASS | Phase 10: `playwright.config.ts` updated (`screenshot: 'on'`); gallery generator added |
| **SC-010** Tier 3 screenshots visually confirm Block Kit rendering | ✅ PASS | Phase 8: `message-rendering.spec.ts`; Phase 9: modal A/B screenshots |

### Gate Summary

| Gate | Result |
|---|---|
| `cargo test` (Phase 10) | ✅ 1,190 tests passed, 0 failed |
| `cargo test` timing — Tier 1 subset | ✅ ~12.4 s (SC-004) |
| `cargo test` without credentials — Tier 2 skipped | ✅ (SC-005) |
| `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ PASS |
| `cargo fmt --all -- --check` | ✅ PASS |
| Playwright HTML reporter configured (`screenshot: 'on'`) | ✅ PASS |
| Gallery generator (`helpers/generate-gallery.ts`) created | ✅ PASS |
| Modal diagnostic final report created | ✅ `modal-in-thread-final-report.md` |
| All 10 success criteria verified | ✅ PASS |





---

## Contracts

# Test Harness Contracts: Slack UI Automated Testing

**Feature**: 008-slack-ui-testing | **Date**: 2026-03-09

## Tier 1 — Block Kit Assertion Contracts

Each Block Kit builder function in `blocks.rs` must produce output matching these contracts.

### `severity_section(level, message)` → SlackBlock::Section

```json
{
  "type": "section",
  "text": {
    "type": "mrkdwn",
    "text": "{emoji} {message}"
  }
}
```

Where `{emoji}` is:
- `level = "success"` → ✅
- `level = "warning"` → ⚠️
- `level = "error"` → ❌
- `level = _` (info/default) → ℹ️

### `approval_buttons(request_id)` → SlackBlock::Actions

```json
{
  "type": "actions",
  "block_id": "approval_{request_id}",
  "elements": [
    { "type": "button", "action_id": "approve_accept", "text": { "text": "Accept" }, "value": "{request_id}" },
    { "type": "button", "action_id": "approve_reject", "text": { "text": "Reject" }, "value": "{request_id}" }
  ]
}
```

### `prompt_buttons(prompt_id)` → SlackBlock::Actions

```json
{
  "type": "actions",
  "block_id": "prompt_{prompt_id}",
  "elements": [
    { "type": "button", "action_id": "prompt_continue", "text": { "text": "Continue" }, "value": "{prompt_id}" },
    { "type": "button", "action_id": "prompt_refine", "text": { "text": "Refine" }, "value": "{prompt_id}" },
    { "type": "button", "action_id": "prompt_stop", "text": { "text": "Stop" }, "value": "{prompt_id}" }
  ]
}
```

### `nudge_buttons(alert_id)` → SlackBlock::Actions

```json
{
  "type": "actions",
  "block_id": "stall_{alert_id}",
  "elements": [
    { "type": "button", "action_id": "stall_nudge", "text": { "text": "Nudge" }, "value": "{alert_id}" },
    { "type": "button", "action_id": "stall_nudge_instruct", "text": { "text": "Nudge with Instructions" }, "value": "{alert_id}" },
    { "type": "button", "action_id": "stall_stop", "text": { "text": "Stop" }, "value": "{alert_id}" }
  ]
}
```

### `wait_buttons(session_id)` → SlackBlock::Actions

```json
{
  "type": "actions",
  "block_id": "wait_{session_id}",
  "elements": [
    { "type": "button", "action_id": "wait_resume", "text": { "text": "Resume" }, "value": "{session_id}" },
    { "type": "button", "action_id": "wait_resume_instruct", "text": { "text": "Resume with Instructions" }, "value": "{session_id}" },
    { "type": "button", "action_id": "wait_stop", "text": { "text": "Stop Session" }, "value": "{session_id}" }
  ]
}
```

### `instruction_modal(callback_id, title, placeholder)` → SlackView::Modal

```json
{
  "type": "modal",
  "callback_id": "{callback_id}",
  "title": { "type": "plain_text", "text": "{title}" },
  "submit": { "type": "plain_text", "text": "Submit" },
  "blocks": [
    {
      "type": "input",
      "block_id": "instruction_block",
      "element": {
        "type": "plain_text_input",
        "action_id": "instruction_text",
        "multiline": true,
        "placeholder": { "type": "plain_text", "text": "{placeholder}" }
      },
      "label": { "type": "plain_text", "text": "Instructions" }
    }
  ]
}
```

### `session_started_blocks(session)` → Vec<SlackBlock>

Must contain a section with:
- Session ID prefix (first 8 chars + "…")
- Protocol mode: "MCP" or "ACP"
- Operational mode: "remote", "local", or "hybrid"
- Workspace root path
- Creation timestamp in "YYYY-MM-DD HH:MM UTC" format

### `stall_alert_blocks(session_id, idle_seconds)` → Vec<SlackBlock>

Must contain:
- Warning severity section with idle duration display
- Nudge/Nudge with Instructions/Stop action buttons

### `command_approval_blocks(command, request_id)` → Vec<SlackBlock>

Must contain:
- Lock emoji (🔐) + "Terminal command approval requested" header
- Command in code fence
- Accept/Reject approval buttons

## Tier 2 — Live Interaction Contracts

### Message Verification (via conversations.history)

After posting a message, the test verifies:
- `messages[0].blocks` matches the expected Block Kit structure
- `messages[0].thread_ts` matches expected threading (None for top-level, parent ts for threaded)
- `messages[0].ts` is a valid Slack timestamp

### Interaction Round-Trip

After dispatching a synthetic interaction payload:
- Database record updated (e.g., `ApprovalRequest.status = "approved"`)
- Oneshot channel resolved (blocking tool call returns)
- Follow-up message posted to correct thread (verified via conversations.replies)

## Tier 3 — Visual Assertion Contracts

### Screenshot Naming Convention

```
{scenario_id}_{step_number}_{description}_{timestamp}.png
```

Example: `modal_in_thread_03_after_click_20260309T064500.png`

### HTML Report Structure

```html
<h1>Tier 3 Visual Test Report — {date}</h1>
<section class="scenario">
  <h2>{scenario_name} — {PASS|FAIL}</h2>
  <div class="step">
    <h3>Step {n}: {description}</h3>
    <img src="screenshots/{filename}" />
    <p class="assertion">{pass|fail}: {what was verified}</p>
  </div>
</section>
```

<!-- SECTION:DESCRIPTION:END -->
