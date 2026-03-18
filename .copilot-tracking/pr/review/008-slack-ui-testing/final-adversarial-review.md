# Final Adversarial Review — 008-slack-ui-testing

**Feature**: `008-slack-ui-testing`  
**Base commit**: `b17e594bd0158006cc39925f3458f656211fda5b`  
**Review date**: 2026-03-09  
**Scope**: all feature changes from base commit to `HEAD`

## Reviewer Summary

| Reviewer | Model | Focus | Raw Findings |
|---|---|---|---:|
| A | Gemini 3 Pro Preview | Correctness, security, edge cases | 3 |
| B | GPT-5.3 Codex | Technical quality, architecture, performance | 5 |
| C | Claude Opus 4.6 | Logical consistency, completeness, maintainability | 6 |

## Synthesis Notes

- The Gemini review surfaced three findings in `src/slack/handlers/thread_reply.rs` and `src/slack/blocks.rs`, but those files were not modified in the feature diff and were excluded as out-of-scope for the final feature remediation pass.
- The strongest corroborated issues were:
  - cleanup semantics in `tests/live/live_helpers.rs` (reviewers B and C)
  - expanded visibility of `dispatch_command` in `src/slack/commands.rs` (reviewers B and C)
- A severity conflict existed on the `dispatch_command` visibility finding (MEDIUM vs LOW). The higher severity was retained during synthesis.

## Unified Findings

| ID | Severity | File | Lines | Summary | Recommended Fix | Consensus |
|---|---|---|---|---|---|---|
| FR-001 | HIGH | `specs/008-slack-ui-testing/quickstart.md` | 24-31, 55-62 | Quickstart environment variable names did not match the actual Tier 2 and Tier 3 code paths, causing silent misconfiguration for live and visual runs. | Align the documented env vars with `.env.example` and the code (`SLACK_TEST_BOT_TOKEN`, `SLACK_TEST_CHANNEL_ID`, `SLACK_WORKSPACE_URL`, `SLACK_EMAIL`, `SLACK_PASSWORD`, `SLACK_TEST_CHANNEL`). | single (1/3) |
| FR-002 | HIGH | `tests/visual/scenarios/message-rendering.spec.ts` and other Phase 8/9 visual specs | multiple | Visual tests skipped when required Slack UI artifacts were absent even in a configured environment, creating false-green runs. | Keep skip only for missing environment/credentials; once configured, convert missing artifact paths into explicit assertions after capturing a diagnostic screenshot. | single (1/3) |
| FR-003 | HIGH | `tests/visual/scenarios/thread-reply-fallback.spec.ts` | multiple | The fallback visual scenario only validated a pre-existing prompt and skipped if it was absent, so the scenario could pass without proving FR-023 behavior. | Remove the false-green skip path and fail clearly when the fallback prompt is absent in a configured environment. | single (1/3) |
| FR-004 | MEDIUM | `tests/live/live_helpers.rs` | 211-249 | `cleanup_test_messages` documentation promised best-effort cleanup, but the implementation aborted on the first delete failure. | Attempt every deletion, collect errors, and return aggregated failure information only after the full cleanup pass. | majority (2/3) |
| FR-005 | MEDIUM | `src/slack/commands.rs` | 108-124 | `dispatch_command` was widened to `pub` for testing, increasing API surface area. | Prefer narrower visibility or document that the function is intentionally public for test access. | majority (2/3) |
| FR-006 | MEDIUM | `tests/live/*.rs` | multiple | Live tests generally assert before cleanup, so failures can strand Slack test messages. | Introduce cleanup guards or restructure assertions to preserve cleanup on failure paths. | single (1/3) |
| FR-007 | LOW | `tests/visual/helpers/slack-nav.ts` | 29 | Quick-switch navigation uses `Control+K`, which is not portable to macOS where Slack uses `Meta+K`. | Branch on `process.platform` and use `Meta+K` on macOS. | single (1/3) |

## Metrics

| Metric | Value |
|---|---:|
| Total raw findings | 14 |
| Out-of-scope findings excluded | 3 |
| Findings after synthesis | 7 |
| Majority/unanimous findings | 2 |
| Agreement rate | 28.6% |
| Conflict count | 1 |

## Remediation Outcome

- Applied fixes for **FR-001**, **FR-002**, **FR-003**, and **FR-004** in commit `8a14dc9`.
- Deferred **FR-005**, **FR-006**, and **FR-007**:
  - **FR-005** deferred because the current external test harness in `tests/` requires public visibility for `dispatch_command`; tightening visibility would require broader test architecture changes.
  - **FR-006** deferred because introducing cleanup guards across all live tests is a wider refactor than necessary for this build completion pass.
  - **FR-007** deferred because the current execution environment is Windows and the issue is low severity/platform-specific.

## Final Assessment

After remediation, no critical findings remain and the high-severity review issues were resolved. The remaining medium/low findings are documented follow-up improvements rather than blockers for feature completion.
