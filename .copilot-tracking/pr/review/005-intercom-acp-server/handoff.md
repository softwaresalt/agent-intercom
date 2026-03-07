<!-- markdownlint-disable-file -->
# PR Review Handoff: 005-intercom-acp-server

## PR Overview

Adds full Agent Client Protocol (ACP) support to the agent-intercom MCP server. Introduces a protocol-agnostic `AgentDriver` trait, ACP NDJSON stream processing, subprocess spawning/lifecycle management, workspace-to-channel routing with hot-reload, multi-session Slack threading, mode-aware slash commands (`/acom` for MCP, `/arc` for ACP), offline message queuing, and ACP-specific stall detection. All 4 quality gates pass.

* Branch: `005-intercom-acp-server`
* Base Branch: `main`
* Total Files Changed: 133
* Total Review Comments: 8

## PR Comments Ready for Submission

### File: `src/slack/commands.rs`

#### Comment 1 (Lines 559 through 561)

* Category: Reliability / Observability
* Severity: Medium

Database error silently discarded via `.ok()` when marking an ACP session as `Interrupted` after a startup failure. If this DB write fails, the session lingers in a stale state with no diagnostic trace.

**Suggested Change**

```rust
if let Err(e) = repo.set_terminated(&bg_session_id, SessionStatus::Interrupted).await {
    warn!(%e, session_id = %bg_session_id, "failed to mark session interrupted after startup failure");
}
```

#### Comment 2 (Lines 631 through 633)

* Category: Reliability / Observability
* Severity: Medium

Same `.ok()` pattern — discards the DB error when marking a session interrupted after a handshake failure. Apply the same `if let Err(e)` + `warn!()` pattern as Comment 1.

**Suggested Change**

```rust
if let Err(e) = repo.set_terminated(session_id, SessionStatus::Interrupted).await {
    warn!(%e, session_id = %session_id, "failed to mark session interrupted after handshake failure");
}
```

---

### File: `src/config_watcher.rs`

#### Comment 3 (Lines 220 through 223)

* Category: Reliability / Observability
* Severity: Medium

`RwLock` poison recovery is silent. Lock poisoning means a thread panicked while holding the lock — recovering without logging masks potentially serious concurrency issues.

**Suggested Change**

```rust
let guard = self.mappings.read().unwrap_or_else(|e| {
    warn!("workspace_mappings lock was poisoned, recovering with inner value");
    e.into_inner()
});
```

---

### File: `run-debug-acp.ps1`

#### Comment 4 (Lines 1 through 89)

* Category: Repository Hygiene
* Severity: Low

Development debug script referencing `.\target\debug\` paths and environment-specific log levels. Consider adding to `.gitignore` or relocating to `scripts/dev/` with a note in the README. No hardcoded secrets present (only env var names), so low risk.

---

### File: `findings.json`

#### Comment 5 (Lines 1 through 208)

* Category: Repository Hygiene
* Severity: Low

Audit artifact with 25 resolved security/HITL findings committed at repository root. Suggest moving to `specs/005-intercom-acp-server/findings.json` to co-locate with the feature spec, or adding to `.gitignore` if it's purely ephemeral.

---

### File: `docs/migration-guide.md`

#### Comment 6 (Entire file)

* Category: Documentation
* Severity: High

The migration guide has no section for adopting ACP mode. Existing users upgrading need guidance on:

1. **Mode selection** — when to use `--mode acp` vs default MCP
2. **Config changes** — `[acp]` section (`max_sessions`, `startup_timeout_seconds`, `host_cli`, `host_cli_args`), `[[workspace]].path` field
3. **Credential separation** — `_ACP`-suffixed env vars and separate keychain entries (per ADR-0015)
4. **Slack app setup** — second Slack app with `/arc` slash commands
5. **Behavioral differences** — session lifecycle (`/arc session-start`/`session-stop`), thread-per-session model, push event routing

**Suggested Change**

Add an "Adopting ACP Mode" section after the existing "channel_id to workspace_id Migration" section, following the same step-by-step format used throughout the guide.

---

### File: `docs/configuration.md`

#### Comment 7 (Lines 178 through 201)

* Category: Documentation
* Severity: Medium

ACP section exists but is missing documentation for the `[[workspace]].path` field, which determines the ACP agent's working directory. The `config.toml.example` (lines 135, 167) documents this well, but `configuration.md` doesn't mention it. Also missing: how `/arc session-start <workspace>` resolves the target workspace and selects `cwd`.

**Suggested Change**

Add a subsection under ACP Configuration documenting the `path` field and workspace resolution logic for ACP sessions.

---

### File: `src/acp/handshake.rs`

#### Comment 8 (Lines 1 through 438)

* Category: Test Coverage
* Severity: Medium

The handshake FSM (initialize → initialized → session/new → session/prompt) is exercised via integration tests but has no dedicated unit tests. Each public function (`send_initialize`, `wait_for_initialize_result`, `send_session_new`, `send_initialized_notification`) should have focused tests for request construction, response parsing, timeout handling, and error recovery. Can be a follow-up item given the PR's size.

---

### File: `src/driver/acp_driver.rs`

#### Comment 8b (Lines 1 through 461)

* Category: Test Coverage
* Severity: Medium

Contract tests exist for `McpDriver` (`tests/contract/driver_contract_tests.rs`) but no equivalent for `AcpDriver`. Missing: clearance registration → resolution, prompt forwarding via session writer, `deregister_session()` cleanup, concurrent request handling. Can be a follow-up item alongside RI-07.

---

## Review Summary by Category

* Security Issues: 0
* Code Quality / Reliability: 3 (RI-01, RI-02 — observability; RI-03, RI-04 — hygiene)
* Convention Violations: 0
* Documentation: 2 (RI-05, RI-06)
* Test Coverage: 2 (RI-07, RI-08)

## Instruction Compliance

* ✅ `constitution.instructions.md` (Principles I, II, III, IV, VI, VII, VIII): All rules followed — zero unsafe code, proper error handling, idempotent DDL, security boundaries enforced, single workspace binary
* ⚠️ `constitution.instructions.md` (Principle V — Structured Observability): Two silent `.ok()` calls suppress DB errors without tracing (RI-01)
* ⚠️ `constitution.instructions.md` (Principle III — Test-First Development): ACP handshake and driver routing lack dedicated unit/contract tests (RI-07, RI-08)

## Follow-Up Recommendations

These are non-blocking strategic items for subsequent iterations:

1. **Create `tests/unit/acp_handshake_tests.rs`** — unit tests for each handshake function (RI-07)
2. **Create `tests/contract/acp_driver_contract_tests.rs`** — contract tests mirroring MCP driver suite (RI-08)
3. **Consolidate `config_tests.rs` and `credential_loading_tests.rs`** — appear partially redundant
4. **Add spawner signal-handling tests** — `SIGTERM` vs `SIGKILL` paths, process group cleanup edge cases
5. **Add timeout tests** — startup handshake timeout scenarios are untested
