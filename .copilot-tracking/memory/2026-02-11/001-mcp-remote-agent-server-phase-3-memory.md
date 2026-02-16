# Session Memory: Phase 3 — User Story 1 (Remote Code Review and Approval)

**Feature**: 001-mcp-remote-agent-server
**Phase**: 3 (User Story 1)
**Date**: 2026-02-11
**Status**: Complete

## Task Overview

Phase 3 implements User Story 1: Remote Code Review and Approval. Six tasks
covering contract tests, integration tests, the `ask_approval` MCP tool handler,
Slack approval interaction callback, timeout logic, pending request wiring, and
tracing spans.

## Current State

### Completed Tasks (6/6)

| ID   | Description                       | Status |
| ---- | --------------------------------- | ------ |
| T105 | `ask_approval` contract tests     | Done   |
| T106 | Approval flow integration tests   | Done   |
| T038 | `ask_approval` tool handler       | Done   |
| T039 | Approval interaction callback     | Done   |
| T040 | Approval timeout logic            | Done   |
| T041 | Pending approval map wiring       | Done   |
| T042 | `ask_approval` tracing spans      | Done   |

### Test Results (61/61 pass)

| Suite       | Count | Status |
| ----------- | ----- | ------ |
| Contract    | 17    | Pass   |
| Integration | 7     | Pass   |
| Unit        | 37    | Pass   |
| **Total**   | **61**| **Pass** |

### Toolchain Gates

- `cargo check` — pass
- `cargo clippy -- -D warnings -D clippy::pedantic` — pass
- `cargo test` — 61/61 pass
- `cargo fmt --all -- --check` — pass

### Files Created (Phase 3)

- `src/mcp/tools/ask_approval.rs` — tool handler with SHA-256 hashing,
  inline/snippet diff rendering, oneshot blocking, timeout logic, tracing spans
- `src/slack/handlers/approval.rs` — Accept/Reject button handler with owner
  verification, DB update, oneshot resolution, button replacement
- `tests/contract/ask_approval_tests.rs` — 14 contract tests validating
  input/output JSON schemas against mcp-tools.json
- `tests/integration/approval_flow_tests.rs` — 7 integration tests covering
  DB record lifecycle, accept/reject flows, oneshot resolution, and timeout
  expiry
- `tests/contract.rs` — root entry file for contract test discovery
- `tests/unit.rs` — root entry file for unit test discovery
- `tests/integration.rs` — root entry file for integration test discovery

### Files Modified (Phase 3 — bug fixes discovered during first test run)

- `src/models/mod.rs` — added `deserialize_surreal_id` function using
  `surrealdb::sql::Thing`
- `src/models/approval.rs` — added `#[serde(skip_serializing, default,
  deserialize_with)]` on `id` field
- `src/models/session.rs` — same serde annotation on `id`
- `src/models/checkpoint.rs` — same serde annotation on `id`
- `src/models/prompt.rs` — same serde annotation on `id`
- `src/models/stall.rs` — same serde annotation on `id`
- `src/persistence/schema.rs` — removed `IF NOT EXISTS` (unsupported in
  SurrealDB 1.5); removed `TYPE datetime` / `TYPE option<datetime>` constraints
- `tests/unit/config_tests.rs` — fixed TOML literal strings for Windows paths,
  moved `authorized_user_ids` before `[slack]` section, canonicalized path
  assertions
- `tests/unit/model_tests.rs` — removed `id` equality assertions from
  round-trip tests (id is skip_serializing), prefixed unused `back` variable
- `tests/unit/path_validation_tests.rs` — canonicalized expected paths in
  assertions, changed boundary test from `/etc/passwd` to relative escape
- `tests/unit/session_repo_tests.rs` — fixed TOML literal strings and
  `authorized_user_ids` placement
- `tests/contract/schema_tests.rs` — same TOML fixes
- `tests/integration/approval_flow_tests.rs` — same TOML fixes, removed
  unused import
- `docs/adrs/0002-surrealdb-idempotent-schema-bootstrap.md` — amended to
  document `IF NOT EXISTS` removal

### ADRs Created

- `docs/adrs/0004-surrealdb-record-id-serde-pattern.md` — documents the
  `skip_serializing + default + deserialize_with` pattern for SurrealDB 1.5
  record ID handling

## Important Discoveries

### Pre-existing Phase 2 Bugs (8 cascading issues)

Tests had never run before this session (MSVC linker was unavailable). First
successful compilation exposed eight cascading bugs from Phase 2:

1. **TOML `\U` unicode escape** — Windows temp paths contain `\U` in
   `C:\Users\...`; TOML interprets `\U` as a Unicode escape in basic strings.
   Fixed by using TOML literal strings (single quotes).
2. **TOML structure** — `authorized_user_ids` was placed inside the `[slack]`
   table instead of at the top level. Fixed in all test TOML fixtures.
3. **SurrealDB `IF NOT EXISTS`** — unsupported in SurrealDB 1.5 (2.x feature).
   Removed from all DEFINE statements; 1.x DEFINE is already idempotent.
4. **SurrealDB double-ID conflict** — `.create(("table", id)).content(struct)`
   fails when content also has `id`. Fixed with `#[serde(skip_serializing)]`.
5. **SurrealDB `TYPE datetime`** — chrono ISO 8601 strings rejected by the SDK.
   Removed datetime type constraints from schema DDL.
6. **SurrealDB `Thing` deserialization** — SDK returns `id` as internal `Thing`
   type, not plain String. Fixed with custom `deserialize_surreal_id`.
7. **Model JSON round-trip** — `skip_serializing` on `id` causes "missing
   field" during deserialization. Fixed with `#[serde(default)]`.
8. **Windows `\\?\` path prefix** — `canonicalize()` adds `\\?\` on Windows.
   Fixed test assertions to also canonicalize expected paths.

### Key Insight

All eight bugs were latent from Phase 2. The MSVC linker was unavailable, so
`cargo test` never compiled. The first failure (TOML parsing) masked all
subsequent issues. TDD works only when tests can actually run.

## Next Steps

- **Phase 4** (User Story 2 — Programmatic Diff Application): implements
  `accept_diff` tool handler, file writing, diff/patch application, hash
  integrity checking.
  - Tasks: T107-T109 (tests), T043-T046 (implementation)
  - Key files: `src/diff/writer.rs`, `src/diff/patcher.rs`,
    `src/mcp/tools/accept_diff.rs`

## Context to Preserve

- PATH must be refreshed in each new terminal session using the registry reload
  pattern: `$machinePath = [System.Environment]::GetEnvironmentVariable("Path",
  "Machine"); $userPath = [System.Environment]::GetEnvironmentVariable("Path",
  "User"); $env:Path = "$machinePath;$userPath"`
- MSVC tools location: `D:\MVS2022\VC\Tools\MSVC\14.50.35717\bin\Hostx64\x64\`
- RocksDB C++ native dependency: already compiled; subsequent builds are fast
- SurrealDB 1.5 quirks documented in ADR-0002 (amended) and ADR-0004
- Test entry files (`tests/contract.rs`, `tests/unit.rs`,
  `tests/integration.rs`) are required for Cargo to discover subdirectory tests
