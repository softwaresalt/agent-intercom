# Tasks: Agent Intercom Release

**Input**: Design documents from `/specs/003-agent-intercom-release/`  
**Prerequisites**: plan.md (required), spec.md (required), SCENARIOS.md, research.md, data-model.md, contracts/

**Tests**: TDD is mandated by the project constitution (Principle III). Write tests first, verify they fail (red), then implement (green). For mechanical rename phases, compilation (`cargo check`) serves as the validation gate.

**Organization**: Tasks follow the plan.md phase ordering, which accounts for technical dependencies. Each phase maps to a user story and is independently testable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/`, `ctl/` at repository root
- Tool handlers: `src/mcp/tools/{tool_name}.rs`
- Test tiers: `tests/unit/`, `tests/contract/`, `tests/integration/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify baseline state before making changes

- [x] T001 Verify `003-agent-intercom-release` branch is checked out and clean (`git status`)
- [x] T002 Run `cargo test` and confirm all existing tests pass before any modifications

---

## Phase 2: Foundational (Cargo + Core Source Rename)

**Purpose**: Rename all Rust crate identifiers and core constants so the project compiles under the new name. This is the blocking prerequisite for ALL subsequent phases.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete and `cargo check` passes.

**Scenarios covered**: S001–S014

### Core Cargo Rename

- [x] T003 Update `Cargo.toml` package name from `monocoque-agent-rc` to `agent-intercom`, rename both `[[bin]]` entries to `agent-intercom` and `agent-intercom-ctl`, update `repository`/`homepage`/`description` metadata (FR-001, FR-002, FR-010)
- [x] T004 Update `src/lib.rs` crate root: module doc comment, any `extern crate` references, re-export docs mentioning old name
- [x] T005 Update `src/main.rs` binary entry point: crate references, about text, CLI metadata to `agent-intercom`
- [x] T006 Update `ctl/main.rs` CLI entry point: crate references, about text, CLI metadata to `agent-intercom-ctl`

### Core String Constants

- [x] T007 [P] Update `KEYCHAIN_SERVICE` constant from `"monocoque-agent-rc"` to `"agent-intercom"` in `src/config.rs` (FR-006)
- [x] T008 [P] Update IPC pipe/socket name from `"monocoque-agent-rc"` to `"agent-intercom"` in `src/ipc/socket.rs` and `ctl/main.rs` (FR-007)
- [x] T009 [P] Update environment variable prefix from `MONOCOQUE_` to `INTERCOM_` in `src/config.rs` and `src/orchestrator/spawner.rs` (FR-008)
- [x] T010 [P] Update policy directory constants from `".agentrc"` to `".intercom"` and `".agentrc/settings.json"` to `".intercom/settings.json"` in `src/policy/loader.rs` and `src/policy/watcher.rs` (FR-003)
- [x] T011 [P] Update Slack slash command root from `"/monocoque"` to `"/intercom"` in `src/slack/commands.rs` (FR-005)

### Global Import Update

- [x] T012 Replace all `use monocoque_agent_rc::` with `use agent_intercom::` across all files in `src/` directory
- [x] T013 Replace all `use monocoque_agent_rc::` with `use agent_intercom::` across all files in `tests/` directory
- [x] T014 Replace all `monocoque_agent_rc` references in `ctl/main.rs` extern imports
- [x] T015 Replace all occurrences of "monocoque" in source code doc comments (`///`), module doc comments (`//!`), and log messages across `src/` (FR-009)
- [x] T015a Rename `AgentRcServer` struct and all its references in `src/mcp/handler.rs`, `src/mcp/context.rs`, `tests/` — the "Rc" suffix is residue from the old "monocoque-agent-**rc**" branding (see Analysis finding U1)
- [x] T016 Replace all occurrences of "monocoque" string literals in `config.toml`
- [x] T017 Run `cargo check` and confirm zero compilation errors — EXIT GATE for Phase 2

**Checkpoint**: Codebase compiles under the new name. All `use` paths resolve. Constants reference `agent-intercom`.

---

## Phase 3: User Story 1 — Consistent Product Identity (Priority: P1) 🎯 MVP

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

## Phase 4: User Story 4 — Intercom-Themed Tool Names (Priority: P4)

**Goal**: All 9 MCP tools use intercom-themed names. MCP ServerInfo reports `"agent-intercom"` with the crate version. Old tool names are rejected.

**Independent Test**: Connect an MCP client, call `tools/list`, verify 9 tools with new names. Call an old tool name and confirm error.

**Scenarios covered**: S021–S027

### Tests for User Story 4 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T033 [P] [US4] Write contract test verifying `tools/list` returns 9 tools with new names (check_clearance, check_diff, auto_check, transmit, standby, ping, broadcast, reboot, switch_freq) in `tests/contract/schema_tests.rs` or new file (S022)
- [x] T034 [P] [US4] Write contract test verifying ServerInfo returns `name: "agent-intercom"` and `version` matches `env!("CARGO_PKG_VERSION")` in `tests/contract/schema_tests.rs` (S021)
- [x] T035 [P] [US4] Write contract test verifying old tool name `ask_approval` returns error when called in `tests/contract/` (S023)
- [x] T036 [P] [US4] Write contract test verifying each renamed tool preserves its input schema (check_clearance has same fields as ask_approval, etc.) in `tests/contract/` (S024, S025)
- [x] T037 [P] [US4] Write integration test verifying `call_tool` with empty string name returns error in `tests/integration/call_tool_dispatch_tests.rs` (S027)
- [x] T038 [US4] Run tests and confirm new assertions FAIL (red gate) before proceeding to implementation

### Implementation for User Story 4

- [x] T039 [US4] Update `Tool::name` fields for all 9 tools in tool registration in `src/mcp/handler.rs` per mapping: ask_approval→check_clearance, accept_diff→check_diff, check_auto_approve→auto_check, forward_prompt→transmit, wait_for_instruction→standby, heartbeat→ping, remote_log→broadcast, recover_state→reboot, set_operational_mode→switch_freq (FR-025)
- [x] T040 [US4] Update `ToolRouter` dispatch keys to match new tool names in `src/mcp/handler.rs`
- [x] T041 [US4] Set `ServerInfo { name: "agent-intercom", version: env!("CARGO_PKG_VERSION") }` in `src/mcp/handler.rs` (FR-004, FR-037)
- [x] T042 [P] [US4] Update all tool name references in `src/mcp/tools/mod.rs` (tool module re-exports, any name constants)
- [x] T042a [P] [US4] Update test assertion strings in `tests/contract/` that reference old tool names (`"ask_approval"` → `"check_clearance"`, `"accept_diff"` → `"check_diff"`, etc.) — deferred from Phase 3 T027 to avoid red state between phases
- [x] T043 [P] [US4] Update tool name references in copilot-instructions at `.github/copilot-instructions.md` (FR-026)
- [x] T044 [P] [US4] Update tool name references in all agent files in `.github/agents/` directory (FR-026)
- [x] T045 [US4] Run `cargo test` and confirm all tests pass including new contract tests (green gate) — EXIT GATE for Phase 4
- [x] T046 [US4] Verify all 9 tools visible regardless of config (S026): run contract test with minimal config

**Checkpoint**: All tools have intercom-themed names, ServerInfo is correct, old names are rejected, copilot-instructions updated.

---

## Phase 5: User Story 2 — Reliable Slack Notifications (Priority: P2)

**Goal**: Every significant lifecycle event produces a Slack notification. Five identified notification gaps are fixed: accept_diff success, accept_diff conflict, accept_diff force-apply, no-channel error for ask_approval, and rejection delivery confirmation.

**Independent Test**: Run HITL test scenarios covering a full agent session (proposal → approve → apply → continue → standby). Operator receives at minimum 5 distinct Slack notifications.

**Scenarios covered**: S028–S041

### Tests for User Story 2 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation (TDD required)**

- [x] T047 [P] [US2] Write contract test verifying `check_diff` (accept_diff) posts Slack message on successful patch apply in `tests/contract/accept_diff_tests.rs` (S028)
- [x] T048 [P] [US2] Write contract test verifying `check_diff` posts Slack alert on patch conflict (hash mismatch without force) in `tests/contract/accept_diff_tests.rs` (S029)
- [x] T049 [P] [US2] Write contract test verifying `check_diff` posts Slack warning on force-apply in `tests/contract/accept_diff_tests.rs` (S030)
- [x] T050 [P] [US2] Write contract test verifying `check_diff` applies patch without Slack message when no channel configured in `tests/contract/accept_diff_tests.rs` (S031)
- [x] T051 [P] [US2] Write contract test verifying `check_diff` posts Slack confirmation for new file write in `tests/contract/accept_diff_tests.rs` (S032)
- [x] T052 [P] [US2] Write contract test verifying `check_clearance` (ask_approval) returns descriptive error when no Slack channel configured in `tests/contract/ask_approval_tests.rs` (S033)
- [x] T053 [P] [US2] Write contract test verifying `check_clearance` returns error when Slack service is unavailable in `tests/contract/ask_approval_tests.rs` (S034)
- [x] T054 [P] [US2] Write contract test verifying rejection posts confirmation to Slack in `tests/contract/ask_approval_tests.rs` (S036, S037)
- [x] T055 [P] [US2] Write contract test verifying `transmit` (forward_prompt) returns error when no Slack channel configured in `tests/contract/forward_prompt_tests.rs` (S040)
- [x] T056 [P] [US2] Write contract test verifying `standby` (wait_for_instruction) returns error when no Slack channel configured — new file `tests/contract/wait_for_instruction_tests.rs` or add to existing (S041)
- [x] T057 [US2] Run tests and confirm all new notification assertions FAIL (red gate) before proceeding

### Implementation for User Story 2

- [x] T058 [US2] Add Slack success notification in `src/mcp/tools/accept_diff.rs` after successful patch application: post file path + bytes written via `SlackService` (FR-012)
- [x] T059 [US2] Add Slack alert notification in `src/mcp/tools/accept_diff.rs` when patch conflict (hash mismatch) detected: post conflict details via `SlackService` (FR-013)
- [x] T060 [US2] Ensure Slack warning notification in `src/mcp/tools/accept_diff.rs` always fires on `force: true` with hash mismatch (FR-014)
- [x] T061 [US2] Add descriptive error return in `src/mcp/tools/ask_approval.rs` when no Slack channel configured (replace silent block with `CallToolResult` error) (FR-015)
- [x] T062 [US2] Add Slack notification for rejection delivery confirmation in `src/mcp/tools/ask_approval.rs` or `src/slack/events.rs` (FR-018)
- [x] T063 [P] [US2] Add Block Kit builder for accept_diff success notification in `src/slack/blocks.rs`
- [x] T064 [P] [US2] Add Block Kit builder for accept_diff conflict alert in `src/slack/blocks.rs`
- [x] T065 [P] [US2] Add Block Kit builder for accept_diff force-apply warning in `src/slack/blocks.rs`
- [x] T066 [P] [US2] Add Block Kit builder for rejection delivery confirmation in `src/slack/blocks.rs`
- [x] T067 [US2] Add error return for `transmit` (forward_prompt) and `standby` (wait_for_instruction) when no Slack channel configured in `src/mcp/tools/forward_prompt.rs` and `src/mcp/tools/wait_for_instruction.rs` (FR-016, FR-017 — verify existing behavior; add error path if missing)
- [x] T068 [US2] Run `cargo test` and confirm all notification tests pass (green gate) — EXIT GATE for Phase 5
- [x] T069 [US2] Run `cargo clippy -- -D warnings` and confirm zero warnings on notification code

**Checkpoint**: All 5 notification gaps fixed. Every lifecycle event produces a Slack notification when a channel is configured. No silent failures.

---

## Phase 6: User Story 3 — Comprehensive Product Documentation (Priority: P3)

**Goal**: Complete, accurate documentation covering installation, configuration, all features, all tools (with new names), Slack commands (under `/intercom`), CLI subcommands, and developer contribution workflow. A migration guide covers the transition for existing users.

**Independent Test**: A new user follows the documentation from download to first approval workflow in under 30 minutes. Zero "monocoque" references in docs (except migration guide referencing old name for context).

**Scenarios covered**: S042–S047

### Tests for User Story 3 ⚠️

- [x] T070 [US3] After all documentation is written, verify zero "monocoque" occurrences in `docs/` (except `docs/migration-guide.md` which references the old name for context) (S047)

### Implementation for User Story 3

- [x] T071 [P] [US3] Rewrite `README.md` with new product name, binary names (`agent-intercom`, `agent-intercom-ctl`), quick start instructions, and updated feature list (FR-019)
- [x] T072 [P] [US3] Update `docs/setup-guide.md` with new Slack app creation steps (using `/intercom` command), credential configuration (keychain service `agent-intercom`, `INTERCOM_` env vars), and first run instructions (FR-020)
- [x] T073 [P] [US3] Update `docs/user-guide.md` with all 9 MCP tools using new names (check_clearance, check_diff, auto_check, transmit, standby, ping, broadcast, reboot, switch_freq), all Slack commands under `/intercom`, workspace policy config in `.intercom/settings.json` (FR-021)
- [x] T074 [P] [US3] Create new `docs/developer-guide.md` covering build instructions, test commands (`cargo test`), project structure, coding conventions, contribution workflow, and the approval workflow process (FR-022)
- [x] T075 [P] [US3] Complete `agent-intercom-ctl` CLI documentation in `docs/user-guide.md` or separate `docs/cli-reference.md` with usage examples for every subcommand (FR-023)
- [x] T076 [P] [US3] Create new `docs/migration-guide.md` documenting transition steps: keychain rename, env var prefix change (`MONOCOQUE_` → `INTERCOM_`), Slack app command update (`/monocoque` → `/intercom`), policy directory rename (`.agentrc/` → `.intercom/`), `mcp.json` URL update, binary name change (FR-024)
- [x] T077 [P] [US3] Update `docs/REFERENCE.md` with new tool names, new Slack commands, updated configuration reference
- [x] T078 [US3] Update all doc comments in `src/` that reference user-facing concepts to use new names (final sweep)
- [x] T079 [US3] Run search for "monocoque" across `docs/` and `README.md` — verify matches only in migration-guide.md context (S047) — EXIT GATE for Phase 6

**Checkpoint**: Documentation is complete, accurate, and reflects the new product name throughout.

---

## Phase 7: User Story 6 — Release Pipeline (Priority: P6)

**Goal**: A GitHub Actions workflow that triggers on semver tags, produces cross-platform binaries, generates a changelog, and publishes to GitHub Releases. Feature flags gate unreleased capabilities.

**Independent Test**: Trigger the release workflow on a test tag; confirm it produces correctly named archives for all 4 target platforms with proper version metadata.

**Scenarios covered**: S048–S056

### Tests for User Story 6 ⚠️

- [x] T080 [P] [US6] Write unit test verifying `--version` flag outputs version matching `env!("CARGO_PKG_VERSION")` for `agent-intercom` binary in `tests/unit/` or `tests/integration/` (S053)
- [x] T081 [P] [US6] Write unit test verifying feature flag compile-time gating works: a `#[cfg(feature = "...")]` gated function is absent when feature is not enabled (S054, S055)
- [x] T082 [US6] Run tests and confirm new assertions FAIL (red gate)

### Implementation for User Story 6

- [x] T083 [US6] Add `--version` flag handling using `env!("CARGO_PKG_VERSION")` to `src/main.rs` (clap `version` attribute) and `ctl/main.rs` (FR-037)
- [x] T084 [US6] Add `[features]` section to `Cargo.toml` with `default = []` and placeholder feature flag (e.g., `rmcp-upgrade = []`) (FR-036)
- [x] T085 [P] [US6] Create `.github/workflows/release.yml` with trigger on `v*.*.*` tags and build matrix for 4 targets: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin` (FR-033)
- [x] T086 [P] [US6] Add `cross` compilation steps in release workflow for Linux and macOS targets
- [x] T087 [P] [US6] Add archive packaging step to produce `agent-intercom-v{version}-{target}.{zip|tar.gz}` containing server binary, CLI binary, and `config.toml.example` (FR-034)
- [x] T088 [P] [US6] Add `git-cliff` changelog generation step in release workflow (FR-035)
- [x] T089 [US6] Add `softprops/action-gh-release` step to publish archives and changelog to GitHub Releases
- [x] T090 [US6] Add failure handling: ensure partial platform failure aborts entire release (S056)
- [x] T091 [US6] Create `config.toml.example` from current `config.toml` with placeholder values for release archive inclusion
- [x] T092 [US6] Run `cargo test` with feature flag tests passing (green gate) — EXIT GATE for Phase 7
- [x] T093 [US6] Run local `cargo build --release` and verify binary names match `agent-intercom` / `agent-intercom-ctl` (S002, S003)

**Checkpoint**: Release pipeline workflow exists, feature flags work, --version outputs correct version, local release build produces correctly named binaries.

---

## Phase 8: User Story 5 — rmcp 0.13 Upgrade (Priority: P5)

**Goal**: The MCP SDK dependency is upgraded from rmcp 0.5 to 0.13.0. The Streamable HTTP transport replaces the removed SSE transport. Both HTTP and stdio transports work. All existing functionality is preserved.

**Independent Test**: `cargo test` passes with rmcp 0.13.0. Manual verification that HTTP and stdio transports work with MCP clients.

**Scenarios covered**: S057–S062

### Tests for User Story 5 ⚠️

> **NOTE: Write tests FIRST. Some existing integration tests will need rewiring for the new transport API.**

- [ ] T094 [P] [US5] Write integration test verifying StreamableHttpService on `/mcp` endpoint accepts HTTP POST and returns tool list in `tests/integration/` (S057)
- [ ] T095 [P] [US5] Write integration test verifying stdio transport still works with rmcp 0.13 in `tests/integration/` (S058)
- [ ] T096 [P] [US5] Write integration test verifying old `/sse` endpoint returns redirect or 410 Gone in `tests/integration/` (S060)
- [ ] T097 [P] [US5] Write integration test verifying concurrent MCP connections are handled independently in `tests/integration/` (S061)
- [ ] T098 [P] [US5] Write integration test verifying graceful handling of dropped connections in `tests/integration/` (S062)
- [ ] T099 [US5] Run tests and confirm new transport assertions FAIL (red gate)

### Implementation for User Story 5

- [ ] T100 [US5] Update `Cargo.toml`: change rmcp version from `0.5` to `0.13.0`, change feature from `transport-sse-server` to `transport-streamable-http-server` (FR-028)
- [ ] T101 [US5] Rewrite `src/mcp/sse.rs` — replace `SseServer`/`SseServerConfig` with `StreamableHttpService`, implement `SessionManager` trait (or use `LocalSessionManager`), replace `/sse` + `/message` two-endpoint model with single `/mcp` POST endpoint (FR-031) — **CRITICAL: this is a complete rewrite**
- [ ] T102 [US5] Add backward-compatible route: mount redirect from `/sse` to `/mcp` with deprecation header in `src/mcp/sse.rs` (Design Decision D-003)
- [ ] T103 [US5] Update `src/mcp/handler.rs`: adapt `AgentRcServer` (or successor) to rmcp 0.13 `ServerHandler` trait, handle any new required methods (FR-029)
- [ ] T104 [US5] Update tool registration to use rmcp 0.13 patterns (`ToolRouter` / proc macros / new API) in `src/mcp/handler.rs` (FR-030)
- [ ] T105 [US5] Verify `src/mcp/transport.rs` stdio transport compiles and works with rmcp 0.13 (FR-031)
- [ ] T106 [P] [US5] Update `src/mcp/resources/` resource providers to compile with rmcp 0.13 type changes
- [ ] T107 [US5] Update all integration tests that use SSE transport setup in `tests/integration/` to use new StreamableHttpService transport
- [ ] T108 [US5] Run full `cargo test` suite — all unit, contract, and integration tests pass (FR-032) (S059) — EXIT GATE for Phase 8
- [ ] T109 [US5] Run `cargo clippy -- -D warnings` and confirm zero warnings with rmcp 0.13 (SC-007)

**Checkpoint**: rmcp 0.13 upgrade complete. Both HTTP (Streamable) and stdio transports working. All tests pass. Zero clippy warnings.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: Final verification, cleanup, and cross-cutting improvements

- [ ] T110 [P] Run final `grep -r "monocoque" .` across entire workspace — verify matches only in git history, archived ADR/spec docs, and migration guide context (SC-001)
- [ ] T111 [P] Update `.github/copilot-instructions.md` with final tool names, binary names, policy directory, env var prefix — complete final sweep (FR-026)
- [ ] T112 [P] Update all remaining `.github/agents/` files with new tool names and binary names
- [ ] T113a Verify automated success criteria: SC-001 (zero monocoque matches), SC-002 (binary names), SC-003 (all tests pass), SC-006 (release archives), SC-007 (clippy clean), SC-008 (contract tests pass)
- [ ] T113b Verify HITL success criteria: SC-004 (operator receives ≥5 distinct Slack notifications during full agent session), SC-005 (new user follows docs end-to-end in <30 min) — requires live server + Slack
- [ ] T114 Run full quality gate sequence: `cargo check` → `cargo clippy -- -D warnings` → `cargo fmt --all -- --check` → `cargo test`
- [ ] T115 Run `specs/003-agent-intercom-release/quickstart.md` validation: execute each phase verification step
- [ ] T116 Draft constitution amendment v2.0.0: update title from "monocoque-agent-rc" to "agent-intercom", Principle II rmcp version 0.5 → 0.13, Principle VI binary names, Technical Constraints rmcp version + transport feature. Document as MAJOR bump (binary rename + SDK version change). Stage in `.specify/memory/constitution.md` for post-merge ratification.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — **BLOCKS all subsequent phases**
- **US1 (Phase 3)**: Depends on Phase 2 — test imports + assertions need compilable crate
- **US4 (Phase 4)**: Depends on Phase 3 — tool name tests reference new tool names
- **US2 (Phase 5)**: Depends on Phase 4 — notification tests reference new tool names
- **US3 (Phase 6)**: Depends on Phases 3–5 — documentation must reflect final functional state
- **US6 (Phase 7)**: Depends on Phase 3 — needs final binary names; can run parallel to Phase 5/6
- **US5 (Phase 8)**: Depends on Phase 4 — most isolated; highest risk; should be last functional phase
- **Polish (Phase 9)**: Depends on all prior phases

### User Story Dependencies

```text
Phase 1 (Setup)
    │
    ▼
Phase 2 (Foundational) ── BLOCKS ALL ──┐
    │                                    │
    ▼                                    │
Phase 3 (US1: Identity)                  │
    │                                    │
    ├──────────────────┐                 │
    ▼                  ▼                 │
Phase 4 (US4: Tools)  Phase 7 (US6: Release)
    │                                    │
    ├──────────────────┐                 │
    ▼                  ▼                 │
Phase 5 (US2: Slack)  Phase 8 (US5: rmcp)
    │                                    │
    ▼                                    │
Phase 6 (US3: Docs) ◄───────────────────┘
    │
    ▼
Phase 9 (Polish)
```

### Within Each User Story

- Tests (marked with ⚠️) MUST be written and FAIL before implementation
- Compilation gate (`cargo check`) after each structural change
- Test gate (`cargo test`) at each phase EXIT GATE
- [P] tasks within a phase can run in parallel
- Non-[P] tasks must execute sequentially

### Parallel Opportunities

- All [P] tasks within a phase can run in parallel (different files, no dependencies)
- **Phase 4 (US4) and Phase 7 (US6)** can run in parallel after Phase 3 completes
- **Phase 5 (US2) and Phase 8 (US5)** can run in parallel after Phase 4 completes
- Within each phase, all Block Kit builder tasks [P] can run in parallel with each other
- Within Phase 2, all constant update tasks (T007–T011) can run in parallel

---

## Parallel Example: Phase 2 (Foundational)

```text
# Sequential: Cargo.toml must be updated first
T003: Update Cargo.toml package name and binary names

# Then crate roots (can be parallel):
T004: Update src/lib.rs            ──┐
T005: Update src/main.rs            ├── Parallel
T006: Update ctl/main.rs           ──┘

# Then constants (all parallel):
T007: KEYCHAIN_SERVICE             ──┐
T008: IPC pipe name                 │
T009: Env var prefix                ├── Parallel
T010: Policy directory              │
T011: Slack command root           ──┘

# Then global imports (parallel per directory):
T012: src/ imports                 ──┐
T013: tests/ imports                ├── Parallel
T014: ctl/ imports                 ──┘

# Then sweep + gate:
T015: Doc comments sweep
T016: config.toml update
T017: cargo check gate
```

---

## Parallel Example: Phase 5 (US2 — Slack Notifications)

```text
# All test tasks in parallel (different test files):
T047: accept_diff success test     ──┐
T048: accept_diff conflict test     │
T049: force-apply test              │
T050: no channel test               ├── Parallel
T051: new file write test           │
T052: ask_approval no channel test  │
T053: Slack unavailable test        │
T054: rejection confirmation test   │
T055: transmit no channel test      │
T056: standby no channel test      ──┘

# Red gate:
T057: Confirm all FAIL

# Block Kit builders in parallel:
T063: success builder              ──┐
T064: conflict builder              ├── Parallel
T065: force-apply builder           │
T066: rejection builder            ──┘

# Then handler implementations (sequential per handler):
T058–T062, T067: Handler changes

# Green gate:
T068: cargo test
T069: cargo clippy
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: User Story 1 (Product Identity)
4. **STOP and VALIDATE**: `cargo build --release` produces `agent-intercom` binaries; `cargo test` passes; zero "monocoque" in source
5. Deploy/demo if ready — the product is usable under its new name

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add US1 (Identity) → Test independently → MVP!
3. Add US4 (Tool Names) → Test independently → Tools renamed
4. Add US2 (Notifications) → Test independently → Full operator awareness
5. Add US3 (Docs) → Validate end-to-end → Documentation complete
6. Add US6 (Release) → Test pipeline → Release-ready
7. Add US5 (rmcp Upgrade) → Test transports → Fully upgraded
8. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational + US1 together (sequential dependency)
2. Once US1 is done:
   - Developer A: US4 (Tool Names) → US2 (Notifications)
   - Developer B: US6 (Release Pipeline)
3. Once US4 is done:
   - Developer A: US2 (Slack Notifications)
   - Developer C: US5 (rmcp Upgrade)
4. After all functional phases → US3 (Docs) and Polish

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- Phase 7 (Release) and Phase 8 (rmcp) can overlap after Phase 4
- TDD is required by constitution Principle III: write tests first, verify they fail
- Tool handler files on disk are NOT renamed (Design Decision D-002)
- Commit after each phase completion (at each EXIT GATE checkpoint)
- Stop at any checkpoint to validate — each phase is a stable increment
