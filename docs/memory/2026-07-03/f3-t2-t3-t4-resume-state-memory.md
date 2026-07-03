---
type: session-memory
date: 2026-07-03
branch: feat/013.003-controller-hardening
feature: 013.003-F
tasks: [013.003.002-T, 013.003.003-T, 013.003.004-T]
---

# Session memory — F.3 T2/T3/T4 (durable resume state)

Continues the same session that shipped F.3-T1 (crash-detect → respawn →
resume). Completed the remaining three F.3 tasks; feature 013.003-F now in
`review` with all four tasks in `review`.

## Tasks completed

- **T2 013.003.002-T** durable steering-queue rebind — commit 6a968af
- **T3 013.003.003-T** correlation-ID + pending-state persistence — commit 01d039e
- **T4 013.003.004-T** stdio-child resume-state contract (wiring + ADR) — commit 380a894

## Key reality-check

Explore probes found the steering queue and approval/prompt records **already
persist** in file-backed SQLite (survive restart) and `recover_state` already
restores them. So T2/T3 were NOT "add durability" — the real gap for *resume*
is that respawn creates a **new session id** (T1), orphaning state keyed to the
crashed session id. T2/T3 provide durable **reassign** primitives; T4 wires
them into the respawn path.

## Files modified

- `src/persistence/schema.rs` — additive `origin_session_id` column on
  `steering_message` + idempotent `migrate_steering_columns`.
- `src/models/steering.rs` — `origin_session_id: Option<String>` field.
- `src/persistence/steering_repo.rs` — `reassign_unconsumed_to_session(from,to)`
  (COALESCE preserves first origin across chained restarts).
- `src/persistence/approval_repo.rs` — `reassign_pending_to_session` (pending only).
- `src/persistence/prompt_repo.rs` — `reassign_pending_to_session` (undecided only).
- `src/orchestrator/spawner.rs` — `respawn_session` gained `db: &Arc<Database>`
  param + `rebind_pending_state` helper (steering+approval+prompt reassign,
  best-effort). Updated child_monitor call site.
- `docs/adrs/0017-stdio-child-resume-state-contract.md` — the contract.
- Tests: steering_repo_tests (+5), approval_repo_tests (+2), prompt_repo_tests
  (+2), schema_tests (+1 contract), crash_recovery_tests (+1 characterization,
  updated T1 respawn signature).

## Decisions

- Correlation ids = the approval/prompt `id` (agent-generated). Reassign keeps
  `id` intact → "correlation ids restored". No new correlation column needed.
- `origin_session_id` added only to steering (traceability + chained-restart
  origin); approvals/prompts rely on stable `id` + session `restart_of` chain.
- Rebind is best-effort in respawn: a reassign failure is logged, not fatal —
  a live resumed session beats none.
- In-memory oneshot re-wiring / Slack re-post after full server restart is NOT
  in scope — owned by ADR-0011 reconnect re-post + `recover_state` (documented
  as a Negative in ADR-0017).

## Quality gates (all green, run after each task)

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`
- `cargo test` — 653 unit + 251 contract + 302 integration + 7 live; 0 failures

## Commits

- d6c59f9 feat(orchestrator): crash-detect, respawn and resume (T1)
- 6a968af feat(persistence): durable steering-queue rebind (T2)
- 01d039e feat(persistence): rebind pending clearances and prompts (T3)
- 380a894 feat(orchestrator): wire pending-state rebind into resume path (T4)

## Next steps

- Feature-level review + PR for 013.003-F (Ship pipeline).
- Runtime verification (F.3 DoD): live "kill agent mid-task → auto respawn +
  resume with pending state intact" — recovery runbook + rollback trigger
  (closure artifact still outstanding per feature DoD).
- Then F.5 (013.005-F) is unblocked once F.1 ADR (013.004-F) merges and
  F.2/F.3 are accepted (operator CP-1 gate).
- Open PR-review follow-up stash items: CDA8EE31, 8AE87207, EB0D238D.
