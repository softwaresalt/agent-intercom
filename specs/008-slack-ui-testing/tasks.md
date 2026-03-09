# Task Plan: Slack UI Automated Testing

**Feature**: 008-slack-ui-testing | **Date**: 2026-03-09

## Phase Overview

| Phase | Name | Description | Est. Tests |
|---|---|---|---|
| 1 | Test infrastructure & Block Kit assertions | Tier 1 foundation: test helpers, Block Kit builder coverage | ~20 |
| 2 | Simulated interaction dispatch | Tier 1: synthetic button/modal/command handler tests | ~15 |
| 3 | Edge cases & error paths | Tier 1: double-submission, auth guard, stale references, fallback | ~12 |
| 4 | Live Slack test harness | Tier 2: feature-gated test infrastructure, live helpers | ~5 |
| 5 | Live message & interaction tests | Tier 2: post/verify messages, synthetic interaction round-trips | ~10 |
| 6 | Modal diagnostics (API level) | Tier 2: threaded vs top-level modal API testing | ~4 |
| 7 | Playwright scaffolding | Tier 3: Node.js project, auth, navigation helpers | ~3 |
| 8 | Visual rendering tests | Tier 3: message rendering, button interactions, screenshots | ~6 |
| 9 | Modal-in-thread visual diagnosis | Tier 3: the critical A/B test + fallback visual flow | ~4 |
| 10 | Report generation & CI integration | HTML report, screenshot gallery, CI pipeline gates | ~3 |

---

## Phase 1: Test Infrastructure & Block Kit Assertions

**Goal**: Establish the Tier 1 test foundation and achieve SC-001 (every Block Kit builder has a test).

**Depends on**: Nothing — this is the starting phase.

### Tasks

**1.1** Create `tests/unit/blocks_approval_tests.rs`
- Test `command_approval_blocks()` with representative inputs
- Assert block types, action_ids, button labels, request_id in values
- Assert severity section emoji (🔐)
- Scenarios: S-T1-001
- FRs: FR-001

**1.2** Create `tests/unit/blocks_prompt_tests.rs`
- Test prompt message block construction
- Assert Continue/Refine/Stop buttons with correct action_ids
- Assert prompt text and type indicator
- Scenarios: S-T1-002
- FRs: FR-001

**1.3** Create `tests/unit/blocks_stall_tests.rs`
- Test `stall_alert_blocks()` with representative idle durations
- Assert Nudge/Nudge with Instructions/Stop buttons
- Assert warning severity section
- Scenarios: S-T1-003
- FRs: FR-001

**1.4** Create `tests/unit/blocks_session_tests.rs`
- Test `session_started_blocks()` for MCP and ACP modes
- Assert session ID prefix, protocol mode, operational mode, workspace root, timestamp
- Test `session_ended()` for correct format
- Scenarios: S-T1-005
- FRs: FR-001

**1.5** Create `tests/unit/blocks_misc_tests.rs`
- Test `wait_buttons()` — assert Resume/Resume with Instructions/Stop Session
- Test `severity_section()` for all four levels — assert emoji mapping
- Test `code_snippet_blocks()` — assert label headers and code content
- Test `diff_section()`, `diff_applied_section()`, `diff_conflict_section()`
- Scenarios: S-T1-004, S-T1-006, S-T1-007, S-T1-008
- FRs: FR-001

**1.6** Extend existing `tests/unit/blocks_tests.rs`
- Verify existing `instruction_modal` test still passes
- Add comprehensive modal structure assertion (callback_id, title, submit, input block, placeholder)
- Scenario: S-T1-007
- FRs: FR-001

**1.7** Register all new test modules in `tests/unit/mod.rs`
- Add `mod blocks_approval_tests;`, etc.

### Constitution Gate

- [ ] All new tests compile: `cargo check --tests`
- [ ] All new tests run: `cargo test -- blocks_`
- [ ] Clippy clean: `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`
- [ ] Each Block Kit builder in `blocks.rs` has at least one test (SC-001)

---

## Phase 2: Simulated Interaction Dispatch

**Goal**: Verify handler pipeline processes synthetic operator actions correctly (SC-002 for Tier 1).

**Depends on**: Phase 1 (test infrastructure).

### Tasks

**2.1** Create `tests/integration/slack_interaction_tests.rs`
- Build mock `AppState` with in-memory DB and registered oneshot channels
- Test approval accept: dispatch synthetic action → verify oneshot resolved, DB updated
- Test approval reject: same pattern
- Scenarios: S-T1-009, S-T1-010
- FRs: FR-002, FR-009

**2.2** Add prompt interaction tests to `slack_interaction_tests.rs`
- Test prompt continue: dispatch → verify oneshot resolved
- Test prompt stop: dispatch → verify oneshot resolved
- Test nudge: dispatch → verify stall resolved
- Test wait resume: dispatch → verify standby resolved
- Scenarios: S-T1-011, S-T1-025, S-T1-026
- FRs: FR-002, FR-009

**2.3** Create `tests/integration/slack_modal_flow_tests.rs`
- Test prompt refine → modal open path (with `state.slack = None`, verify fallback activates)
- Test modal submission → prompt resolution with instruction text
- Scenarios: S-T1-012, S-T1-013
- FRs: FR-002, FR-009, FR-011

**2.4** Create `tests/unit/command_routing_tests.rs`
- Test `/acom` prefix routing for MCP mode
- Test `/arc` prefix routing for ACP mode
- Test mode gating: ACP-only command rejected in MCP mode
- Test malformed arguments → usage message
- Scenarios: S-T1-021, S-T1-022, S-T1-023
- FRs: FR-005

**2.5** Register new test modules in `tests/integration/mod.rs`

### Constitution Gate

- [ ] All interaction tests pass: `cargo test -- slack_interaction`
- [ ] All modal flow tests pass: `cargo test -- slack_modal`
- [ ] All command routing tests pass: `cargo test -- command_routing`
- [ ] Clippy clean: `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`

---

## Phase 3: Edge Cases & Error Paths

**Goal**: Cover error paths, guard logic, and edge cases (FR-003, FR-004, FR-010, FR-012).

**Depends on**: Phase 2 (interaction dispatch infrastructure).

### Tasks

**3.1** Add authorization guard tests to `slack_interaction_tests.rs`
- Test unauthorized user → action rejected, no state change
- Test authorized user → action proceeds
- Scenarios: S-T1-015, S-T1-016
- FRs: FR-003

**3.2** Add double-submission prevention test
- Dispatch same action twice → first resolves, second silently ignored
- Scenario: S-T1-014
- FRs: FR-004

**3.3** Create `tests/integration/slack_fallback_tests.rs`
- Test thread-reply fallback: register pending, send reply → oneshot resolved
- Test orphaned thread reply: no pending → ignored gracefully
- Scenarios: S-T1-017, S-T1-018
- FRs: FR-010, FR-011

**3.4** Add error path tests to `slack_interaction_tests.rs`
- Unknown action_id → graceful handling
- Stale session reference → graceful error
- Consumed oneshot channel → graceful handling
- Scenarios: S-T1-019, S-T1-020, S-T1-027
- FRs: FR-010, FR-012

**3.5** Create `tests/integration/slack_threading_tests.rs`
- Two sessions in same channel with different thread_ts
- Button action in Session A → only Session A affected
- Scenario: S-T1-024
- FRs: FR-006

### Constitution Gate

- [ ] All error path tests pass: `cargo test -- slack_fallback slack_threading`
- [ ] Clippy clean
- [ ] No panics in any error path test
- [ ] SC-002 Tier 1 portion complete: all 6 interaction types have simulated tests

---

## Phase 4: Live Slack Test Harness

**Goal**: Build the Tier 2 test infrastructure with feature gating.

**Depends on**: Phase 1 (test helpers can be reused).

### Tasks

**4.1** Add `live-slack-tests` feature flag to `Cargo.toml`
- `[features]` section: `live-slack-tests = []`
- FRs: FR-021

**4.2** Create `tests/live/mod.rs` with feature gate
- `#![cfg(feature = "live-slack-tests")]`
- Module declarations for all live test files

**4.3** Create `tests/live/live_helpers.rs`
- `LiveTestConfig` — loads from env vars
- `LiveSlackClient` — wrapper around `reqwest` for Slack Web API
- `post_test_message()` — post to test channel, return ts
- `get_message()` — retrieve via `conversations.history`
- `get_thread_replies()` — retrieve via `conversations.replies`
- `cleanup_test_messages()` — delete test messages after suite
- `assert_blocks_contain()` — verify block structure in API response

**4.4** Create `tests/live/live_message_tests.rs` (skeleton with 1 smoke test)
- Post a simple message, retrieve via API, verify it exists
- Scenario: S-T2-001 (partial)

### Constitution Gate

- [ ] `cargo check --features live-slack-tests` compiles
- [ ] Clippy clean with feature flag
- [ ] Smoke test passes when credentials are available

---

## Phase 5: Live Message & Interaction Tests

**Goal**: Tier 2 message posting, threading, and interaction round-trips (SC-006, SC-008).

**Depends on**: Phase 4 (live harness).

### Tasks

**5.1** Complete `tests/live/live_message_tests.rs`
- Post approval message → verify structure via API
- Post to thread → verify via `conversations.replies`
- Post to multiple sessions → verify correct threading
- Scenarios: S-T2-001, S-T2-002, S-T2-003
- FRs: FR-013, FR-018

**5.2** Create `tests/live/live_interaction_tests.rs`
- Approval accept round-trip: post approval → dispatch synthetic accept → verify DB + follow-up message
- Prompt continue round-trip
- Stall nudge round-trip
- Button replacement: verify message updated after action
- Scenarios: S-T2-004, S-T2-005, S-T2-010, S-T2-013
- FRs: FR-014

**5.3** Create `tests/live/live_threading_tests.rs`
- Multi-session thread isolation in real Slack
- Scenarios: S-T2-003
- FRs: FR-018

**5.4** Create `tests/live/live_command_tests.rs`
- Slash command dispatch and response verification
- Scenario: S-T2-012

**5.5** Add rate limit handling test
- Post messages in rapid succession → verify server handles backoff
- Scenario: S-T2-009

### Constitution Gate

- [ ] All live tests pass with credentials: `cargo test --features live-slack-tests`
- [ ] Clippy clean
- [ ] SC-006: all Tier 2 scenarios produce structured results
- [ ] SC-008: threaded messages verified in correct threads

---

## Phase 6: Modal Diagnostics (API Level)

**Goal**: Diagnose modal-in-thread issue at the API level (FR-022, FR-023).

**Depends on**: Phase 5 (live interaction infrastructure).

### Tasks

**6.1** Create `tests/live/live_modal_tests.rs`
- Test modal open for top-level button → document API result
- Test modal open for threaded button → document API result
- Compare results: success/error, trigger_id scope, timing
- Scenarios: S-T2-006, S-T2-007
- FRs: FR-015, FR-016, FR-022

**6.2** Add thread-reply fallback end-to-end test
- Simulate modal failure → fallback activates → thread reply resolves prompt
- Scenario: S-T2-008
- FRs: FR-017, FR-023

**6.3** Test wait-resume-instruct modal in thread
- Same pattern as prompt refine: threaded vs top-level
- Scenario: S-T2-011
- FRs: FR-015

**6.4** Write diagnostic report section in `SCENARIOS.md` or standalone file
- Categorize failure mode based on API evidence
- Scenarios: S-X-001

### Constitution Gate

- [ ] Modal diagnostic tests pass
- [ ] API-level evidence documented for modal-in-thread behavior
- [ ] Fallback coverage verified for all 3 modal paths (SC-003 API portion)

---

## Phase 7: Playwright Scaffolding

**Goal**: Set up the Tier 3 Node.js/Playwright project with auth and navigation helpers.

**Depends on**: Nothing (can run in parallel with Phases 1–6).

### Tasks

**7.1** Create `tests/visual/package.json`
- Dependencies: `@playwright/test`, `dotenv`
- Scripts: `test`, `test:setup`, `report`

**7.2** Create `tests/visual/playwright.config.ts`
- Chromium-only project
- `testDir: './scenarios'`
- Screenshot output to `screenshots/`
- Report output to `reports/`
- Global setup for auth
- Configurable timeouts

**7.3** Create `tests/visual/helpers/slack-auth.ts`
- Navigate to Slack workspace URL
- Enter email/password
- Handle login flow
- Save session cookies to `auth/` directory

**7.4** Create `tests/visual/helpers/slack-nav.ts`
- Navigate to channel by name
- Navigate into a thread by message timestamp
- Wait for channel to fully load
- Scroll to latest message

**7.5** Create `tests/visual/helpers/slack-selectors.ts`
- DOM selector strategies for: buttons, modals, text inputs, messages, threads, code blocks
- Strategy: prefer `data-qa` attributes, fall back to `aria-label`, then class-based selectors
- Document which selectors may break on Slack client updates

**7.6** Create `tests/visual/helpers/screenshot.ts`
- `captureStep(page, scenarioId, stepNumber, description)` — captures screenshot with naming convention
- Screenshot naming: `{scenarioId}_{stepNumber}_{description}_{timestamp}.png`
- Utility to check if element is visible within timeout

**7.7** Create directory stubs: `auth/`, `screenshots/`, `reports/` with `.gitkeep`

**7.8** Add `tests/visual/` entries to `.gitignore`
- `tests/visual/node_modules/`
- `tests/visual/auth/*.json` (session cookies)
- `tests/visual/screenshots/*.png`
- `tests/visual/reports/`

### Constitution Gate

- [ ] `npm install` succeeds in `tests/visual/`
- [ ] `npx playwright install chromium` succeeds
- [ ] Auth setup test can navigate to Slack login page (manual verification)
- [ ] Screenshot helper saves a test image to the correct path

---

## Phase 8: Visual Rendering Tests

**Goal**: Screenshot-based verification of Block Kit rendering in real Slack (SC-009, SC-010).

**Depends on**: Phase 7 (Playwright scaffolding).

### Tasks

**8.1** Create `tests/visual/scenarios/message-rendering.spec.ts`
- Navigate to test channel
- Verify approval message rendering: emoji, diff block, buttons
- Verify prompt message rendering: text, buttons
- Verify stall alert rendering: warning emoji, duration, buttons
- Verify session started notification
- Verify code snippet blocks
- Capture screenshots for each
- Scenarios: S-T3-002, S-T3-003, S-T3-004, S-T3-009, S-T3-010
- FRs: FR-026

**8.2** Create `tests/visual/scenarios/approval-flow.spec.ts`
- Click Accept button on approval message
- Capture before/after screenshots showing button replacement
- Scenario: S-T3-008
- FRs: FR-027, FR-025

**8.3** Create `tests/visual/scenarios/button-replacement.spec.ts`
- Click various buttons (Continue, Nudge, Resume) and capture transitions
- Verify static status text replaces interactive buttons
- Scenarios: S-T3-008
- FRs: FR-027

### Constitution Gate

- [ ] All visual rendering tests pass against test workspace
- [ ] Screenshots captured for every scenario
- [ ] SC-010: visual confirmation of correct Block Kit rendering

---

## Phase 9: Modal-in-Thread Visual Diagnosis

**Goal**: The critical A/B test — visual evidence of modal behavior (SC-003, FR-022).

**Depends on**: Phase 8 (visual test infrastructure).

### Tasks

**9.1** Create `tests/visual/scenarios/modal-top-level.spec.ts`
- Post prompt as top-level message
- Click Refine button
- Screenshot: modal appears with title, text input, submit button
- Type test text, screenshot, submit
- Screenshot: message updated with resolved status
- Scenario: S-T3-005
- FRs: FR-027, FR-028

**9.2** Create `tests/visual/scenarios/modal-in-thread.spec.ts`
- Post prompt inside a thread
- Navigate into thread
- Click Refine button
- Wait configurable timeout (5+ seconds)
- Screenshot: document whether modal appeared
- If no modal: screenshot of unchanged thread view
- If modal: screenshot of modal in thread context
- Scenario: S-T3-006
- FRs: FR-027, FR-028, FR-030, FR-022

**9.3** Create `tests/visual/scenarios/thread-reply-fallback.spec.ts`
- Given modal-in-thread failure confirmed
- Trigger fallback: server posts prompt in thread
- Screenshot: fallback prompt visible
- Type reply in thread composer
- Screenshot: reply being composed
- Submit reply
- Screenshot: resolved state
- Scenario: S-T3-007
- FRs: FR-023, FR-028

**9.4** Add wait-resume-instruct modal-in-thread test
- Same A/B pattern as Refine but for Resume with Instructions button
- Scenario: S-T3-011
- FRs: FR-022, FR-028

### Constitution Gate

- [ ] A/B comparison screenshots captured: threaded vs non-threaded modal
- [ ] Modal-in-thread failure mode documented with visual evidence
- [ ] Fallback flow visually verified
- [ ] SC-003: root cause categorized, all 3 modal paths tested, fallback coverage verified

---

## Phase 10: Report Generation & CI Integration

**Goal**: HTML report, CI gates, final verification (SC-004, SC-005, SC-009).

**Depends on**: All previous phases.

### Tasks

**10.1** Configure Playwright HTML reporter
- Inline screenshots in report
- Pass/fail annotations per scenario
- Chronological screenshot gallery
- Scenario: S-T3-012
- FRs: FR-029

**10.2** Verify Tier 1 performance gate
- Run `cargo test` and measure total time
- Confirm Tier 1 tests add < 30 seconds
- SC-004

**10.3** Verify CI gate (no-credential environment)
- Run `cargo test` without Slack credentials
- Confirm all Tier 1 tests pass, Tier 2 tests skipped (feature gate)
- SC-005

**10.4** Write modal-in-thread diagnostic report
- Compile API evidence (Tier 2) and visual evidence (Tier 3)
- Categorize failure mode
- Document remediation recommendation
- Cross-reference: S-X-001, S-X-002
- FRs: FR-022, FR-023

**10.5** Update spec `checklists/requirements.md` with final pass/fail status

### Constitution Gate

- [ ] `cargo test` passes in CI-like environment (no credentials)
- [ ] `cargo test --features live-slack-tests` passes with credentials
- [ ] Playwright visual suite passes with screenshots + HTML report
- [ ] All 10 success criteria verified
- [ ] Modal diagnostic report complete

---

## Task Dependency Graph

```
Phase 1 ─┬─► Phase 2 ─► Phase 3
          │
          └─► Phase 4 ─► Phase 5 ─► Phase 6
                                       │
Phase 7 ─► Phase 8 ─► Phase 9 ◄───────┘
                         │
                         ▼
                      Phase 10
```

- Phases 1 and 7 have no dependencies and can start immediately.
- Phase 7 (Playwright scaffolding) can run in parallel with Phases 1–6.
- Phase 6 feeds diagnostic context into Phase 9 (visual diagnosis).
- Phase 10 depends on all other phases being complete.

## Estimated Test Count

| Tier | Test Files | Approx. Tests |
|---|---|---|
| Tier 1 Unit | 6 new files | ~20 |
| Tier 1 Integration | 4 new files | ~15 |
| Tier 2 Live | 5 new files | ~15 |
| Tier 3 Visual | 6 new spec files | ~15 |
| **Total** | **21 new files** | **~65** |
