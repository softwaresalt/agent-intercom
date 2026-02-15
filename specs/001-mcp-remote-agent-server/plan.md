# Implementation Plan: MCP Remote Agent Server (US11–US13 Addendum)

**Branch**: `001-mcp-remote-agent-server` | **Date**: 2026-02-14 | **Spec**: [spec.md](spec.md)
**Input**: User Stories 11, 12, 13 added to the existing feature specification on 2026-02-14.
**Predecessor**: Original plan (Phases 1–14) completed for US1–US10. This addendum covers three new user stories.

## Summary

Three new user stories extend the existing MCP Remote Agent Server:

1. **US11 — Slack Environment Variable Configuration** (P1): Formalize and validate the existing environment variable fallback for `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, and `SLACK_TEAM_ID`. Existing implementation satisfies the functional requirements; this phase adds explicit test coverage and improved error messaging.

2. **US12 — Dynamic Slack Channel Selection** (P2): Formalize and validate the existing `?channel_id=` query parameter on the SSE endpoint. Existing implementation satisfies the functional requirements; this phase adds test coverage and documentation.

3. **US13 — Service Rebranding** (P1): Rename from `monocoque-agent-rem` to `monocoque-agent-rc` across the entire codebase — binary names, crate name, keychain service, SurrealDB database, IPC pipe, documentation, and all internal references.

## Technical Context

**Language/Version**: Rust (stable, edition 2021) — unchanged
**Primary Dependencies**: Same as original plan — no new dependencies required for US11–US13
**Storage**: SurrealDB 1.5 embedded — database name changes from `agent_rem` to `agent_rc` (US13)
**Credential Storage**: OS keychain service name changes from `monocoque-agent-rem` to `monocoque-agent-rc` (US13). Env var names unchanged (US11).
**Testing**: `cargo test` — existing test infrastructure; new unit and integration tests for US11/US12 credential and channel override paths
**Project Type**: Single workspace, two binaries — primary binary renamed to `monocoque-agent-rc`, companion `monocoque-ctl` unchanged

### Impact Assessment

| Area | US11 | US12 | US13 |
|------|------|------|------|
| New code | Minimal (error message improvements) | None | None (string replacements only) |
| Test code | New unit tests for credential loading paths | New integration test for channel isolation | Test reference updates (~75 occurrences) |
| Source changes | 0–2 files | 0 files | ~5 source files (~36 occurrences) |
| Config changes | 0 | 0 | `Cargo.toml`, `config.toml` |
| Documentation | `quickstart.md` updates | `quickstart.md`, `config.toml` comments | All docs (rename references) |
| Risk level | Low | Low | Low (mechanical, compiler-verified) |

## Constitution Check

*GATE: Must pass before implementation. Validated against constitution.md v1.0.0.*

| Principle | Status | Evidence (US11–US13) |
|-----------|--------|----------------------|
| I. Safety-First Rust | PASS | No new `unsafe` code. No new `unwrap()`/`expect()`. Credential loading uses existing `Result`/`AppError` pattern. Rename is string-only — no logic changes. |
| II. MCP Protocol Fidelity | PASS | No MCP protocol changes. `monocoque/nudge` notification method unchanged. All tools remain unconditionally visible. |
| III. Test-First Development | PASS | US11 adds credential loading tests before any error message improvements. US12 adds channel isolation tests validating existing behavior. US13 uses `cargo build` + `cargo test` + grep as verification gates. |
| IV. Security Boundary Enforcement | PASS | Credential loading order (keychain > env var) maintained (FR-039). No relaxation of path validation or session owner binding. Keychain service name updated to new canonical name (FR-046). |
| V. Structured Observability | PASS | Tracing spans for credential loading already exist (source: keychain/env). No new span requirements. Post-rename, tracing output uses new service name. |
| VI. Single-Binary Simplicity | PASS | No new dependencies. Binary count unchanged (2). Rename does not affect architecture. |

**Gate status**: PASS — all 6 principles satisfied. No violations.

## Project Structure

### Source Code Changes (US13 rename scope)

```text
Cargo.toml                    # package name, [[bin]] name
config.toml                   # comments, ipc_name default
src/main.rs                   # binary name refs, tracing target
src/config.rs                 # keychain service name constant
src/persistence/db.rs         # SurrealDB database name
src/mcp/handler.rs            # server info name
src/mcp/resources/slack_channel.rs  # resource URI prefix
ctl/main.rs                   # CLI about text, IPC pipe name
tests/**/*.rs                 # extern crate / use references (~75)
README.md                     # project name, install instructions
specs/**/*.md                 # documentation references
.specify/memory/constitution.md  # project name references
.github/copilot-instructions.md # project name references
```

### New Test Files

```text
tests/unit/credential_loading_tests.rs     # US11: env var fallback paths
tests/integration/channel_override_tests.rs # US12: per-session channel isolation
```

## Complexity Tracking

No constitution violations identified. All changes are mechanical (US13) or test-coverage additions (US11, US12) with no architectural impact.

---

## Phase 15: User Story 11 — Slack Environment Variable Configuration (Priority: P1)

**Goal**: Validate and formalize the existing credential loading behavior; ensure error messages are clear and actionable.

**Prerequisites**: Phase 2 complete (config.rs, credential loading infrastructure exists)

**Independent Test**: Unset keychain entries, set SLACK_BOT_TOKEN/SLACK_APP_TOKEN/SLACK_TEAM_ID as env vars, start server, verify successful Slack connection.

### Analysis of Existing Implementation

The `load_credential()` function in `src/config.rs` already implements:
- OS keychain lookup via `keyring::Entry` (wrapped in `spawn_blocking`)
- Environment variable fallback when keychain fails
- Error return when both sources fail

**Gaps identified**:
1. Error messages may not explicitly name the expected keychain service and env var for the missing credential
2. `SLACK_TEAM_ID` optionality (FR-041) may not be distinct from required credentials (FR-040)
3. No dedicated test for the env-var-only path or the keychain-takes-precedence path

### Tasks

- [ ] T200 [US11] Write unit tests in `tests/unit/credential_loading_tests.rs`: test env-var-only credential loading (no keychain), keychain-takes-precedence when both sources exist, missing required credential error message content (must name both keychain service and env var), optional `SLACK_TEAM_ID` absent is not an error, empty env var treated as absent
- [ ] T201 [US11] Review and improve `load_credential()` error messages in `src/config.rs`: ensure error includes keychain service name (`monocoque-agent-rc` post-rename), expected env var name, and both resolution methods. Ensure `SLACK_TEAM_ID` is loaded with a separate non-failing path (optional credential)
- [ ] T202 [US11] Update `quickstart.md` with explicit environment variable setup instructions: list all three env vars, explain keychain-first precedence, document optional nature of SLACK_TEAM_ID
- [ ] T203 [US11] Add tracing span to credential loading: log which source (keychain or env var) was used for each credential at info level (never log the credential value itself)

**Checkpoint**: All credential loading paths tested, error messages are clear and actionable, quickstart documented.

---

## Phase 16: User Story 12 — Dynamic Slack Channel Selection (Priority: P2)

**Goal**: Validate and formalize the existing per-session channel override via SSE query parameter.

**Prerequisites**: Phase 2 complete (SSE transport, handler infrastructure exists)

**Independent Test**: Connect two SSE agents with different `?channel_id=` values, invoke `remote_log` from each, verify messages appear in different channels.

### Analysis of Existing Implementation

The SSE transport in `src/mcp/sse.rs` already implements:
- `extract_channel_id()` parses `channel_id` from URI query string
- Semaphore-protected inbox pattern passes channel_id to the factory closure
- `AgentRemServer::with_channel_override()` stores per-session channel
- `effective_channel_id()` returns override or default
- All tool handlers use `effective_channel_id()` consistently

**Gaps identified**:
1. No integration test for multi-session channel isolation
2. Config.toml comments don't document the `?channel_id=` parameter
3. `quickstart.md` doesn't mention the feature

### Tasks

- [ ] T204 [US12] Write integration test in `tests/integration/channel_override_tests.rs`: test SSE connection with `?channel_id=C_TEST` uses override, SSE connection without `?channel_id=` uses default, SSE connection with empty `?channel_id=` uses default, two concurrent SSE sessions with different channel_ids route independently
- [ ] T205 [US12] Update `config.toml` comments to document the `?channel_id=` query parameter on the SSE endpoint, with example `.vscode/mcp.json` configuration
- [ ] T206 [US12] Update `quickstart.md` with multi-workspace channel configuration instructions
- [ ] T207 [US12] Verify `extract_channel_id()` handles URL-encoded values and edge cases (multiple `channel_id` params, `channel_id` with no `=`)

**Checkpoint**: Channel override behavior validated with tests, documented in config and quickstart.

---

## Phase 17: User Story 13 — Service Rebranding to Remote Control (Priority: P1)

**Goal**: Rename all references from `monocoque-agent-rem` / `agent_rem` to `monocoque-agent-rc` / `agent_rc` across the entire codebase.

**Prerequisites**: US11 and US12 should be complete first (so renamed references are consistent from the start). However, US13 can be done first if preferred — it's independent.

**Independent Test**: After rename, `cargo build` produces `monocoque-agent-rc`, `cargo test` passes, grep finds zero remaining `agent.rem` references in non-changelog files.

### Rename Categories

| Category | Pattern to find | Replace with |
|----------|----------------|--------------|
| Cargo package | `monocoque-agent-rem` | `monocoque-agent-rc` |
| Rust crate name | `monocoque_agent_rem` | `monocoque_agent_rc` |
| Binary name | `monocoque-agent-rem` | `monocoque-agent-rc` |
| SurrealDB database | `agent_rem` | `agent_rc` |
| Keychain service | `monocoque-agent-rem` | `monocoque-agent-rc` |
| IPC pipe default | `monocoque-agent-rem` | `monocoque-agent-rc` |
| Documentation | `agent-rem` / `agent_rem` | `agent-rc` / `agent_rc` |

### Tasks

- [ ] T208 [US13] Rename Cargo.toml: update `[package] name` from `monocoque-agent-rem` to `monocoque-agent-rc` and `[[bin]] name` from `monocoque-agent-rem` to `monocoque-agent-rc`
- [ ] T209 [US13] Rename source code references in `src/`: update `monocoque-agent-rem` and `monocoque_agent_rem` to `monocoque-agent-rc` and `monocoque_agent_rc` across `main.rs`, `config.rs`, `persistence/db.rs`, `mcp/handler.rs`, `mcp/resources/slack_channel.rs`
- [ ] T210 [US13] Rename CLI references in `ctl/main.rs`: update `monocoque-agent-rem` / `monocoque_agent_rem` to `monocoque-agent-rc` / `monocoque_agent_rc`
- [ ] T211 [US13] Rename config.toml references: update comments and default values referencing `monocoque-agent-rem` to `monocoque-agent-rc`
- [ ] T212 [US13] Rename test references in `tests/`: update all `monocoque_agent_rem` crate references to `monocoque_agent_rc` (~75 occurrences)
- [ ] T213 [US13] Rename documentation references: update `README.md`, `quickstart.md`, all `specs/**/*.md` files, `.specify/memory/constitution.md`, `.github/copilot-instructions.md`
- [ ] T214 [US13] Verify compilation: run `cargo build` and confirm binary is named `monocoque-agent-rc`
- [ ] T215 [US13] Verify tests: run `cargo test` and confirm all tests pass with renamed crate
- [ ] T216 [US13] Verify naming consistency: grep the entire workspace for `agent.rem` (regex) and confirm zero matches in non-changelog files (SC-015)
- [ ] T217 [US13] Run `cargo clippy -- -D warnings` and confirm zero warnings

**Checkpoint**: Full rename complete — binary, crate, keychain, DB, IPC, docs all use `monocoque-agent-rc`. Zero `agent-rem` / `agent_rem` references remain.

---

## Dependencies & Execution Order

### Phase Dependencies

```text
Phase 15 (US11) ─── depends on ──→ Phase 2 (Foundational) ✅ complete
Phase 16 (US12) ─── depends on ──→ Phase 2 (Foundational) ✅ complete
Phase 17 (US13) ─── depends on ──→ Phase 2 (Foundational) ✅ complete
```

### Recommended Execution Order

```text
Option A (rename first — preferred):
  Phase 17 (US13: Rename) → Phase 15 (US11: Env Vars) → Phase 16 (US12: Channel)

Option B (tests first):
  Phase 15 (US11: Env Vars) → Phase 16 (US12: Channel) → Phase 17 (US13: Rename)
```

**Recommendation**: Option A — perform the rename first so that all new test files and documentation written for US11/US12 use the correct `monocoque-agent-rc` name from the start, avoiding double-editing.

### Parallel Opportunities

- T200–T203 (US11 tests/docs) can run in parallel with T204–T207 (US12 tests/docs) — they touch different files.
- T208–T213 (US13 rename) must be sequential within the phase (Cargo.toml first, then source, then tests, then docs).
- T214–T217 (US13 verification) must follow all rename tasks.

## Implementation Strategy

### Approach

All three user stories are low-risk, primarily involving test additions and mechanical renaming:

1. **US13 (Rename)**: Execute first. Mechanical find-replace across the codebase. Compiler verification (`cargo build`) catches any missed references. Grep verification (SC-015) confirms completeness.

2. **US11 (Env Vars)**: Add test coverage for credential loading paths. Minor error message improvements. No architectural changes.

3. **US12 (Channel Selection)**: Add test coverage for channel override isolation. Documentation updates. No code changes expected.

### Verification Gates

Each phase has a clear pass/fail gate:

| Phase | Gate | Command |
|-------|------|---------|
| 15 (US11) | All credential tests pass | `cargo test credential_loading` |
| 16 (US12) | All channel override tests pass | `cargo test channel_override` |
| 17 (US13) | Build + test + grep zero matches | `cargo build && cargo test && grep -r "agent.rem" src/ tests/ ctl/ Cargo.toml` |

### Estimated Effort

| Phase | Tasks | Estimated size |
|-------|-------|---------------|
| 15 (US11) | 4 | Small (tests + docs) |
| 16 (US12) | 4 | Small (tests + docs) |
| 17 (US13) | 10 | Medium (mechanical, high file count) |
| **Total** | **18** | |
