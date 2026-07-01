# Adversarial Analysis: Slack UI Automated Testing

**Feature**: 008-slack-ui-testing | **Date**: 2026-03-09

## Risk Assessment

### R-01: Slack DOM Selector Fragility (HIGH)

**Risk**: Slack updates their web client frequently. DOM selectors (`data-qa`, `aria-label`, class-based) may break without warning, causing Tier 3 tests to fail with false negatives.

**Mitigation**: Centralize all selectors in `slack-selectors.ts` (Task 7.5). Use a priority strategy: `data-qa` → `aria-label` → role-based → class-based. Document which selectors are fragile. Plan for periodic maintenance.

**Residual risk**: Selector breakage is inevitable with third-party web clients. The test suite must be designed to make selector updates fast and localized.

### R-02: Slack Rate Limiting Disrupts Live Tests (MEDIUM)

**Risk**: Tier 2 tests post multiple messages in succession. Slack's rate limits (1 msg/sec per channel, burst limits on API calls) may cause intermittent test failures.

**Mitigation**: Live test helpers include inter-test delays. The existing `SlackService` rate-limiting queue handles backoff. Scenario S-T2-009 explicitly tests this path. Use `serial_test` for Tier 2 tests to avoid parallel API calls.

### R-03: Slack Authentication Changes Break Tier 3 (MEDIUM)

**Risk**: Slack may change their login flow (add CAPTCHA, change 2FA requirements, update OAuth flow), breaking the browser-based authentication in Tier 3.

**Mitigation**: Session persistence (Task 7.3) reduces login frequency. Support both email/password and session-token injection. Document fallback: manual browser login → export cookies → reuse in Playwright.

### R-04: Modal-in-Thread Issue Is Version-Specific (MEDIUM)

**Risk**: The modal-in-thread issue may only reproduce on specific Slack client versions (desktop vs web vs mobile) or specific browser versions. Tier 3 tests using Chromium may not reproduce the issue the operator experiences on the desktop app.

**Mitigation**: FR-022 requires categorizing by client type. Tier 3 provides web client evidence. Document that desktop app behavior requires separate manual verification. Consider adding Playwright Electron support for Slack desktop app testing as a future enhancement.

### R-05: Test Channel Pollution (LOW)

**Risk**: Tier 2 and 3 tests post real messages to a real Slack channel. Over time, the channel accumulates test artifacts, making it hard to find real messages.

**Mitigation**: Task 4.3 includes `cleanup_test_messages()` that deletes test messages after the suite runs. Use a dedicated test channel that is not used for any other purpose. Consider auto-archiving old test channels.

### R-06: Credential Exposure (LOW)

**Risk**: Test workspace credentials (`SLACK_TEST_BOT_TOKEN`, `SLACK_TEST_PASSWORD`) could be accidentally committed.

**Mitigation**: Credentials are environment variables only — never in config files. `.gitignore` entries for auth cookies (Task 7.8). No credential fallback to plaintext files. CI credentials injected via secrets.

### R-07: Tier 1 Test Fragility from Internal Refactoring (LOW)

**Risk**: Tier 1 tests assert specific JSON structure of Block Kit payloads. Internal refactoring of `blocks.rs` (reordering blocks, changing text wording) could break tests without changing user-visible behavior.

**Mitigation**: Use pattern-based assertions (presence of keys/values, block type sequences) rather than exact JSON equality. The contract in `test-harness-contracts.md` defines the minimum required structure, not exact output.

## Spec Gaps Identified

### G-01: No NFR for Tier 3 Execution Time

**Gap**: The spec defines SC-004 (Tier 1 < 30s) but no time constraint for Tier 3.

**Impact**: Low. Tier 3 is on-demand and inherently slow (browser automation). A timeout of 10 minutes per visual suite is reasonable and documented in the plan.

**Recommendation**: Add a note to quickstart.md that Tier 3 typically takes 5–10 minutes. No spec change needed.

### G-02: No Explicit Test for `push_events.rs`

**Gap**: The plan lists `push_events.rs` as a test target but no specific scenarios cover push event handling.

**Impact**: Low. Push events are message events that route to `thread_reply` handler, which IS covered by S-T1-017/S-T1-018.

**Recommendation**: No change — coverage is implicit through the thread-reply scenarios.

### G-03: Reject with Reason Modal Path Underspecified

**Gap**: SC-003 mentions "all three modal-dependent paths (Refine, Resume with Instructions, Reject with Reason)" but the scenarios focus heavily on Refine and Resume with Instructions. Reject with Reason modal testing is less detailed.

**Impact**: Medium. The Reject with Reason path goes through `approval.rs` → `modal.rs` → `handle_view_submission()`, which is the same pipeline but with a different callback_id prefix.

**Recommendation**: Add a note to Phase 9 tasks to include Reject with Reason in the modal A/B comparison. The handler pipeline is identical, so one additional screenshot scenario suffices.

### G-04: No Rollback Strategy for Failed Tier 2/3 Test Runs

**Gap**: If a Tier 2 test fails mid-suite (e.g., approval posted but not cleaned up), subsequent test runs may see stale state.

**Impact**: Medium. Stale approval messages with pending oneshots could interfere with later tests.

**Recommendation**: Live test cleanup should be robust — use `afterAll` hooks to clean up regardless of test outcome. Each test should use unique request IDs to avoid collisions.

### G-05: No Accessibility Testing

**Gap**: Tier 3 captures screenshots but does not verify accessibility properties (screen reader labels, keyboard navigation, contrast).

**Impact**: Low for this feature. Accessibility is a Slack client responsibility, not agent-intercom's. Block Kit accessibility is controlled by Slack's rendering.

**Recommendation**: Out of scope. Note for future consideration.

## Scenario Coverage Gaps

### Missing Scenarios Identified

1. **Reject with Reason modal in thread** — Add to Phase 9 as variant of S-T3-006
2. **Slack web client loading timeout** — What if the channel takes >10s to load? Add timeout handling to `slack-nav.ts`
3. **Multiple button clicks in rapid succession** — Test that only the first click is processed (Tier 3 visual confirmation of double-click prevention)
4. **Emoji rendering across platforms** — Slack renders custom emoji differently across platforms; screenshots only capture web client rendering

### Traceability Verification

All 32 FRs are covered:
- FR-001–012: Tier 1 scenarios S-T1-001 through S-T1-027
- FR-013–021: Tier 2 scenarios S-T2-001 through S-T2-013
- FR-022–023: Cross-tier scenarios S-X-001, S-X-002
- FR-024–032: Tier 3 scenarios S-T3-001 through S-T3-012

All 10 SCs are traceable:
- SC-001: Phase 1 constitution gate
- SC-002: Phases 2 + 5 + 8 combined
- SC-003: Phase 9 + cross-tier analysis
- SC-004: Phase 10 task 10.2
- SC-005: Phase 10 task 10.3
- SC-006: Phase 5 constitution gate
- SC-007: Phase 2 task 2.4
- SC-008: Phase 5 constitution gate
- SC-009: Phase 10 task 10.1
- SC-010: Phase 8 constitution gate

## Verdict

**Proceed with implementation.** The spec is comprehensive, the task plan covers all requirements, and identified risks have mitigations. The three gaps (G-03, G-04) are minor and can be addressed during implementation without spec revision.
