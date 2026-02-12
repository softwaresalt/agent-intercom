# Session Memory: 001-mcp-remote-agent-server Phase 12

**Date**: 2026-02-11
**Phase**: 12 — User Story 10: Operational Mode Switching
**Spec**: `specs/001-mcp-remote-agent-server/`

## Task Overview

Phase 12 implements runtime operational mode switching between remote (Slack-only), local (IPC-only), and hybrid (both) modes. It adds the `set_operational_mode` and `wait_for_instruction` MCP tools, a local IPC server for the `monocoque-ctl` CLI companion, and mode-aware message routing helpers.

**Tasks completed**: T124, T125, T084, T085, T086, T087, T088, T089, T090 (9/9)

## Current State

### Files Modified

- `src/mcp/tools/set_operational_mode.rs` — Full handler: validates mode, updates session in DB, notifies Slack, returns `{previous_mode, current_mode}`
- `src/mcp/tools/wait_for_instruction.rs` — Full handler: posts wait buttons to Slack, blocks on oneshot with configurable timeout, returns `{status, instruction?}`
- `src/mcp/handler.rs` — Added `WaitResponse` struct, `PendingWaits` type alias, `pending_waits` field on `AppState`, wired both tools into router
- `src/ipc/server.rs` — New IPC server: `interprocess::local_socket` listener, JSON-line protocol, dispatches `list`/`approve`/`reject`/`resume`/`mode` commands
- `src/ipc/mod.rs` — Re-exported `server` module alongside existing `socket`
- `ctl/main.rs` — Full `monocoque-ctl` CLI with clap subcommands: `List`, `Approve`, `Reject`, `Resume`, `Mode`
- `src/slack/client.rs` — Added `should_post_to_slack(mode)` and `should_post_to_ipc(mode)` routing helpers
- `src/slack/blocks.rs` — Added `wait_buttons(session_id)` function (Resume / Resume with Instructions / Stop Session)
- `src/slack/handlers/wait.rs` — New Slack interaction handler for wait-related button actions
- `src/slack/handlers/mod.rs` — Exported `wait` module
- `src/slack/events.rs` — Routed `wait_` action prefix to wait handler
- `src/persistence/session_repo.rs` — Added `update_mode(id, mode)` method
- `src/main.rs` — Added `PendingWaits` import and initialization in `AppState`
- `tests/contract/mode_tests.rs` — 24 contract tests for mode and wait schemas
- `tests/unit/mode_routing_tests.rs` — 7 unit tests for routing behavior
- `tests/contract.rs` — Registered `mode_tests` module
- `tests/unit.rs` — Registered `mode_routing_tests` module

### Pre-existing Clippy Fixes

Fixed 8 clippy pedantic warnings in prior phase test files:
- `forward_prompt_tests.rs` — doc_markdown for `prompt_type`
- `heartbeat_tests.rs` — doc_markdown for `status_message`, `progress_snapshot`
- `policy_evaluator_tests.rs` — `map_or` replaced with `is_some_and`
- `diff_apply_tests.rs` — doc_markdown for `new_file`
- `stall_detector_tests.rs` — used underscore-prefixed binding
- `schema_tests.rs` — doc_markdown for `SurrealDB`
- `session_lifecycle_tests.rs` — `map_or` replaced with comparison

### Test Results

- **Contract tests**: 119 passed (24 new for mode/wait)
- **Integration tests**: 38 passed (no new — existing flows sufficient)
- **Unit tests**: 99 passed (7 new for mode routing)
- **Total**: 256 tests, all passing
- **Clippy**: Clean under `-D warnings -D clippy::pedantic`
- **Format**: Clean under `cargo fmt --all -- --check`

## Important Discoveries

### Design Decisions

1. **IPC JSON-line protocol**: Chose connection-per-command model with newline-delimited JSON over `interprocess::local_socket`. Simple, cross-platform, and human-debuggable. See ADR-0009.

2. **Socket naming from workspace path**: The IPC socket name is derived from the canonical workspace root path, allowing multiple server instances on the same machine for different workspaces.

3. **`PendingWaits` follows `PendingApprovals` pattern**: Reused the same `Arc<Mutex<HashMap<String, oneshot::Sender<T>>>>` pattern for wait-for-instruction blocking, keeping consistency across all blocking tools.

4. **Mode-aware routing as free functions**: `should_post_to_slack()` and `should_post_to_ipc()` are standalone functions rather than methods on a trait, since routing is a simple boolean decision based on `SessionMode` enum.

5. **IPC handlers resolve oneshot channels directly**: The IPC server's `resume` and `approve`/`reject` commands look up and consume oneshot senders from the shared `PendingWaits`/`PendingApprovals` maps, providing immediate feedback to the blocked MCP tool handler.

### Failed Approaches

- **`interprocess` 2.x borrow semantics**: The `to_ns_name()` method takes ownership of the string (unlike the 1.x borrow pattern). Fixed by cloning the pipe name before the call.

### Framework Quirks

- **`interprocess::local_socket::ToNsName` trait**: Must be explicitly imported in `ctl/main.rs` for the `.to_ns_name()` method to be available on strings.

## Next Steps

- **Phase 13**: Slack channel history MCP resource (`slack://channel/{id}/recent`)
- **Phase 14**: Polish — authorization guards, double-submission prevention, Slack reconnection hardening
- **Integration testing**: End-to-end IPC flow testing would benefit from a dedicated integration test when the full server can be instantiated

## Context to Preserve

- ADR-0009 documents the IPC protocol design
- The `monocoque-ctl` binary target is defined in `Cargo.toml` under `[[bin]]` as `monocoque-ctl` with `path = "ctl/main.rs"`
- `WaitResponse` and `PendingWaits` live in `src/mcp/handler.rs` alongside the existing `ApprovalResponse`, `PromptResponse`, and their pending maps
- Mode routing helpers are at the bottom of `src/slack/client.rs`
