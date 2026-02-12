# Session Memory: 001-mcp-remote-agent-server Phase 5

**Date**: 2026-02-11
**Phase**: 5 — User Story 4 (Agent Stall Detection and Remote Nudge)
**Spec**: specs/001-mcp-remote-agent-server/

## Task Overview

Phase 5 implements User Story 4 (P1): the server detects when an agent goes silent (no MCP tool calls for a configurable threshold), alerts the operator via Slack, and nudges the agent to resume. This covers stall detection, auto-nudge escalation, self-recovery detection, the `heartbeat` tool, and nudge interaction callbacks.

Eleven tasks completed:

| Task | Description | Status |
|------|-------------|--------|
| T110 | Unit tests for stall detection (6 tests) | Done |
| T111 | Contract tests for `heartbeat` tool (6 tests) | Done |
| T112 | Integration test for nudge flow (3 tests) | Done |
| T047 | Per-session stall detection timer | Done |
| T048 | Stall alert posting events | Done |
| T049 | `heartbeat` MCP tool handler | Done |
| T050 | Nudge interaction callback | Done |
| T051 | Auto-nudge escalation | Done |
| T052 | Self-recovery detection | Done |
| T053 | Wire stall timer reset into MCP handler | Done |
| T054 | Tracing spans on stall detection | Done |

## Current State

### Test Results

- Contract tests: 40 pass (34 Phase 3-4 + 6 Phase 5)
- Integration tests: 15 pass (12 Phase 3-4 + 3 Phase 5)
- Unit tests: 53 pass (47 Phase 3-4 + 6 Phase 5)
- Total: 108/108 pass

### Toolchain Gates

- `cargo check` — pass
- `cargo clippy -- -D warnings -D clippy::pedantic` — pass (no suppressions except `too_many_arguments` on internal function)
- `cargo test` — 108/108 pass
- `cargo fmt --all -- --check` — pass

### Files Created

- `tests/unit/stall_detector_tests.rs` — 6 unit tests: timer fires, reset prevents firing, pause/resume toggle, consecutive nudge counting, self-recovery clears alert, cancellation stops detector
- `tests/contract/heartbeat_tests.rs` — 6 contract tests: input schema validation (status-only, valid snapshot, malformed snapshot, invalid status deser), output schema structure, omitted snapshot preserves existing
- `tests/integration/nudge_flow_tests.rs` — 3 integration tests: stall alert creation on silence, nudge updates alert and increments count, self-recovery clears active alert
- `docs/adrs/0005-stall-detector-architecture.md` — ADR for the detector design (Notify + AtomicBool + mpsc pattern)
- `docs/adrs/0006-surrealdb-schemafull-nested-fields.md` — ADR for nested field definitions in SCHEMAFULL tables

### Files Modified

- `src/orchestrator/stall_detector.rs` — from placeholder to full implementation: `StallDetector`, `StallDetectorHandle`, `StallEvent` enum, core timer loop with escalation
- `src/mcp/tools/heartbeat.rs` — from placeholder to full handler: validates progress snapshot, updates session DB, resets stall timer, optionally logs to Slack
- `src/slack/handlers/nudge.rs` — from placeholder to full handler: routes nudge/instruct/stop actions, validates authorized user, replaces Slack buttons with status text
- `src/mcp/handler.rs` — added `StallDetectors` type alias (`Arc<Mutex<HashMap<String, StallDetectorHandle>>>`), `stall_detectors` field on `AppState`, wired `heartbeat` tool route, added stall timer reset on every `call_tool`
- `src/slack/events.rs` — wired `stall_` action ID prefix to `handlers::nudge::handle_nudge_action()`
- `src/main.rs` — imports `StallDetectors`, passes `stall_detectors: Some(StallDetectors::default())` to `AppState`
- `src/persistence/schema.rs` — added `DEFINE FIELD progress_snapshot.*`, `progress_snapshot.*.label`, `progress_snapshot.*.status` on session, checkpoint, and stall_alert tables
- `tests/unit.rs` — registered `stall_detector_tests` module
- `tests/contract.rs` — registered `heartbeat_tests` module
- `tests/integration.rs` — registered `nudge_flow_tests` module
- `specs/001-mcp-remote-agent-server/tasks.md` — marked T110-T112, T047-T054 complete

## Important Discoveries

### Architecture Decisions

1. **Stall detector pattern (ADR-0005)**: Used `Arc<Notify>` for reset signaling instead of a channel because resets are fire-and-forget — only the most recent notification matters. Combined with `Arc<AtomicBool>` for paused/stalled flags to avoid holding any lock across `.await` points. Events flow out via `mpsc` channel to decouple detection from side-effects.

2. **SurrealDB SCHEMAFULL nested fields (ADR-0006)**: `SCHEMAFULL` mode strips any fields not explicitly defined, including nested object fields within arrays. Required `DEFINE FIELD progress_snapshot.* ON TABLE ... TYPE object` plus `*.label` and `*.status` for each table storing progress snapshots. This affects session, checkpoint, and stall_alert tables.

3. **`StallDetectorHandle` is not a Future**: Initially tried `handle.await` in tests, but the handle is a control struct, not a task future. The `JoinHandle` from `tokio::spawn` is detached (stored but not exposed). Callers shut down via `CancellationToken`, not by awaiting the handle. Tests use `drop(handle)` when cleanup is needed.

4. **Clippy `too_many_arguments` suppression**: The internal `StallDetector::run()` method takes 10 parameters (session_id, thresholds, sender, cancel token, shared atomics). Suppressed with `#[allow(clippy::too_many_arguments)]` and documented as internal plumbing. A config struct would add complexity without improving the API since `run()` is `async fn` (not callable externally).

### Integration Test TOML Ordering

TOML field placement matters: `authorized_user_ids` placed after the `[stall]` table section was parsed as a stall field. Fixed by placing all bare keys before any table sections in the test config string.

### Slack Button Replacement Pattern

When a Slack interaction button is pressed (nudge, stop), the original message buttons must be replaced with static status text via `chat.update`. This prevents double-submission (FR-022). The `handle_nudge_action` function constructs a replacement `SlackMessageContent` with section blocks showing the action taken and the actor.

## Next Steps

Phase 6 (User Story 3 — Remote Status Logging) is the next story. Key areas:

- `remote_log` MCP tool handler in `src/mcp/tools/remote_log.rs`
- Block Kit severity formatting (info, success, warning, error)
- Rate-limited Slack message queue integration
- Contract tests for `remote_log`

## Context to Preserve

- **StallDetectors type**: `Arc<Mutex<HashMap<String, StallDetectorHandle>>>` stored as `AppState.stall_detectors: Option<StallDetectors>`. Guarded with `if let Some(ref detectors) = state.stall_detectors`.
- **Stall event consumer**: The `mpsc::Receiver<StallEvent>` side is not yet wired to an orchestrator consumer. Phase 5 emits events but the consumer (posting Slack alerts, creating DB records, sending notifications) needs to be wired in a future phase or orchestrator integration task.
- **Heartbeat DB update pattern**: Uses `state.db.query("UPDATE session SET ... WHERE id = $id")` with `bind()`. Does not go through the session repo abstraction — direct query for performance since it only updates two fields.
- **Nudge modal placeholder**: The "Nudge with Instructions" action opens a Slack modal — currently logged but modal opening is not implemented (requires `views.open` API call). Flagged for future implementation.
- **applicator.rs placeholder**: Still exists as an empty module from Phase 2. Can be removed in Phase 14 polish.
- **Progress snapshot schema**: Defined on three tables (session, checkpoint, stall_alert). If the snapshot structure changes, all three table definitions must be updated in lock-step in `schema.rs`.
