---
id: TASK-008.02
title: "008 - Test Infrastructure & Block Kit Assertions"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-008
dependencies: []
ordinal: 8020
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Establish the Tier 1 test foundation and achieve SC-001 (every Block Kit builder has a test).

**Depends on**: Nothing — this is the starting phase.

### Tasks

- [X] **1.1** Create `tests/unit/blocks_approval_tests.rs`
- Test `command_approval_blocks()` with representative inputs
- Assert block types, action_ids, button labels, request_id in values
- Assert severity section emoji (🔐)
- Scenarios: S-T1-001
- FRs: FR-001

- [X] **1.2** Create `tests/unit/blocks_prompt_tests.rs`
- Test prompt message block construction
- Assert Continue/Refine/Stop buttons with correct action_ids
- Assert prompt text and type indicator
- Scenarios: S-T1-002
- FRs: FR-001

- [X] **1.3** Create `tests/unit/blocks_stall_tests.rs`
- Test `stall_alert_blocks()` with representative idle durations
- Assert Nudge/Nudge with Instructions/Stop buttons
- Assert warning severity section
- Scenarios: S-T1-003
- FRs: FR-001

- [X] **1.4** Create `tests/unit/blocks_session_tests.rs`
- Test `session_started_blocks()` for MCP and ACP modes
- Assert session ID prefix, protocol mode, operational mode, workspace root, timestamp
- Test `session_ended()` for correct format
- Scenarios: S-T1-005
- FRs: FR-001

- [X] **1.5** Create `tests/unit/blocks_misc_tests.rs`
- Test `wait_buttons()` — assert Resume/Resume with Instructions/Stop Session
- Test `severity_section()` for all four levels — assert emoji mapping
- Test `code_snippet_blocks()` — assert label headers and code content
- Test `diff_section()`, `diff_applied_section()`, `diff_conflict_section()`
- Scenarios: S-T1-004, S-T1-006, S-T1-007, S-T1-008
- FRs: FR-001

- [X] **1.6** Extend existing `tests/unit/blocks_tests.rs`
- Verify existing `instruction_modal` test still passes
- Add comprehensive modal structure assertion (callback_id, title, submit, input block, placeholder)
- Scenario: S-T1-007
- FRs: FR-001

- [X] **1.7** Register all new test modules in `tests/unit/mod.rs`
- Add `mod blocks_approval_tests;`, etc.

### Constitution Gate

- [X] All new tests compile: `cargo check --tests`
- [X] All new tests run: `cargo test -- blocks_`
- [X] Clippy clean: `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`
- [X] Each Block Kit builder in `blocks.rs` has at least one test (SC-001)

---

<!-- SECTION:DESCRIPTION:END -->
