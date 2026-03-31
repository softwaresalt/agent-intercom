---
id: TASK-005.04
title: "005 - User Story 1 — Dual-Mode Startup (Priority: P1) 🎯 MVP"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-005
dependencies: []
ordinal: 5040
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Add `--mode` CLI flag to select MCP or ACP mode at startup

**Independent Test**: Start server with `--mode mcp` and `--mode acp`, verify each mode initializes correctly

### Tests (S001–S007)

- [x] T015 [P] [US1] Write unit test for `Mode` enum CLI parsing (mcp default, acp explicit, invalid) in `tests/unit/cli_tests.rs` — covers S001, S002, S003, S006
- [x] T016 [P] [US1] Write unit test for ACP config validation (missing host_cli) in `tests/unit/config_tests.rs` — covers S004, S005

### Implementation

- [x] T017 [US1] Add `Mode` enum (`Mcp`, `Acp`) with `ValueEnum` derive to `src/main.rs` CLI struct
- [x] T018 [US1] Add `--mode` flag to `Cli` struct in `src/main.rs` with `default_value_t = Mode::Mcp`
- [x] T019 [US1] Add ACP config validation in `src/config.rs` — validate `host_cli` is non-empty and exists when ACP mode selected
- [x] T020 [US1] Branch `run()` in `src/main.rs` on mode: MCP path unchanged, ACP path skips MCP transport startup
- [x] T021 [US1] Verify MCP mode regression — all 9 tools visible and functional with no changes (S007)

**Checkpoint**: Server starts in MCP or ACP mode; MCP behavior unchanged; ACP validates config

---

<!-- SECTION:DESCRIPTION:END -->
