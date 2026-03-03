# Session Memory: 005-intercom-acp-server Phase 14

**Date**: 2026-03-03
**Branch**: 005-intercom-acp-server
**Status**: COMPLETE

## Task Overview

Phase 14: Security Hardening — implementing ES-004 (process tree termination),
ES-010 (host_cli path validation), and ES-008 (outbound message sequence numbers).

## Tasks Completed

- [x] T122: Integration test — Windows process tree kill (`kill_process_tree` terminates child)
- [x] T123: Integration test — Unix process group kill (`kill_process_group` terminates child)
- [x] T124: Windows `CREATE_NEW_PROCESS_GROUP` + `kill_process_tree` in `spawner.rs`
- [x] T125: Unix `process_group(0)` + `kill_process_group` in `spawner.rs`
- [x] T126: `check_for_orphan_processes` cross-platform orphan detection in `spawner.rs`
- [x] T127: Unit tests for `validate_host_cli_path` (3 tests in `config_tests.rs`)
- [x] T128: `validate_host_cli_path` + helpers (`is_host_cli_on_path`, `is_standard_host_cli_location`) in `config.rs`
- [x] T129: Wire `validate_host_cli_path().ok()` + `check_for_orphan_processes` into `main.rs` ACP startup block
- [x] T130: Unit test — monotonic sequence numbers (`outbound_messages_have_monotonic_sequence_numbers`)
- [x] T131: Unit test — write failure logging (`write_failure_returns_acp_error`)
- [x] T132: Per-session `AtomicU64` counter in `AcpDriver` (`register_session` returns `Arc<AtomicU64>`)
- [x] T133: Sequence number injection in `run_writer` (stamps `seq` field on each outbound JSON object)
- [x] T134: Write failure → mark session `Interrupted` in DB via `SessionRepo`

## Files Modified

- `src/acp/spawner.rs` — Added `process_group(0)` (Unix) + `CREATE_NEW_PROCESS_GROUP` (Windows) to `spawn_agent`;
  added `kill_process_tree` (Windows), `kill_process_group` (Unix), `check_for_orphan_processes` (cross-platform)
- `src/acp/writer.rs` — Completely rewritten: generic `W: AsyncWrite + Unpin + Send`, added `counter: Arc<AtomicU64>`,
  `db: Arc<SqlitePool>` params; seq stamping; marks session `Interrupted` on write failure
- `src/config.rs` — Added `validate_host_cli_path` method on `GlobalConfig`; `is_host_cli_on_path` and
  `is_standard_host_cli_location` private helpers
- `src/driver/acp_driver.rs` — Added `seq_counters: SeqCounterMap`; `register_session` now returns `Arc<AtomicU64>`;
  cleanup in `deregister_session`
- `src/main.rs` — ACP startup block calls `validate_host_cli_path().ok()` and
  `agent_intercom::acp::spawner::check_for_orphan_processes`
- `src/slack/commands.rs` — Updated `register_session` call to capture `seq_counter`; updated `run_writer` call
  to pass `seq_counter` + `Arc::clone(&state.db)`; added process tree kill before `terminate_session` in
  `handle_session_stop`
- `tests/integration/acp_lifecycle_tests.rs` — Updated `queued_messages_delivered_on_reconnect` to capture
  `let _seq_counter`; added T122 (Windows) and T123 (Unix) process kill tests
- `tests/unit/acp_codec_tests.rs` — Added T130 and T131
- `tests/unit/command_tests.rs` — Fixed `doc_markdown` clippy violations (backtick `session_id`)
- `tests/integration/acp_mcp_bridge_tests.rs` — Fixed `doc_markdown` clippy violations

## Test Results

**831 tests pass, 0 failed**:
- 33 contract tests
- 211 unit tests (first batch)
- 251 integration tests
- 331 unit tests (second batch)
- 5 doc tests
- 2 ignored (doc test stubs for codec/driver)

## Important Discoveries

### `crate::` vs `agent_intercom::` in main.rs
`main.rs` is a separate binary crate; it must reference the library as `agent_intercom::acp::spawner::...`
not `crate::acp::spawner::...`. The `crate::` shortcut only works inside the library crate (`src/lib.rs` tree).

### Clippy pedantic: `map_unwrap_or`
`option.map(f).unwrap_or_else(g)` must be written as `option.map_or_else(g, f)`.
Caught in `spawner.rs` (host_cli name extraction) and `config.rs` (`is_host_cli_on_path`).

### Clippy: `case_sensitive_file_extension_comparisons`
Use `Path::extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))` instead of `str::ends_with(".exe")`.

### `run_writer` API change summary
Old: `run_writer(session_id: String, stdin: ChildStdin, msg_rx, cancel)`
New: `run_writer<W: AsyncWrite + Unpin + Send>(session_id, stdin: W, msg_rx, cancel, counter: Arc<AtomicU64>, db: Arc<SqlitePool>)`

### `register_session` return type change
Old: `async fn register_session(&self, session_id: &str, sender: Sender<Value>) -> ()`
New: `async fn register_session(&self, session_id: &str, sender: Sender<Value>) -> Arc<AtomicU64>`
All callers must capture the return value (use `let _seq_counter = ...` if unused in tests).

## Next Steps (Phase 15: Reliability & Observability)

- T135–T138: HITL-001 — Socket Mode WebSocket drop notifications via HTTP REST
- T139–T142: HITL-007 — ACP audit logging (session lifecycle events)
- T143–T146: ES-005 — Token-bucket rate limiting in ACP reader
- T147–T149: ES-006 — Stall timer initialized from persisted `last_activity_at`
- T150–T152: ES-007 — Session DB commit before ACP reader task starts
- T153–T154: ES-009 — Workspace config read lock during session creation

## Context to Preserve

- Branch: `005-intercom-acp-server`
- Spec: `specs/005-intercom-acp-server/`
- All quality gates (check, test, clippy, fmt) pass with 0 failures
- Windows process tree kill calls `taskkill /F /T /PID <pid>` (safe, no unsafe code)
- Unix process group kill calls `kill -TERM -<pid>` and `kill -KILL -<pid>` via `tokio::process::Command`
- `check_for_orphan_processes` on Windows appends `.exe` and uses `tasklist /FI "IMAGENAME eq ..."` filter
