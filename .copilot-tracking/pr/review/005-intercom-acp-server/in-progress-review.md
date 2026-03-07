<!-- markdownlint-disable-file -->
# PR Review Status: 005-intercom-acp-server

## Review Status

* Phase: 4 — Finalize Handoff
* Last Updated: 2026-03-06T08:13Z
* Summary: Feature branch adds Agent Client Protocol (ACP) support — 133 files changed, ~18,238 lines added, ~1,350 removed. All quality gates pass (385 tests, clean clippy/fmt/check).

## Branch and Metadata

* Normalized Branch: `005-intercom-acp-server`
* Source Branch: `005-intercom-acp-server`
* Base Branch: `main`
* Linked Work Items: spec at `specs/005-intercom-acp-server/spec.md`
* Commits: 48 commits (d8fb779..c230f47)

## Quality Gates

| Gate | Status | Notes |
|------|--------|-------|
| `cargo check` | ✅ Pass | Clean compilation |
| `cargo clippy -- -D warnings` | ✅ Pass | Zero warnings |
| `cargo fmt --all -- --check` | ✅ Pass | Formatted correctly |
| `cargo test` | ✅ Pass | 385 passed, 0 failed, 6 doc-tests passed, 2 ignored |

## Instruction Files Reviewed

* `.github/instructions/constitution.instructions.md`: Applies to all files (`applyTo: "**"`). Constitution v2.2.0 — safety-first Rust, test-first development, security boundary enforcement, structured observability, single-binary simplicity.

## New Modules Added

| Module | Files | Purpose |
|--------|-------|---------|
| `src/acp/` | mod.rs, codec.rs, handshake.rs, reader.rs, spawner.rs, writer.rs | ACP protocol: NDJSON framing, JSON-RPC handshake, stream I/O, process management |
| `src/driver/` | mod.rs, acp_driver.rs, mcp_driver.rs | Protocol-agnostic `AgentDriver` trait with ACP and MCP implementations |
| `src/config_watcher.rs` | single file | Hot-reload workspace mappings from config.toml via `notify` |
| `src/mode.rs` | single file | `ServerMode` enum (Mcp, Acp) |
| `src/slack/push_events.rs` | single file | Route Slack push events (mentions, thread messages) as steering |
| `src/models/session.rs` | expanded | `ProtocolMode`, session title truncation, status model |

## Diff Mapping (Key Files)

| File | Type | Lines | Category |
|------|------|-------|----------|
| src/acp/codec.rs | New | 1-114 | Core Protocol |
| src/acp/handshake.rs | New | 1-438 | Core Protocol |
| src/acp/reader.rs | New | 1-618 | Core Protocol |
| src/acp/spawner.rs | New | 1-368 | Core Protocol |
| src/acp/writer.rs | New | 1-128 | Core Protocol |
| src/driver/mod.rs | New | 1-162 | Architecture |
| src/driver/acp_driver.rs | New | 1-461 | Architecture |
| src/driver/mcp_driver.rs | New | 1-230 | Architecture |
| src/config.rs | Modified | Heavy | Configuration |
| src/config_watcher.rs | New | 1-233 | Configuration |
| src/main.rs | Modified | Heavy | Bootstrap |
| src/mcp/handler.rs | Modified | Moderate | Infrastructure |
| src/slack/commands.rs | Modified | Heavy | Slack Integration |
| src/slack/push_events.rs | New | 1-166 | Slack Integration |
| src/persistence/schema.rs | Modified | +110 | Database |
| src/persistence/session_repo.rs | Modified | +302 | Database |

## Strengths

1. ✅ **Zero unsafe code** across all new modules
2. ✅ **No `unwrap()`/`expect()` in production code** — all fallible ops use `Result`/`AppError`
3. ✅ **Comprehensive doc comments** on all public items
4. ✅ **No `todo!()`/`unimplemented!()`/`unreachable!()` macros** — code is complete
5. ✅ **Clean `AgentDriver` trait abstraction** — protocol-agnostic with MCP/ACP impls
6. ✅ **Rate limiting** (token-bucket FR-044) with termination threshold in ACP reader
7. ✅ **Environment isolation** via `ALLOWED_ENV_VARS` allowlist (FR-029, S075)
8. ✅ **Idempotent schema DDL** — all `CREATE TABLE IF NOT EXISTS` and `add_column_if_missing()`
9. ✅ **Platform-specific process handling** — Windows/Unix process group cleanup
10. ✅ **385 tests passing** across unit/contract/integration tiers

---

## Review Items

### 🔍 In Review

(All items resolved)

### ✅ Approved for PR Comment

#### RI-01: Silent error suppression via `.ok()` on database operations — ✅ Approved
* File: `src/slack/commands.rs` | Lines: 559-561, 631-633 | Severity: Medium
* Decision: Replace `.ok()` with `if let Err(e)` + `warn!()` logging

#### RI-02: Silent RwLock poison recovery without logging — ✅ Approved
* File: `src/config_watcher.rs` | Lines: 220-223 | Severity: Medium
* Decision: Add `warn!()` before `into_inner()` recovery

#### RI-03: `run-debug-acp.ps1` committed to repository — ✅ Approved
* File: `run-debug-acp.ps1` | Lines: 1-89 | Severity: Low
* Decision: Add to `.gitignore` or relocate to `scripts/dev/`

#### RI-04: `findings.json` committed to repository root — ✅ Approved
* File: `findings.json` | Lines: 1-208 | Severity: Low
* Decision: Move to `specs/005-intercom-acp-server/findings.json` or gitignore

#### RI-05: Missing ACP migration documentation — ✅ Approved
* File: `docs/migration-guide.md` | Lines: entire file | Severity: High
* Decision: Add "Adopting ACP Mode" section covering mode selection, config, credentials, slash commands

#### RI-06: `docs/configuration.md` missing workspace `path` field for ACP — ✅ Approved
* File: `docs/configuration.md` | Lines: ~178-201 | Severity: Medium
* Decision: Add workspace `path` field docs and ACP workspace routing explanation

#### RI-07: Test coverage gaps for ACP handshake protocol — ✅ Approved
* File: `src/acp/handshake.rs` | Lines: 1-438 | Severity: Medium
* Decision: Create `tests/unit/acp_handshake_tests.rs` (may be follow-up)

#### RI-08: Test coverage gaps for ACP driver routing — ✅ Approved
* File: `src/driver/acp_driver.rs` | Lines: 1-461 | Severity: Medium
* Decision: Create `tests/contract/acp_driver_contract_tests.rs` (may be follow-up)

### ❌ Rejected / No Action

(No items rejected)

## Next Steps

* [x] Present RI-01 through RI-08 to user for decisions
* [x] All 8 items approved by user
* [ ] Generate handoff.md with finalized PR comments
