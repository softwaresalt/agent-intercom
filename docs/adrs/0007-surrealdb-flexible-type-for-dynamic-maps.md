# ADR-0007: SurrealDB FLEXIBLE TYPE for Dynamic HashMap Fields

**Status**: Accepted
**Date**: 2026-02-11
**Phase**: 9 (User Story 7), Tasks T119-T120

## Context

SurrealDB 1.5 in `SCHEMAFULL` mode requires explicit `DEFINE FIELD` statements
for every field on a table. For fields with statically-known subfields (such as
`progress_snapshot` arrays), ADR-0006 established the pattern of defining each
nested field individually.

However, the `checkpoint` table stores two fields whose keys are
**runtime-determined**:

- `file_hashes`: a `HashMap<String, String>` mapping file paths to SHA-256
  digests. The keys are arbitrary file paths that vary per workspace.
- `session_state`: a serialized `serde_json::Value` representing the session
  snapshot, whose structure depends on session mode and active tool state.

Defining these as `TYPE object` under `SCHEMAFULL` mode caused SurrealDB to
silently strip all dynamically-keyed subfields on insert. Records were stored
with empty objects, and reads returned `{}` for both fields despite successful
inserts.

## Decision

Changed the field definitions in `src/persistence/schema.rs` to use
`FLEXIBLE TYPE object` instead of `TYPE object`:

```surql
DEFINE FIELD file_hashes ON TABLE checkpoint FLEXIBLE TYPE object;
DEFINE FIELD session_state ON TABLE checkpoint FLEXIBLE TYPE object;
```

The `FLEXIBLE` modifier tells SurrealDB to accept and store any arbitrarily
nested keys within the field while still enforcing the top-level type as
`object`. This preserves `SCHEMAFULL` enforcement for all other fields on the
table.

## Alternatives Considered

- **Switch to `SCHEMALESS` table**: removes all schema enforcement, which the
  crate relies on for data integrity. Rejected.
- **Define `file_hashes.* TYPE string`**: the wildcard `*` pattern works for
  array elements but does not match arbitrary object keys in SurrealDB 1.5.
  Tested and confirmed it still strips unknown keys. Rejected.
- **Serialize to JSON string**: store the map as `TYPE string` with
  `serde_json::to_string`. Works but prevents querying individual file hashes
  from SurrealQL. Rejected as it limits future query flexibility.

## Consequences

**Positive**:

- Dynamic HashMap fields are correctly persisted and retrieved.
- `SCHEMAFULL` enforcement is maintained for all statically-known fields.
- Pattern is self-documenting: `FLEXIBLE TYPE object` signals to future
  developers that the field accepts arbitrary keys.

**Negative**:

- SurrealDB cannot validate the value types of individual keys within
  `FLEXIBLE` fields. Incorrect values (e.g., a number instead of a string hash)
  would be stored without error. Mitigation: Rust-side serialization through
  typed `HashMap<String, String>` ensures correct types at the application layer.

**Risks**:

- If SurrealDB changes `FLEXIBLE` semantics in a future version, the field
  definitions may need updating. Low risk given it is a documented feature.
