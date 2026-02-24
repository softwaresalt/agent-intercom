# Phase 4 Memory — 003-agent-intercom-release

**Date**: 2026-02-23  
**Spec**: `003-agent-intercom-release`  
**Phase**: 4 — US4 Intercom-Themed Tool Names  
**Tasks**: T033–T046 (14 tasks)  
**Status**: ✅ Complete

---

## What Was Built

Phase 4 renamed all 9 MCP tools from their original `monocoque-agent-rc` names to intercom-themed names and updated `ServerInfo` to report `agent-intercom` identity.

### Tool Rename Mapping (FINAL — uses `ping` not `signal`)

| Old Name | New Name |
|---|---|
| `ask_approval` | `check_clearance` |
| `accept_diff` | `check_diff` |
| `check_auto_approve` | `auto_check` |
| `forward_prompt` | `transmit` |
| `wait_for_instruction` | `standby` |
| `heartbeat` | `ping` |
| `remote_log` | `broadcast` |
| `recover_state` | `reboot` |
| `set_operational_mode` | `switch_freq` |

**Note**: The Rust MODULE/FILE names (e.g., `ask_approval.rs`) were NOT changed — only the MCP-visible `Tool::name` field values changed.

---

## Key Implementation Details

### `src/mcp/handler.rs`
- All 9 `Tool::name` fields updated to new intercom names
- All 9 `ToolRouter` dispatch keys updated to match
- `ServerInfo` updated: `server_info: Implementation { name: env!("CARGO_PKG_NAME").into(), version: env!("CARGO_PKG_VERSION").into() }`
- `Implementation` added to `rmcp::model` imports
- `all_tools()` changed to `pub(crate)` for test access

**Critical rmcp 0.5 finding**: `ServerInfo` is aliased to `InitializeResult` with fields `protocol_version`, `server_info: Implementation`, and `instructions`. The `Implementation` struct has `name` and `version` fields. There are NO top-level `name:` or `version:` fields on `ServerInfo` — they must be nested inside `Implementation`.

### New Test File: `tests/contract/tool_names_tests.rs`
- 20 pure JSON structure tests (T036) verifying all 9 renamed tools preserve their input schemas
- Tests `check_clearance`, `check_diff`, `auto_check`, `transmit`, `standby`, `ping`, `broadcast`, `reboot`, `switch_freq`
- Registered in `tests/contract.rs` as `mod tool_names_tests;`

### Updated Integration Tests (`tests/integration/mcp_dispatch_tests.rs`)
- **T033**: `transport_list_tools_uses_new_intercom_names` — verifies all 9 new names in `tools/list`
- **T034**: `transport_server_info_reports_agent_intercom` — verifies `serverInfo.name == "agent-intercom"`
- **T035**: `transport_old_tool_name_returns_error` — verifies `ask_approval` returns error
- **T037**: `transport_empty_tool_name_returns_error` — verifies empty string returns error
- Existing tests updated: `heartbeat`→`ping`, `recover_state`→`reboot`, `set_operational_mode`→`switch_freq`

### Contract Test Updates (T042a)
Updated `TOOL_NAME` constants in 5 existing contract test files:
- `accept_diff_tests.rs` → `"check_diff"`
- `ask_approval_tests.rs` → `"check_clearance"`
- `check_auto_approve_tests.rs` → `"auto_check"`
- `forward_prompt_tests.rs` → `"transmit"`
- `remote_log_tests.rs` → `"broadcast"`

### Contract JSON File: `specs/001-mcp-remote-agent-server/contracts/mcp-tools.json`
All 9 tool keys renamed from old to new names. Product name in description updated from `monocoque-agent-rc` to `agent-intercom`.

### Documentation Updates (T043, T044)
- `.github/copilot-instructions.md`: Updated `AgentRcServer`→`IntercomServer`, blocking tool names, and full Remote Approval Workflow section (all 3 step headers + table entries + Rules)
- `.github/agents/rust-engineer.agent.md`: Updated 9-row MCP tool table, stall detector `ping` reference, blocking tool pattern header

---

## Test Results

**Before Phase 4**: ~151 tests (150 + 1 doc test)  
**After Phase 4**: 545 tests total (0 failures)

Breakdown:
- 17 unit (test group 1)
- 157 contract tests (was 153)
- 210 integration tests (was 208)
- 150 unit tests
- 1 doc test

---

## Constitution Gates Passed

1. ✅ `cargo check` — clean
2. ✅ `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` — clean (fixed 15 doc-backtick violations in new test files)
3. ✅ `cargo fmt --all -- --check` — clean (auto-formatted 3 style violations)
4. ✅ `cargo test` — 545 tests, 0 failures

### Clippy Fix Notes
The new test files initially had doc comment violations for identifiers without backticks (clippy `missing_backticks_in_doc`). All fixed by wrapping bare identifier names in doc comments with backticks.

---

## Next Phase

**Phase 5** — US2 Reliable Slack Notifications (Priority P2)
- Fix reconnect/repost logic for pending messages after Socket Mode disconnects
- Fix duplicate-post guard on reconnect
- Tasks: T047–T060 (check tasks.md for current state)

---

## Branch Info

**Branch**: `003-agent-intercom-release`  
**Phase 4 commit**: pending (to be recorded after git push)  
**Previous commits**:
- Phase 3: `0f11b44` (implementation) + `59e6a20` (memory + checkpoint)
- Phase 2: `b129453`
- Spec heartbeat→ping rename: `25d3034`
