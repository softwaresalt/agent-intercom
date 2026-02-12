# ADR-0005: Stall Detector Architecture

**Status**: Accepted
**Date**: 2026-02-11
**Phase**: 5 (User Story 4), Tasks T047-T054

## Context

The MCP server needs per-session stall detection: when an agent stops making
tool calls for a configurable threshold, the operator must be alerted via Slack
and the agent nudged to resume. The design must support:

- Independent timers per session (multiple agents may be connected).
- Reset on any MCP tool call or explicit `heartbeat` calls.
- Pause/resume for long-running server operations.
- Auto-nudge escalation with configurable retry limits.
- Self-recovery detection when an agent resumes before operator intervention.
- Graceful shutdown without leaked tasks.

## Decision

Implemented a three-component architecture:

### 1. `StallDetector` + `StallDetectorHandle`

`StallDetector` is a builder that, on `spawn()`, launches a background `tokio`
task and returns a `StallDetectorHandle`. The handle exposes `reset()`,
`pause()`, `resume()`, and `is_stalled()` as synchronous methods that
manipulate shared atomic state:

- **`Arc<Notify>`** for reset signaling — `notify_one()` wakes the timer
  `select!` branch without requiring a channel.
- **`Arc<AtomicBool>`** for the paused flag — checked in a spin-wait loop
  with 50ms poll interval before starting the sleep timer.
- **`Arc<AtomicBool>`** for the stalled flag — set when the threshold fires,
  cleared on self-recovery.

### 2. `StallEvent` enum via `mpsc` channel

Events (`Stalled`, `AutoNudge`, `Escalated`, `SelfRecovered`) are sent through
a `tokio::sync::mpsc` channel to the orchestrator. This decouples detection
from side-effects (Slack posting, DB writes, notifications), keeping the
detector pure and testable.

### 3. `StallDetectors` type alias on `AppState`

A `HashMap<String, StallDetectorHandle>` behind `Arc<Mutex<>>` is stored in
`AppState.stall_detectors`. The MCP handler resets the detector for the active
session on every `call_tool` invocation. The heartbeat tool also resets it
explicitly.

## Alternatives Considered

- **Single global timer polling all sessions**: simpler but creates coupling
  between sessions and makes per-session pause/resume awkward.
- **`tokio::time::Interval`**: considered but `Interval` cannot be reset to
  restart its period; manual `sleep` + `select!` is more flexible.
- **`watch` channel for reset**: more overhead than `Notify` for a fire-and-
  forget reset signal.

## Consequences

**Positive**:

- Each session's stall detector is fully independent — no cross-session
  interference.
- The `mpsc` channel pattern keeps side-effects out of the detector, enabling
  unit tests with fast timers and no Slack/DB dependencies.
- `CancellationToken` integration ensures graceful shutdown without task leaks.
- Atomic flags avoid holding any lock across `.await` points.

**Negative**:

- The 50ms spin-wait during pause adds minor CPU overhead. Acceptable for
  the expected session count (single digits).
- `StallDetectorHandle` does not retain the `JoinHandle` — callers rely on
  `CancellationToken` for shutdown rather than awaiting task completion
  directly. This is intentional but means panics in the detector task are not
  propagated to the caller.
