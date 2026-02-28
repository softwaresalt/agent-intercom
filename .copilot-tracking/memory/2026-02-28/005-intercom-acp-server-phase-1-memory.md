# Session Memory: 005-intercom-acp-server Phase 1

**Date**: 2026-02-28
**Spec**: `specs/005-intercom-acp-server/`
**Phase**: 1 — Setup
**Status**: COMPLETE

---

## Task Overview

Phase 1 establishes the scaffolding for the ACP (Agent Client Protocol) feature.
Five tasks created the new module stubs and extended existing types:

- T001: `src/driver/mod.rs` — `AgentDriver` trait + `AgentEvent` enum
- T002: `src/acp/mod.rs` — empty module stub for future ACP stream handling
- T003: `AppError::Acp(String)` variant added to `src/errors.rs`
- T004: `ProtocolMode { Mcp, Acp }` enum added to `src/models/session.rs`
- T004b: `tokio-util` codec feature enabled in `Cargo.toml`

---

## Current State

### Tasks Completed

All 5 Phase 1 tasks marked `[x]` in `specs/005-intercom-acp-server/tasks.md`.

### Files Modified

| File | Change |
|------|--------|
| `src/driver/mod.rs` | NEW — AgentDriver trait (5 methods), AgentEvent enum (5 variants) |
| `src/acp/mod.rs` | NEW — empty module stub with doc comments |
| `src/lib.rs` | Added `pub mod acp` and `pub mod driver` |
| `src/errors.rs` | Added `Acp(String)` variant + Display arm |
| `src/models/session.rs` | Added `ProtocolMode { Mcp, Acp }` enum |
| `Cargo.toml` | tokio-util features `["rt"]` → `["rt", "codec"]` |
| `docs/adrs/0014-agent-driver-trait-protocol-abstraction.md` | NEW — ADR for trait design decision |
| `specs/005-intercom-acp-server/tasks.md` | Tasks T001–T004b marked complete |

### Test Results

- `cargo test`: 239 passed, 0 failed, + 2 doc-tests ✅
- `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`: PASS ✅
- `cargo fmt --all -- --check`: PASS ✅

### Adversarial Review

0 critical, 0 high, 0 medium, 0 low findings. Phase 1 is pure scaffolding
with no logic — no security surface, no data flows, no error paths.

---

## Important Discoveries

### AgentDriver Trait Design

The trait uses `Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>` for
async methods instead of `async_trait`. This is the correct pattern for
object-safe async traits in Rust stable as of 2026. The `'_` lifetime captures
`&self` so futures can borrow from the implementing struct.

**No `async_trait` dependency** — the `Pin<Box>` approach avoids the proc-macro
crate and is idiomatic for library-level trait definitions.

### AppError::Acp vs AppError::Io

`AppError::Acp` is a new distinct variant for ACP stream errors, separate from
`AppError::Io`. This keeps ACP failures distinguishable from general I/O errors
for better operator diagnostics and cleaner error handling in future phases.

### tokio-util codec Feature

The `codec` feature on `tokio-util` enables `LinesCodec`, `FramedRead`, and
`FramedWrite` — the core primitives for ACP NDJSON stream framing in Phase 9.
The feature was added here (Phase 1) to keep Cargo.toml changes in the setup
phase where they belong.

### ProtocolMode Not Yet on Session Struct

`ProtocolMode` is defined but NOT yet added as a field on `Session`. That
happens in Phase 2 (T008) alongside the other new session fields
(`channel_id`, `thread_ts`, `connectivity_status`, `last_activity_at`,
`restart_of`). Phase 1 only scaffolds types; Phase 2 wires them in.

---

## Next Steps

**Phase 2 (Foundational)** must complete next — it BLOCKS all user stories:

1. Tests first (T005–T007):
   - Unit test: `ProtocolMode` serde round-trip in `tests/unit/session_model_tests.rs`
   - Unit test: `AppError::Acp` display format in `tests/unit/error_tests.rs`
   - Unit test: `AgentEvent` construction in `tests/unit/driver_trait_tests.rs`

2. Then implementation (T008–T014):
   - Add 6 new fields to `Session` struct
   - Schema migration in `persistence/schema.rs` (PRAGMA table_info check pattern)
   - Update all SessionRepo queries to include new fields
   - Add 3 new query methods to SessionRepo
   - Add 2 new indexes to schema

**Watch for**:
- PRAGMA table_info check pattern for idempotent column additions (see existing schema.rs)
- All SessionRepo INSERT/SELECT/UPDATE queries must include the new fields
- `connectivity_status` needs a new enum type (similar to `SessionStatus`)

---

## Context to Preserve

### Key Source Files

- `src/driver/mod.rs` — AgentDriver trait definition (stable, not expected to change in Phase 2)
- `src/models/session.rs` — Session struct will gain 6 new fields in Phase 2
- `src/persistence/session_repo.rs` — All queries must be updated in Phase 2
- `src/persistence/schema.rs` — DDL migration will use PRAGMA table_info check pattern
- `specs/005-intercom-acp-server/contracts/agent-driver.md` — Full trait spec
- `specs/005-intercom-acp-server/data-model.md` — Session model extended fields

### Open Questions

None for Phase 2. All implementation is clearly specified in tasks.md and the data-model.md.
