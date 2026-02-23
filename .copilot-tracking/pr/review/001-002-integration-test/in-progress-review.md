<!-- markdownlint-disable-file -->
# PR Review Status: 001-002-integration-test

## Review Status

* Phase: 3 — Collaborative Review
* Last Updated: 2026-02-23
* Summary: Comprehensive integration test suite + architectural fixes for Slack Socket Mode race, auto-session lifecycle, policy simplification, and `.agentrc` rename

## Branch and Metadata

* Normalized Branch: `001-002-integration-test`
* Source Branch: `001-002-integration-test`
* Base Branch: `main`
* Linked Work Items: Spec 001 (MCP Remote Agent Server), Spec 002 (SQLite Migration)

## Commits (10 on branch)

| Hash | Message |
|------|---------|
| 0c96f74 | Committing changes |
| db5995a | Committing current changes to the branch |
| 09fbeec | chore: gitignore SQLite runtime database files and untrack data/*.db |
| e754a39 | Reordered autoApprove items |
| 148d7b4 | docs: rename .monocoque to .agentrc and enhance approval workflow instructions |
| 6b49997 | chore(tracking): phase 5 memory and final checkpoint for 001-002-integration-test |
| 75be2ac | chore(spec): phase 5 — polish gates complete (T029-T032) |
| f32a046 | test(integration): phase 4 — MCP transport dispatch tests (T022-T028) |
| 3b4cc05 | feat(001-002-integration-test): complete phase 3 - ipc server tests |
| daa32a9 | feat(001-002-integration-test): complete phase 2 - policy hot-reload tests |

## Diff Mapping (Source + Test Files)

| File | Type | New Lines | Notes |
|------|------|-----------|-------|
| `src/config.rs` | Modified | 68 | `channel_id` default, db path rename, `SLACK_MEMBER_IDS` loader |
| `src/main.rs` | Modified | 259 | Transport enum, `--port`/`--transport` CLI, socket mode deferred start, shutdown timeout |
| `src/mcp/handler.rs` | Modified | 221 | `with_overrides()`, `effective_channel_id()→Option`, `on_initialized`, `get_info()` |
| `src/mcp/sse.rs` | Modified | 9 | Minor |
| `src/mcp/tools/*.rs` | Modified | Various | Minor adjustments to channel handling |
| `src/slack/client.rs` | Modified | 44 | `socket_task: Option`, `start()` no app_state, new `start_socket_mode()` |
| `src/models/policy.rs` | Modified | 21 | `commands` → `auto_approve_commands` (regex, with alias) |
| `src/policy/evaluator.rs` | Modified | 41 | Regex command matching, removed global allowlist gate |
| `src/policy/loader.rs` | Modified | 34 | `.monocoque` → `.agentrc`, removed global_commands param |
| `src/policy/watcher.rs` | Modified | 24 | `.monocoque` → `.agentrc`, Default impl, removed global_commands |
| `src/orchestrator/spawner.rs` | Modified | 6 | Minor |
| `tests/integration/on_initialized_tests.rs` | Added | 1–380 | Session lifecycle on connect |
| `tests/integration/shutdown_recovery_tests.rs` | Added | 1–369 | Graceful shutdown state |
| `tests/integration/stall_escalation_tests.rs` | Added | 1–323 | Stall detection |
| `tests/integration/session_manager_tests.rs` | Added | 1–289 | Session management |
| `tests/integration/checkpoint_manager_tests.rs` | Added | 1–301 | Checkpoint management |
| `tests/integration/handler_accept_diff_tests.rs` | Added | 1–356 | Diff application |
| `tests/integration/handler_auto_approve_tests.rs` | Added | 1–227 | Auto-approve |
| `tests/integration/handler_blocking_tests.rs` | Added | 1–605 | Blocking tools |
| `tests/integration/handler_edge_case_tests.rs` | Added | 1–500 | Edge cases |
| `tests/integration/handler_heartbeat_tests.rs` | Added | 1–183 | Heartbeat |
| `tests/integration/handler_mode_tests.rs` | Added | 1–154 | Mode mgmt |
| `tests/integration/handler_recover_tests.rs` | Added | 1–298 | Recovery |
| `tests/integration/handler_remote_log_tests.rs` | Added | 1–94 | Remote log |
| `tests/integration/health_endpoint_tests.rs` | Added | 1–111 | Health check |
| `tests/integration/ipc_server_tests.rs` | Added | 1–460 | IPC server |
| `tests/integration/mcp_dispatch_tests.rs` | Added | 1–397 | MCP dispatch |
| `tests/integration/call_tool_dispatch_tests.rs` | Added | 1–336 | Tool dispatch |
| `tests/integration/policy_watcher_tests.rs` | Added | 1–229 | Policy hot-reload |
| `tests/integration/test_helpers.rs` | Added | 1–167 | Shared test infra |
| `tests/unit/policy_evaluator_tests.rs` | Modified | 154 | Updated for regex + .agentrc |
| `tests/unit/policy_tests.rs` | Modified | 96 | Updated for auto_approve_commands |
| `tests/unit/config_tests.rs` | Modified | 72 | Updated for SLACK_MEMBER_IDS |
| `.github/workflows/release.yml` | Added | 110 | CI/CD release pipeline |
| `docs/REFERENCE.md` | Added | 1626 | Full API reference |
| `docs/setup-guide.md` | Added | 361 | Setup guide |
| `docs/user-guide.md` | Added | 338 | User guide |

## Instruction Files Reviewed

* `.github/copilot-instructions.md`: Primary — Rust conventions, quality gates, architecture reference
* `rustfmt.toml`: Formatting (max_width=100, edition=2021)
* `Cargo.toml`: Dependency versions and workspace lints

## Phase 1 Actions

* [x] Normalized branch name: `001-002-integration-test`
* [x] Created tracking directory
* [x] Captured `git diff --stat`, config/handler/slack/policy diffs
* [x] Read key source files and new test modules
* [x] Ran `cargo test` (pending results)
* [x] Ran `cargo clippy -- -D warnings` (pending results)

## Review Items

### 🔍 In Review

#### RI-001: `match_command_pattern` compiles regex on every evaluation

* File: `src/policy/evaluator.rs`
* Category: Performance
* Severity: Medium

**Description**: Each call to `PolicyEvaluator::evaluate()` compiles every regex pattern in `auto_approve_commands` from scratch via `Regex::new(pattern)`. Policy evaluation runs on every tool call, so this adds regex compilation overhead proportional to the number of patterns per call.

**Current Code**:
```rust
fn match_command_pattern(patterns: &[String], command: &str) -> Option<String> {
    for pattern in patterns {
        match Regex::new(pattern) {  // compiled fresh each call
            Ok(re) => { ... }
```

**Suggested Resolution**: Use `regex::RegexSet`. Full implementation details added to `.context/backlog.md` for next feature branch.

* User Decision: ❌ **Deferred** — tracked in backlog for next feature branch

---

#### RI-002: Hardcoded `"agent:local"` owner string

* File: `src/mcp/handler.rs`
* Category: Code Quality / Maintainability
* Severity: Low

**Description**: `"agent:local"` used in two places in `on_initialized` — stale-cleanup predicate and session creation. Divergence would silently break stale cleanup.

**Fix Applied**: Added `const LOCAL_AGENT_OWNER: &str = "agent:local";` above `impl AgentRcServer` with a doc comment. Replaced both literals and updated the inline comment to reference the constant. `cargo clippy` clean.

* User Decision: ✅ **Fixed in branch**

---

#### RI-003: `on_initialized` tests do not invoke the actual method

* File: `tests/integration/on_initialized_tests.rs`
* Category: Test Quality
* Severity: Medium

**Description**: Tests verify constituent repo operations rather than `on_initialized` itself; `NotificationContext<RoleServer>` cannot be constructed without a live MCP transport.

**Fix Applied** (Option A): Added a `# Coverage note` doc section to the module explaining the limitation and pointing to the Option B refactor path for future reference. No production code change.

* User Decision: ✅ **Fixed — comment added**

---

#### RI-004: `load_authorized_users` visibility

* File: `src/config.rs`
* Category: API Design / Security
* Severity: Low

**Description**: `load_authorized_users` was `pub`, enabling unexpected external mutation. The external test crate legitimately calls it, which rules out `pub(crate)` (would require unsafe env var manipulation inside crate where `#![forbid(unsafe_code)]` applies).

**Fix Applied**: Added `#[doc(hidden)]` and an explicit `# Note` doc comment explaining it is `pub` solely for test-crate access and is an internal implementation detail of `load_credentials`. Simplified the two `ensure_authorized` external tests to set `authorized_user_ids` directly (removing unnecessary env var manipulation). Restored `missing_authorized_user_ids_env_var_fails` to the external test file. All 507 tests pass.

* User Decision: ✅ **Fixed — `#[doc(hidden)]` + doc clarification + simplified tests**

---

#### RI-005: DB path rename — no migration guidance

* File: `src/config.rs`
* Category: Operations / Compatibility
* Severity: Medium

* User Decision: ❌ **Rejected — solution is pre-release and 100% private; no existing deployments to migrate**

---

#### RI-006: FR-011 global allowlist removal — no ADR or spec update

* File: `src/policy/evaluator.rs`, `src/policy/loader.rs`
* Category: Architecture / Documentation
* Severity: Medium

**Fix Applied**:
- Created `docs/adrs/0012-policy-workspace-self-contained-auto-approve.md` documenting rationale, consequences, and the path to restore if threat model changes.
- Updated `specs/001-mcp-remote-agent-server/spec.md` FR-011 with strikethrough + superseded note linking to ADR-0012.
- Updated `specs/001-mcp-remote-agent-server/tasks.md` T061 and T116 to reflect current behavior.
- Fixed two stale passages in `docs/REFERENCE.md` (covered RI-009 simultaneously).

* User Decision: ✅ **Fixed — ADR-0012 created, spec/tasks/REFERENCE.md updated**

---

#### RI-007: Slack queue drain race during shutdown with empty global channel

* File: `src/main.rs`
* Category: Reliability
* Severity: Low

**Description**: When `config.slack.channel_id` is empty, the 500ms drain sleep inside `graceful_shutdown()` is skipped, so in-flight queued messages could be truncated before `queue_task.abort()` fires. Very low probability in practice.

* User Decision: ❌ **Deferred** — low-priority edge case logged in `.context/backlog.md` with full fix guidance

---

#### RI-008: `release.yml` workflow — no CI quality gate before release

* File: `.github/workflows/release.yml`
* Category: Security / CI
* Severity: Medium

**Fix Applied**: Added a `test` job (ubuntu-latest, `cargo clippy -- -D warnings` + `cargo test`) with `needs: test` on the `build` matrix job. A broken tag can no longer ship binaries.

* User Decision: ✅ **Fixed in branch**

---

#### RI-009: `docs/REFERENCE.md` stale global allowlist documentation

* File: `docs/REFERENCE.md`
* Category: Documentation
* Severity: Medium

* User Decision: ✅ **Fixed as part of RI-006** — both stale passages updated to reference ADR-0012 and current behavior

---

#### RI-010: `GlobalConfig.commands` retained but no longer used for policy — potential dead-config confusion

* File: `src/config.rs`, `config.toml`
* Lines: 175–177 (config.rs)
* Category: Maintainability / Documentation
* Severity: Low

**Description**: `GlobalConfig.commands: HashMap<String, String>` remains in the config struct and is still used exclusively by `slack/commands.rs` for Slack slash-command alias execution (FR-014). However, it is no longer used by the policy evaluator or loader for auto-approve decisions. The comment says "Registry of allowed commands" which could mislead users or future maintainers into thinking this still gates auto-approve policy. The doc comment should clarify its current sole purpose: Slack slash-command aliases only.

**Suggested Change**:
```rust
/// Registry of Slack slash-command aliases (FR-014).
///
/// Maps a short alias (`status`) to a shell command string
/// (`git status -s`). Only used by the Slack command handler
/// (`/run <alias>`); has no effect on MCP auto-approve policy.
#[serde(default)]
pub commands: HashMap<String, String>,
```

* User Decision: ✅ Fixed — doc comment updated in `src/config.rs` (line 175); clippy clean

---

### ✅ Approved for PR Comment

*(none yet)*

### ❌ Rejected / No Action

*(none yet)*

## Quality Gate Results

| Gate | Status |
|------|--------|
| `cargo check` | ✅ (implied by clean test compile) |
| `cargo clippy -- -D warnings` | ✅ Clean — `Finished dev profile [unoptimized] in 25.45s`, no warnings |
| `cargo fmt --all -- --check` | ✅ Clean — empty output (no violations) |
| `cargo test` | ✅ **507 passed; 0 failed** across unit (145), integration (344), contract (17), doc (1) |

## Next Steps

* [x] Await `cargo test` results — PASS
* [x] Await `cargo clippy` results — PASS
* [x] Run `cargo fmt --all -- --check` — PASS
* [x] Read `release.yml` to validate CI security
* [ ] Surface RI-001 through RI-010 to user for decisions
* [ ] Generate `handoff.md` once decisions are captured
