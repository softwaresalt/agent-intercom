# Phase 2 Memory — 003-agent-intercom-release

**Date**: 2026-02-23  
**Spec**: `003-agent-intercom-release`  
**Phase**: 2 — Foundational (Cargo + Core Source Rename)  
**Commit**: `b129453`  
**Branch**: `003-agent-intercom-release`

---

## What Was Done

The critical blocking rename phase. All Rust identifiers, constants, binary names,
config strings, and test fixtures were updated from "monocoque-agent-rc" to
"agent-intercom" / `agent_intercom`.

### Tasks Completed

| Task | Description | Result |
|---|---|---|
| T003 | Update `Cargo.toml` — package name, bin names, metadata | ✅ |
| T004 | Update `src/lib.rs` doc comment | ✅ |
| T005 | Update `src/main.rs` — CLI name, use paths, log messages | ✅ |
| T006 | Update `ctl/main.rs` — CLI name, IPC default, error messages | ✅ |
| T007 | `KEYCHAIN_SERVICE` constant → `"agent-intercom"` | ✅ |
| T008 | IPC name → `"agent-intercom"` in socket.rs + ctl/main.rs | ✅ |
| T009 | Env var prefix `MONOCOQUE_` → `INTERCOM_` in spawner.rs | ✅ |
| T010 | Policy dir `".agentrc"` → `".intercom"` | ✅ |
| T011 | Slash command `/monocoque` → `/intercom` | ✅ |
| T012 | Replace `use monocoque_agent_rc::` in `src/` | ✅ |
| T013 | Replace `use monocoque_agent_rc::` in `tests/` | ✅ |
| T014 | Replace `monocoque_agent_rc` in `ctl/main.rs` | ✅ |
| T015 | Replace "monocoque" in doc comments/log messages in `src/` | ✅ |
| T015a | Rename `AgentRcServer` → `IntercomServer` everywhere | ✅ |
| T016 | Replace "monocoque" string literals in `config.toml` | ✅ |
| T017 | `cargo check` — exit 0 (compilation gate) | ✅ |

### Files Modified (~77 files)

**Core source:** `Cargo.toml`, `src/lib.rs`, `src/main.rs`, `ctl/main.rs`,
`src/config.rs`, `src/orchestrator/spawner.rs`, `src/policy/loader.rs`,
`src/policy/watcher.rs`, `src/slack/commands.rs`, `config.toml`

**MCP layer:** `src/mcp/handler.rs`, `src/mcp/sse.rs`, `src/mcp/transport.rs`,
all 9 tool files in `src/mcp/tools/`

**Tests:** All files in `tests/unit/`, `tests/contract/`, `tests/integration/`
— `.agentrc` → `.intercom` in fixtures, `AgentRcServer` → `IntercomServer`,
assertion for keychain error message updated

### Quality Gate Results

| Gate | Command | Result |
|---|---|---|
| T017 | `cargo check` | ✅ exit 0 |
| Post | `cargo test` | ✅ 145 + 1 doc test, 0 failures |
| Post | `cargo clippy --all-targets -- -D warnings` | ✅ exit 0 |
| Post | `cargo fmt --all -- --check` | ✅ exit 0 |

---

## Key Decisions / Issues Encountered

- **`multi_replace_string_in_file` multi-match collision**: `name = "monocoque-agent-rc"`
  appeared in both `[package]` and `[[bin]]` in Cargo.toml. Fixed by providing
  more specific surrounding context in `oldString`.
- **Policy test fixture failure**: 6 tests referenced `.agentrc` directory paths in
  TOML fixture strings. Fixed by bulk PowerShell replace `.agentrc` → `.intercom`.
- **Credential assertion**: `missing_required_credential_error_names_both_sources`
  asserted error message contained "monocoque-agent-rc". Updated to "agent-intercom".
- **`cargo fmt` reorder**: `IntercomServer` sorts alphabetically differently than
  `AgentRcServer` in import groups — `cargo fmt --all` fixed automatically.
- **`AgentRcServer` across both transport files**: SSE and stdio transports both
  embedded the struct name; T015a caught all instances.

---

## Next Steps

Phase 3 (US1 Product Identity) adds assertion tests to lock in the renamed constants
and verifies zero "monocoque" references remain in source, tests, and config.
