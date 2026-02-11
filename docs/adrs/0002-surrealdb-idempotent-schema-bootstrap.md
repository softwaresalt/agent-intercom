# ADR-0002: SurrealDB Schema with IF NOT EXISTS for Idempotent Bootstrap

**Status**: Accepted
**Date**: 2026-02-11
**Phase**: 2 (Foundational), Task T017

## Context

The embedded SurrealDB schema must be applied on every server startup to ensure tables and fields exist. Early Phase 1 code used bare `DEFINE TABLE` / `DEFINE FIELD` statements, which would fail or overwrite on subsequent runs. The schema also lacked field-level `ASSERT` constraints to enforce enum invariants at the database layer.

## Decision

Moved all DDL into a dedicated `persistence/schema.rs` module. Every statement uses `DEFINE ... IF NOT EXISTS` making re-execution safe across restarts. Added `ASSERT $value IN [...]` constraints on enum-backed string fields (`status`, `mode`, `risk_level`, `prompt_type`, `StallAlertStatus`) to enforce valid values at the database level, not just at the Rust type level.

The schema covers all five SCHEMAFULL tables: `session`, `approval_request`, `checkpoint`, `continuation_prompt`, `stall_alert`. New fields added in Phase 2 (`workspace_root`, `terminated_at`, `progress_snapshot`) are included.

## Consequences

**Positive**:
- Schema is idempotent — safe to run on fresh databases and existing ones.
- ASSERT constraints provide defense-in-depth against invalid data.
- Schema is centralized in one module, easy to review and extend.

**Negative**:
- `IF NOT EXISTS` means field type changes require manual migration (won't auto-alter existing fields).
- Future schema migrations will need a versioning strategy beyond this initial bootstrap.
