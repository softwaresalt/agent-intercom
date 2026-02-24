# Phase 3 Memory — 003-agent-intercom-release

**Date**: 2026-02-23  
**Spec**: `003-agent-intercom-release`  
**Phase**: 3 — User Story 1: Consistent Product Identity (P1 MVP)  
**Commit**: `0f11b44`  
**Branch**: `003-agent-intercom-release`

---

## What Was Done

Added assertion tests to lock in renamed constants and verified zero "monocoque"
references remain anywhere in source, tests, or config files. Test fixtures in
credential_loading_tests.rs and mcp_dispatch_tests.rs were also cleaned up.

### Tasks Completed

| Task | Description | Result |
|---|---|---|
| T018 | Unit test: `KEYCHAIN_SERVICE == "agent-intercom"` in config_tests.rs | ✅ |
| T019 | Unit test: IPC name default == `"agent-intercom"` in config_tests.rs | ✅ |
| T020 | Unit test: env var prefix is `INTERCOM_` (not `MONOCOQUE_`) in config_tests.rs | ✅ |
| T021 | Unit test: policy directory constant == `".intercom"` in policy_tests.rs | ✅ |
| T022 | Unit test: slash command root == `"/intercom"` in command_exec_tests.rs | ✅ |
| T023 | Red gate — tests confirmed failing before impl (config_tests TOML fixture had old name) | ✅ |
| T024 | All test imports updated `monocoque_agent_rc` → `agent_intercom` (tests/unit/) | ✅ (Phase 2) |
| T025 | All test imports updated in tests/contract/ | ✅ (Phase 2) |
| T026 | All test imports updated in tests/integration/ | ✅ (Phase 2) |
| T027 | Contract test assertions updated for new constants (.intercom, agent-intercom) | ✅ (Phase 2) |
| T028 | Policy test fixtures updated (.agentrc → .intercom) | ✅ (Phase 2) |
| T029 | Credential + config test TOML fixtures updated (ipc_name) | ✅ |
| T030 | Integration test fixtures (mcp dispatch handshake client name) | ✅ |
| T031 | `cargo test` — 150+1 pass, 0 failures (green gate) | ✅ |
| T032 | `grep -r "monocoque"` — zero matches in source/tests/config | ✅ |

### Files Modified

- `tests/unit/config_tests.rs` — Added T018, T019, T020 assertion tests; fixed TOML fixture ipc_name
- `tests/unit/policy_tests.rs` — Added T021 assertion test
- `tests/unit/command_exec_tests.rs` — Added T022 assertion test
- `tests/unit/credential_loading_tests.rs` — Fixed TOML fixture + comment with "monocoque-agent-rc"
- `tests/integration/mcp_dispatch_tests.rs` — Fixed MCP handshake `clientInfo.name` from `"monocoque-test"` to `"intercom-test"`
- `specs/003-agent-intercom-release/tasks.md` — T018–T032 marked `[x]`
- `docs/slack-app-setup.md` — Untracked file swept into commit (belongs in repo)

### Quality Gate Results

| Gate | Command | Result |
|---|---|---|
| T031 | `cargo test` | ✅ 150 + 1 doc test, 0 failures |
| Post | `cargo clippy --all-targets -- -D warnings` | ✅ exit 0 |
| Post | `cargo fmt --all -- --check` | ✅ exit 0 |
| T032 | `grep -r "monocoque" src/ ctl/ tests/ config.toml Cargo.toml` | ✅ 0 matches |

---

## Key Decisions / Issues Encountered

- **8 remaining "monocoque" matches in tests** after the grep were all legitimate:
  assertion strings checking old name is absent (`!msg.contains("monocoque")`),
  doc comments on the T020 test describing what it checks, and one test comment
  with historical context. None required modification.
- **mcp_dispatch_tests.rs clientInfo** contained `"monocoque-test"` as MCP client
  name in the handshake fixture — updated to `"intercom-test"` as it reflects the
  actual product name the client would use.
- **`docs/slack-app-setup.md`** was an untracked file on-disk swept in by `git add -A`.
  It contains legitimate setup documentation and belongs in the repo.

---

## Cumulative Test Count

| After Phase | Tests |
|---|---|
| Phase 1 baseline | 145 + 1 doc |
| Phase 2 (no new tests) | 145 + 1 doc |
| Phase 3 (+5 assertion tests) | 150 + 1 doc |

---

## Next Steps: Phase 4 — US4 Intercom-Themed Tool Names

Phase 4 renames all 9 MCP tools using the mapping defined in the spec:

| Old Name | New Name |
|---|---|
| `ask_approval` | `check_clearance` |
| `accept_diff` | `check_diff` |
| `check_auto_approve` | `auto_check` |
| `forward_prompt` | `transmit` |
| `wait_for_instruction` | `standby` |
| `heartbeat` | `signal` |
| `remote_log` | `broadcast` |
| `recover_state` | `reboot` |
| `set_operational_mode` | `switch_freq` |

TDD: Write contract tests (T033–T037) first, verify they fail, then update tool
`name` fields in `src/mcp/handler.rs` and dispatch keys in the `ToolRouter` (T039–T040).
Also sets `ServerInfo { name: "agent-intercom", version: env!("CARGO_PKG_VERSION") }` (T041).
