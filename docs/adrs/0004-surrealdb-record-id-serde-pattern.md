# ADR-0004: SurrealDB Record ID Serde Pattern

**Status**: Superseded — SurrealDB replaced by SQLite via sqlx (see specs/002-sqlite-migration)
**Date**: 2026-02-09
**Phase**: 3 (User Story 1), Tasks T038-T042

## Context

SurrealDB 1.5 has two interacting behaviors that conflict with straightforward
`#[derive(Serialize, Deserialize)]` on domain model structs:

1. **Double-ID conflict**: when using `.create(("table", id)).content(struct)`,
   the SDK rejects content that also contains an `id` field, producing
   "a]specific record has been specified" errors.
2. **Thing type on read**: the SDK returns the `id` column as an internal
   `surrealdb::sql::Thing { tb, id }` value, not a plain `String`. Standard
   `String` deserialization fails with a type mismatch.

A secondary issue arose with `TYPE datetime` column definitions: SurrealDB 1.5
rejects chrono `DateTime<Utc>` values serialized as ISO 8601 strings through
the Rust SDK, even though the string format is valid.

## Decision

Applied a three-part serde annotation pattern to the `id: String` field on all
five persisted domain models (`Session`, `ApprovalRequest`, `Checkpoint`,
`ContinuationPrompt`, `StallAlert`):

```rust
#[serde(
    skip_serializing,
    default,
    deserialize_with = "super::deserialize_surreal_id"
)]
pub id: String,
```

- **`skip_serializing`**: omits `id` from the serialized content passed to
  `.create()`, preventing the double-ID conflict.
- **`default`**: allows JSON round-trip deserialization (e.g., in tests) where
  `id` is absent from the payload, defaulting to `String::default()`.
- **`deserialize_with`**: uses a shared `deserialize_surreal_id` function in
  `src/models/mod.rs` that deserializes the SDK `Thing` type and extracts the
  plain key string via `Id::String(s) => s`.

For the datetime issue, removed `TYPE datetime` and `TYPE option<datetime>`
constraints from the SurrealDB schema DDL. Datetime fields are now untyped at
the database layer, accepting chrono's ISO 8601 string serialization.

## Consequences

**Positive**:

- All five repository modules use the standard
  `.create(("table", id)).content(entity)` pattern without workarounds.
- Read operations correctly deserialize record IDs from the SDK `Thing` type.
- Domain model structs remain `#[derive(Serialize, Deserialize)]` without
  manual `impl` blocks.

**Negative**:

- `skip_serializing` on `id` means JSON serialization of models omits the ID.
  Consumers that need the ID in serialized output must handle it separately.
- Untyped datetime fields lose database-level type enforcement. Validation
  relies on Rust's type system and serde deserialization.
- The `default` attribute silently produces an empty string for `id` when
  deserializing JSON without an `id` field, which could mask bugs if not
  caught by higher-level validation.

**Risks**:

- Upgrading to SurrealDB 2.x may resolve both the `Thing` deserialization and
  `TYPE datetime` issues, at which point this pattern should be revisited and
  simplified.
