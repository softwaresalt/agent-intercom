---
id: TASK-004.09
title: "004 - User Story 6 + 13 — Policy Hot-Reload + Regex Pre-Compilation (Priority: P2/P4)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-004
dependencies: []
ordinal: 4090
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Policy changes take effect immediately; regex patterns pre-compiled for performance

**Independent Test**: Modify settings.json, verify next auto_check reflects new rules without restart

### Tests for US6/US13

- [x] T047 [P] [US6] Unit test for `CompiledWorkspacePolicy` creation in `tests/unit/policy_tests.rs` (scenarios S074-S079)
- [x] T048 [P] [US6] Unit test for `PolicyEvaluator::check()` with `CompiledWorkspacePolicy` in `tests/unit/policy_evaluator_tests.rs` (scenario S075)
- [x] T049 [P] [US6] Contract test for `auto_check` reading from cache in `tests/contract/auto_check_contract_tests.rs` (scenarios S043-S044)
- [x] T050 [P] [US6] Unit test for invalid regex handling in `tests/unit/policy_tests.rs` (scenario S076)

### Implementation for US6/US13

- [x] T051 [US6] Update `src/policy/evaluator.rs` — replace `match_command_pattern` with `RegexSet::matches()` on `CompiledWorkspacePolicy`
- [x] T052 [US6] Wire `PolicyCache` into `AppState` reads in `src/mcp/tools/check_auto_approve.rs`
- [x] T053 [US6] Update `src/policy/watcher.rs` — ensure cache stores `CompiledWorkspacePolicy`

**Checkpoint**: Policy hot-reload end-to-end, regex pre-compiled

---

<!-- SECTION:DESCRIPTION:END -->
