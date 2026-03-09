# Phase Memory: 008-slack-ui-testing — Phase 1

**Feature**: 008-slack-ui-testing  
**Phase**: 1 — Test Infrastructure & Block Kit Assertions  
**Date**: 2026-03-09  
**Status**: ✅ COMPLETE — all gates passed

---

## What Was Built

Phase 1 establishes the Tier 1 test foundation (SC-001): every public Block Kit builder in
`src/slack/blocks.rs` now has at least one test.

### Files Created

| File | Tests | Scenario Coverage |
|---|---|---|
| `tests/unit/blocks_approval_tests.rs` | 19 | S-T1-001 |
| `tests/unit/blocks_prompt_tests.rs` | 19 | S-T1-002 |
| `tests/unit/blocks_stall_tests.rs` | 17 | S-T1-003 |
| `tests/unit/blocks_session_tests.rs` | 20 | S-T1-005 |
| `tests/unit/blocks_misc_tests.rs` | 35 | S-T1-004, S-T1-006, S-T1-008 |

### Files Modified

| File | Change |
|---|---|
| `tests/unit/blocks_tests.rs` | Added 5 comprehensive modal structure tests (S-T1-007) |
| `tests/unit.rs` | Registered 5 new test modules |
| `specs/008-slack-ui-testing/tasks.md` | Marked tasks 1.1–1.7 and constitution gate complete |

---

## Gate Results

| Gate | Result |
|---|---|
| `cargo check --tests` | ✅ Pass |
| `cargo test -- blocks_` | ✅ Pass — 158 passed, 0 failed |
| `cargo test` (full suite) | ✅ Pass — 596 passed, 0 failed |
| `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ Pass — 0 warnings |
| `cargo fmt --all -- --check` | ✅ Pass |
| SC-001: every Block Kit builder has ≥1 test | ✅ Pass |

---

## Test Coverage — Block Kit Builders (SC-001)

All 26 public functions in `src/slack/blocks.rs` are covered:

| Builder | Test File | Direct Test? |
|---|---|---|
| `action_buttons` | `blocks_misc_tests` | ✅ Direct |
| `approval_buttons` | `blocks_misc_tests` | ✅ Direct |
| `auto_approve_suggestion_button` | `blocks_misc_tests` | ✅ Direct |
| `build_approval_blocks` | `blocks_approval_tests` | ✅ Direct |
| `build_prompt_blocks` | `blocks_prompt_tests` | ✅ Direct |
| `code_snippet_blocks` | `blocks_misc_tests` | ✅ Direct |
| `command_approval_blocks` | `blocks_approval_tests` | ✅ Direct |
| `diff_applied_section` | `blocks_misc_tests` | ✅ Direct |
| `diff_conflict_section` | `blocks_misc_tests` | ✅ Direct |
| `diff_force_warning_section` | `blocks_misc_tests` | ✅ Direct |
| `diff_section` | `blocks_misc_tests` | ✅ Direct |
| `instruction_modal` | `blocks_tests` | ✅ Direct |
| `message_visible_at_level` | `blocks_tests` | ✅ Direct |
| `nudge_buttons` | `blocks_stall_tests` | ✅ Direct |
| `prompt_buttons` | `blocks_prompt_tests` | ✅ Direct |
| `prompt_type_icon` | `blocks_prompt_tests` | ✅ Direct |
| `prompt_type_label` | `blocks_prompt_tests` | ✅ Direct |
| `session_ended_blocks` | `blocks_session_tests` | ✅ Direct |
| `session_started_blocks` | `blocks_session_tests` | ✅ Direct |
| `severity_section` | `blocks_misc_tests` | ✅ Direct |
| `slack_escape` | `blocks_misc_tests` | ✅ Direct |
| `stall_alert_blocks` | `blocks_stall_tests` | ✅ Direct |
| `stall_alert_message` | `blocks_stall_tests` | ✅ Direct |
| `text_section` | `blocks_misc_tests` | ✅ Direct |
| `truncate_text` | `blocks_misc_tests` | ✅ Direct |
| `wait_buttons` | `blocks_misc_tests` | ✅ Direct |

---

## Adversarial Review Findings

**Severity counts: 0 critical, 0 high, 1 medium, 2 low**

### Medium

**M-001 — `doc_markdown` lint violations in initial draft**  
- 10 doc comment identifiers (e.g., `block_id`, `ErrorRecovery`) were not wrapped in backticks.  
- Fixed before commit by adding backticks to all affected identifiers.  
- Clippy passed after fixes.

### Low

**L-001 — Formatting divergence in initial draft**  
- `rustfmt` required line-length adjustments on ~15 function calls (long argument lists on one line).  
- Fixed by running `cargo fmt --all`.

**L-002 — SC-001 gap: `action_buttons`, `text_section`, `approval_buttons` lacked direct tests**  
- These three `pub fn` entries were exercised only indirectly via higher-level builders.  
- Added 8 direct tests to `blocks_misc_tests.rs` during adversarial review.

---

## Design Decisions

### Test approach: JSON serialisation assertions

All block assertions use `serde_json::to_string(&block)` followed by substring checks. This
mirrors the existing `blocks_tests.rs` pattern and avoids needing to destructure the opaque
`slack_morphism` types. Trade-off: assertions are sensitive to serialisation format changes in
upstream crates, but block structure is stable in `slack-morphism` 2.x.

### No test helpers module introduced

The spec's `data-model.md` describes a `BlockKitAssertion` helper, but for Phase 1 the inline
JSON assertion pattern is sufficient and avoids over-engineering. If Phase 2/3 integration tests
require repeated block-structure assertions, a `tests/unit/block_kit_helpers.rs` module should
be introduced then.

---

## Deferred Items

- **Phase 2**: Simulated interaction dispatch tests (integration tests with mock `AppState`)
- **Phase 3**: Edge cases, double-submission, auth guard, fallback tests
- **Phases 4–10**: Live Slack and Playwright visual tests

---

## Next Phase Prerequisites

Phase 2 requires:
- Mock `AppState` construction with in-memory SQLite and registered oneshot channels
- Access to `slack::events` handler internals (may require pub visibility changes)
- `tests/integration/` module extension

No blockers identified for Phase 2 start.
