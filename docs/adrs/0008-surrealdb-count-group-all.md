# ADR-0008: SurrealDB COUNT() Requires GROUP ALL for Single Aggregate

**Status**: Accepted
**Date**: 2026-02-11
**Phase**: 9 (User Story 7), Tasks T119, T068

## Context

The session spawner (`src/orchestrator/spawner.rs`) enforces a maximum
concurrent sessions limit by counting active sessions before spawning a new one.
The query in `SessionRepo::count_active` was written as:

```surql
SELECT count() AS count FROM session WHERE status = 'active'
```

This query returned **one row per matching record** rather than a single
aggregate row. When multiple active sessions existed, `take(0)` (which extracts
a single result from the SurrealDB response) failed with:

> "Tried to take only a single result from a query that contains multiple"

The root cause is that SurrealDB 1.5 treats `count()` as a per-row function
unless an explicit `GROUP` clause is present. Without `GROUP ALL`, the query
produces a result set with one `{ count: 1 }` row per matching record instead
of a single `{ count: N }` row.

## Decision

Added `GROUP ALL` to the count query:

```surql
SELECT count() AS count FROM session WHERE status = 'active' GROUP ALL
```

This forces SurrealDB to aggregate all matching rows into a single result,
producing `{ count: N }` regardless of how many sessions match.

## Alternatives Considered

- **`SELECT * FROM session WHERE status = 'active'` + Rust-side `.len()`**:
  fetches all session records just to count them. Rejected as wasteful for
  large session tables.
- **`SELECT VALUE count() FROM ...`**: `VALUE` syntax with aggregation
  functions does not produce the expected scalar in SurrealDB 1.5 without
  `GROUP`. Tested and produced the same per-row behavior. Rejected.

## Consequences

**Positive**:

- `count_active()` reliably returns a single aggregate count.
- The pattern is documented for future aggregate queries in the crate.
- Max concurrent sessions enforcement (FR-023) works correctly.

**Negative**:

- None identified. `GROUP ALL` is the idiomatic SurrealDB approach for
  ungrouped aggregation.

**Risks**:

- Other aggregate queries in the crate should be audited to ensure they also
  use `GROUP ALL` when a single result is expected. Currently, `count_active`
  is the only aggregate query.
