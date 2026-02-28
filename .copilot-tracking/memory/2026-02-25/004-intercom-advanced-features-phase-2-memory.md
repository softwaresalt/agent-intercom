# Phase 2 Memory — 004-intercom-advanced-features

**Date**: 2026-02-25  
**Branch**: 004-intercom-advanced-features  
**Spec**: 004-intercom-advanced-features  
**Phase**: 2 — Foundational (Blocking Prerequisites)

## Tasks Completed

- **T007** — `src/persistence/steering_repo.rs`: `SteeringRepo` with `insert`, `fetch_unconsumed`, `mark_consumed`, `purge`
- **T008** — `src/persistence/inbox_repo.rs`: `InboxRepo` with `insert`, `fetch_unconsumed_by_channel`, `mark_consumed`, `purge`
- **T009** — `src/models/policy.rs`: `CompiledWorkspacePolicy` struct wrapping `WorkspacePolicy` + `RegexSet` (pre-compiled patterns)
- **T010** — `src/policy/loader.rs`: `load()` now returns `CompiledWorkspacePolicy`; evaluator updated to accept it directly
- **T011** — `src/config.rs`: `slack_detail_level: SlackDetailLevel` field added to `GlobalConfig` (enum: Minimal/Standard/Verbose, default Standard)
- **T012** — `src/mcp/handler.rs`: `AppState` gains `policy_cache: Arc<PolicyCache>` and `audit_logger: Option<Arc<dyn AuditLogger>>`
- **T013** — `src/persistence/mod.rs`: `SteeringRepo` and `InboxRepo` registered

## Key Design Decisions

- `CompiledWorkspacePolicy` holds `.raw: WorkspacePolicy` + `.matchers: RegexSet` — all policy field access via `.raw.field`
- `AuditLogger` is optional in `AppState` (tests pass `None`, production creates `JsonlAuditWriter`)
- `PolicyEvaluator::evaluate` now accepts `&CompiledWorkspacePolicy` directly (eliminates `.raw` access in tool layer)
- `SlackDetailLevel` derives `Default` = `Standard`; serializes as lowercase string in TOML

## Test Results

- 570 tests passing (unit + contract + integration)
- Clippy: clean (0 warnings)
- Format: clean

## Files Modified

`src/persistence/steering_repo.rs` (new), `src/persistence/inbox_repo.rs` (new), `src/models/policy.rs`, `src/policy/loader.rs`, `src/policy/evaluator.rs`, `src/policy/watcher.rs`, `src/config.rs`, `src/mcp/handler.rs`, `src/mcp/tools/check_auto_approve.rs`, `src/main.rs`, `src/persistence/mod.rs`, all test files constructing `AppState`

## Next Phase

Phase 3 — User Story 1: Operator Steering Queue (T014-T023)
