# ADR-0006: SurrealDB SCHEMAFULL Nested Field Definitions

**Status**: Accepted
**Date**: 2026-02-11
**Phase**: 5 (User Story 4), Task T049

## Context

SurrealDB 1.5 in `SCHEMAFULL` mode strips any fields from records that are not
explicitly defined via `DEFINE FIELD`. This applies recursively: fields inside
nested objects and arrays must also be individually defined.

The `progress_snapshot` field on `session`, `checkpoint`, and `stall_alert`
tables is typed as `TYPE option<array>` and contains objects with `label` and
`status` string fields. After inserting records with progress snapshots, reads
returned empty objects inside the array because the nested fields were not
defined.

## Decision

Added explicit nested field definitions for each table that stores
`progress_snapshot`:

```surql
DEFINE FIELD progress_snapshot.* ON TABLE session TYPE object;
DEFINE FIELD progress_snapshot.*.label ON TABLE session TYPE string;
DEFINE FIELD progress_snapshot.*.status ON TABLE session TYPE string;
```

The pattern `field.*` defines the type of each element in the array, and
`field.*.subfield` defines the type of each subfield within those elements.

This was applied to the `session`, `checkpoint`, and `stall_alert` tables in
`src/persistence/schema.rs`.

## Alternatives Considered

- **`SCHEMALESS` tables**: would avoid the problem entirely but loses
  type-safety guarantees that `SCHEMAFULL` provides. Rejected because the
  crate's architecture relies on schema enforcement.
- **Storing JSON strings**: serialize the snapshot to a JSON string and store
  as `TYPE string`. Rejected because it prevents SurrealDB queries from
  filtering or projecting on snapshot fields.

## Consequences

**Positive**:

- `SCHEMAFULL` mode correctly preserves all nested fields on insert and read.
- The nested field syntax is documented and consistent across all three tables.

**Negative**:

- Schema DDL grows more verbose for every nested object. Future fields within
  `progress_snapshot` items require corresponding `DEFINE FIELD` statements.
- This is a SurrealDB 1.x pattern; version 2.x may improve nested field
  handling, requiring DDL review on upgrade.
