# Phase 9 Session Memory — 005-intercom-acp-server

**Date**: 2025-07-16  
**Phase**: 9 — ACP Stream Processing  
**Status**: COMPLETE  
**Commit**: (see git log)

---

## Task Overview

Phase 9 implements bidirectional NDJSON stream communication between the
`agent-intercom` server and ACP agent child processes. The phase covers:

- `AcpCodec` — `LinesCodec` wrapper with 1 MiB line limit
- `run_reader` task — drives `FramedRead<ChildStdout, AcpCodec>`, parses inbound
  NDJSON lines, emits `AgentEvent` values over an mpsc channel
- `run_writer` task — receives `serde_json::Value` over mpsc, serializes to NDJSON,
  writes to `ChildStdin`
- `AcpDriver` — implements `AgentDriver` trait; holds per-session `msg_tx` senders
  in a `WriterMap`; routes operator responses to the correct session stream
- T084 wiring — `AppState` gains `acp_event_tx: Option<mpsc::Sender<AgentEvent>>`
  and `acp_driver: Option<Arc<AcpDriver>>`; in ACP mode `main.rs` spawns a minimal
  `run_acp_event_consumer` task; `handle_acp_session_start` in `commands.rs` spawns
  `run_reader` and `run_writer` per session and registers the session with `AcpDriver`

---

## Current State

### Tasks Completed
All 16 tasks T069–T084 are complete and marked `[x]` in `tasks.md`.

### Files Created (new)
- `src/acp/codec.rs` — `AcpCodec`, `MAX_LINE_BYTES = 1_048_576`
- `src/acp/reader.rs` — `run_reader`, `parse_inbound_line`
- `src/acp/writer.rs` — `run_writer`
- `src/driver/acp_driver.rs` — `AcpDriver`, `WriterMap`
- `tests/unit/acp_codec_tests.rs` — 10 unit tests (T069–T078)

### Files Modified
- `src/acp/mod.rs` — declared `codec`, `reader`, `writer`, `spawner` modules
- `src/driver/mod.rs` — declared `acp_driver` module; `AgentEvent` enum; `AgentDriver` trait
- `src/errors.rs` — `AppError::Io(String)` variant added for stream errors
- `src/mcp/handler.rs` — `AppState` gained `acp_event_tx` and `acp_driver` fields
- `src/main.rs` — mode-branched driver construction; `run_acp_event_consumer`; ACP startup branch
- `src/slack/commands.rs` — T084: `handle_acp_session_start` now spawns reader/writer tasks
- `tests/unit.rs` — declared `acp_codec_tests` module
- `tests/integration/test_helpers.rs` — both helpers updated with `acp_event_tx: None, acp_driver: None`
- `tests/integration/channel_override_tests.rs` — 2 direct construction sites updated
- `tests/integration/disconnect_tests.rs` — 3 direct construction sites updated
- `tests/integration/handler_edge_case_tests.rs` — 2 direct construction sites updated
- `tests/integration/health_endpoint_tests.rs` — 1 direct construction site updated
- `tests/integration/ipc_server_tests.rs` — 1 direct construction site updated
- `tests/integration/mcp_dispatch_tests.rs` — 1 direct construction site updated
- `tests/integration/on_initialized_tests.rs` — 2 direct construction sites updated
- `tests/integration/shutdown_tests.rs` — 2 direct construction sites updated
- `specs/005-intercom-acp-server/tasks.md` — T069–T084 marked complete

### Test Results
- `cargo test`: 313 passed, 0 failed
- `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`: clean
- `cargo fmt --all -- --check`: clean

---

## Important Discoveries

### AppState field additions always require 10+ construction sites
The `AppState` struct had construction sites spread across `src/main.rs`,
`tests/integration/test_helpers.rs`, and 5 test files (15 total sites found, 
with more discovered during `cargo test`). Always run `cargo test` (not just 
`cargo check`) after adding struct fields to find all failing construction sites.

### AcpDriver coercion to Arc<dyn AgentDriver>
`Arc::clone(&acp_driver)` where `acp_driver: Arc<AcpDriver>` does NOT automatically
coerce to `Arc<dyn AgentDriver>`. Must cast explicitly:
```rust
Arc::clone(&acp) as Arc<dyn AgentDriver>
```

### run_acp_event_consumer already existed
The more complete `run_acp_event_consumer` (with structured logging per event variant)
was already written in a prior interrupted session. Adding a second simpler version
caused a duplicate symbol error. Always check the end of `main.rs` before adding new
functions.

### CancellationToken for per-session reader/writer
`handle_acp_session_start` creates a `session_ct = CancellationToken::new()` locally
and passes child tokens to reader and writer. The session CT is not stored in AppState
in Phase 9 — a future phase (Phase 12 or teardown) will need to store it alongside
the child handle to cleanly stop the I/O tasks when a session is terminated.

### clippy::too_many_lines
`handle_acp_session_start` exceeded 100 lines after T084 wiring. The `#[allow(clippy::too_many_lines)]`
was already present on the function; the clippy error resolved after cache invalidation.

---

## Next Steps (Phase 10)

Phase 10: Offline Agent Message Queuing (P3)
- Queue operator messages for offline/disconnected agents
- Deliver queued messages on reconnect (session resumes)
- T085–T096 in tasks.md

Known open issues from Phase 9:
- Session `CancellationToken` not stored — `session-stop` Slack command cannot
  gracefully shut down reader/writer tasks yet. Phase 12 should add a
  `session_cancel_tokens: Arc<Mutex<HashMap<String, CancellationToken>>>` to AppState.
- `run_acp_event_consumer` only logs events. Full Slack dispatch (clearance routing,
  prompt forwarding) deferred to Phase 11.

---

## Context to Preserve

- `AcpDriver.register_session(&session_id, msg_tx)` is `async` — requires `.await`
- `AcpConnection` fields: `.child: Child`, `.stdin: ChildStdin`, `.stdout: BufReader<ChildStdout>`
- `run_reader<R: AsyncRead + Unpin + Send>` — generic over reader type
- `run_writer` — takes `ChildStdin` directly (not generic)
- `AgentEvent` variants: `ClearanceRequested`, `StatusUpdated`, `PromptForwarded`,
  `HeartbeatReceived`, `SessionTerminated`
- Branch: `005-intercom-acp-server`
- Slack thread_ts for Phase 9 broadcasts: `1772331278.012399`
