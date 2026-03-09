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
