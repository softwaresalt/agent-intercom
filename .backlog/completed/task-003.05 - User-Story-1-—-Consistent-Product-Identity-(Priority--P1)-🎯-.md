---
id: TASK-003.05
title: "003 - User Story 1 — Consistent Product Identity (Priority: P1) 🎯 MVP"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-003
dependencies: []
ordinal: 3050
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Every user-facing touchpoint (binaries, config, keychain, Slack, tests, IPC) reflects "agent-intercom" with zero "monocoque" references remaining.

**Independent Test**: `cargo build --release` produces correctly named binaries; `cargo test` passes; `grep -r "monocoque" src/ ctl/ tests/ config.toml Cargo.toml` returns zero matches.

**Scenarios covered**: S001–S020

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation (where applicable)**

- [x] T018 [P] [US1] Write unit test verifying `KEYCHAIN_SERVICE` constant equals `"agent-intercom"` in `tests/unit/config_tests.rs` (S009)
- [x] T019 [P] [US1] Write unit test verifying IPC pipe name constant equals `"agent-intercom"` in `tests/unit/config_tests.rs` (S010, S011)
- [x] T020 [P] [US1] Write unit test verifying env var prefix is `INTERCOM_` (not `MONOCOQUE_`) in `tests/unit/config_tests.rs` (S012, S013)
- [x] T021 [P] [US1] Write unit test verifying policy directory constant equals `".intercom"` in `tests/unit/policy_tests.rs` (S015, S016, S017)
- [x] T022 [P] [US1] Write contract test verifying Slack command root is `/intercom` in `tests/contract/` (new file or existing) (S018, S019, S020)
- [x] T023 [US1] Run tests and confirm new assertions FAIL (red gate) before proceeding to implementation

### Implementation for User Story 1

- [x] T024 [P] [US1] Update all test file imports from `monocoque_agent_rc` to `agent_intercom` in `tests/unit/` (15 files)
- [x] T025 [P] [US1] Update all test file imports from `monocoque_agent_rc` to `agent_intercom` in `tests/contract/` (10 files)
- [x] T026 [P] [US1] Update all test file imports from `monocoque_agent_rc` to `agent_intercom` in `tests/integration/` (27 files including test_helpers.rs)
- [x] T027 [P] [US1] Update test assertions in `tests/contract/` that reference old **import paths** (`monocoque_agent_rc`) and old **string constants** (`.agentrc`, `monocoque-agent-rc` keychain/IPC names, `MONOCOQUE_` env vars). Do NOT update tool name assertion strings (`ask_approval`, `accept_diff`, etc.) — those move to Phase 4 after production tool names change.
- [x] T028 [P] [US1] Update test fixtures and string literals referencing `.agentrc` or `/monocoque` in `tests/unit/policy_tests.rs` and `tests/unit/policy_evaluator_tests.rs`
- [x] T029 [P] [US1] Update test fixtures referencing `MONOCOQUE_` env vars in `tests/unit/config_tests.rs` and `tests/unit/credential_loading_tests.rs`
- [x] T030 [P] [US1] Update integration test fixtures referencing old names in `tests/integration/ipc_server_tests.rs`, `tests/integration/policy_watcher_tests.rs`
- [x] T031 [US1] Run `cargo test` and confirm all tests pass (green gate) — EXIT GATE for Phase 3
- [x] T032 [US1] Run `grep -r "monocoque" src/ ctl/ tests/ config.toml Cargo.toml` and verify zero matches (S005, S006, S007)

**Checkpoint**: The codebase compiles and all tests pass with the new name. Zero "monocoque" references remain in source, tests, and config.

---

<!-- SECTION:DESCRIPTION:END -->
