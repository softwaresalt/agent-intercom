---
type: session-memory
date: 2026-07-03
branch: feat/013.003-controller-hardening
task: 013.003.001-T
---

# Session memory — F.3-T1 crash-detect → respawn → session resume

## Context

Resumed after a system restart interrupted the prior session mid-claim. On
resume the branch held only uncommitted backlog status flips (013.003-F and
013.003.001-T → active) plus three PR-review follow-up stash entries. No
implementation or harness had been written yet.

## Task completed

- **013.003.001-T** (F.3-T1, test-first): Agent crash-detect → respawn →
  session resume. Moved active → review after implementation + green gates.

## Files modified

- `src/orchestrator/child_monitor.rs` — added `ExitClass` + `classify_exit`
  (clean = code 0, else crash). Rewrote poll loop: clean exit terminates the
  session (existing behavior); crash routes to new `attempt_respawn`, bounded
  by `MAX_RESPAWN_ATTEMPTS = 3` per crash chain (in-memory counter carried
  forward across restarts). `spawn_child_monitor` gained an `Arc<GlobalConfig>`
  parameter. Added `notify` helper.
- `src/orchestrator/spawner.rs` — added `respawn_session` (marks crashed
  `Interrupted`, creates a restart session linked via `restart_of`, carrying
  owner/workspace/prompt/mode/protocol/channel/thread and `agent_session_id`
  forward = rebind, then spawns + activates). Extracted `build_agent_command`
  helper shared with `spawn_session` (DRY).
- `src/main.rs` — pass `Arc::clone(&state.config)` to `spawn_child_monitor`.
- `tests/unit/child_monitor_tests.rs` — 3 `classify_exit` tests (clean/crash/
  unknown) using real cross-platform `ExitStatus`; updated the fn-pointer
  signature assertion for the new config param.
- `tests/integration/crash_recovery_tests.rs` — `respawn_creates_resumed_
  session_linked_to_crashed`: induces recovery, asserts crashed → Interrupted
  and resumed → Active with `restart_of` + carried identity/`agent_session_id`.
  Cross-platform `host_cli` (`cmd /c echo` on Windows, `echo` on Unix).

## Decisions

- Restart = new session record linked via the model's existing `restart_of`
  field (not in-place reuse). This is the designed seam.
- Pending approval/prompt/progress migration is intentionally NOT done here —
  it is owned by F.3-T3 (pending-state persistence) and F.3-T4 (resume-state
  contract), which consume the `restart_of` linkage. Kept T1 within its width.
- Bounded respawn (max 3) added for crash-loop safety (circuit-breaker aligned)
  without a schema change (in-memory counter keyed per crash chain).
- `respawn_session` does not re-run `ensure_authorized` or the concurrency
  limit check: it recovers an already-authorized session and frees the crashed
  slot first (marks Interrupted before spawning).

## Quality gates (all green)

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`
- `cargo test` (644 unit + 301 integration + 250 contract + 7 live; 0 fail)

## Commits

- `d6c59f9` feat(orchestrator): crash-detect, respawn and resume agent sessions
- `d17bef3` chore(backlog): claim F.3 + 013.003.001-T review; stash follow-ups

## Next steps

- Review 013.003.001-T, then proceed with sibling F.3 tasks:
  - `013.003.002-T` durable steering-queue persistence (migration-first)
  - `013.003.003-T` correlation-ID + pending-state persistence (test-first)
  - `013.003.004-T` stdio-child reconnection/resume-state contract
    (characterization-first) — wires T2/T3 persistence into the resume path
    established here.
- Open follow-up stash items (unrelated to F.3): `CDA8EE31`, `8AE87207`,
  `EB0D238D` (PR #18/#19 review follow-ups).
- Runtime verification for F.3 closure: live "kill agent mid-task → auto
  respawn + resume with pending state intact" once T3/T4 land.
