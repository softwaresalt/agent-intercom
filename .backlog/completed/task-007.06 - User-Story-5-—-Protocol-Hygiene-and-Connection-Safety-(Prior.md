---
id: TASK-007.06
title: "007 - User Story 5 — Protocol Hygiene and Connection Safety (Priority: P2)"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-007
dependencies: []
ordinal: 7060
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Remove legacy `channel_id` query parameter from MCP endpoint and replace
counter-based correlation IDs with UUIDs.

**Independent Test**: Connect to `/mcp` with only `workspace_id`; verify `channel_id` is
ignored. Generate thousands of correlation IDs and verify zero collisions.

**Fixes**: F-10, F-13 | **Scenarios**: S016–S026 | **FRs**: FR-007, FR-008

### Tests for User Story 5 — F-10 (channel_id removal) ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T018 [P] [US5] Unit test: `update_pending_from_uri` only extracts `session_id` and `workspace_id` (S018, S019) in `tests/unit/sse_workspace_only_routing.rs`
- [x] T019 [P] [US5] Unit test: `workspace_id` resolves channel from mapping (S016) in `tests/unit/sse_workspace_only_routing.rs`
- [x] T020 [P] [US5] Unit test: unknown workspace_id logs warning, no channel (S020) in `tests/unit/sse_workspace_only_routing.rs`
- [x] T021 [P] [US5] Contract test: `/mcp?channel_id=C_DIRECT` — channel_id silently ignored (S018) in `tests/contract/mcp_no_channel_id_contract.rs`
- [x] T022 [P] [US5] Update existing `workspace_mapping_tests.rs` — remove/update tests that reference `channel_id` as fallback param in `tests/unit/workspace_mapping_tests.rs`

### Tests for User Story 5 — F-13 (correlation ID uniqueness) ⚠️

- [x] T023 [P] [US5] Unit test: handshake IDs match `intercom-{purpose}-{uuid}` pattern (S022) in `tests/unit/correlation_id_uniqueness.rs`
- [x] T024 [P] [US5] Unit test: runtime prompt IDs match UUID pattern (S023) in `tests/unit/correlation_id_uniqueness.rs`
- [x] T025 [P] [US5] Unit test: 10,000 IDs with zero collisions (S024) in `tests/unit/correlation_id_uniqueness.rs`
- [x] T026 [P] [US5] Unit test: concurrent sessions produce distinct IDs (S026) in `tests/unit/correlation_id_uniqueness.rs`

### Implementation for User Story 5 — F-10

- [x] T027 [US5] Remove `channel_id` extraction from `update_pending_from_uri()` in `src/mcp/sse.rs` — only extract `session_id` and `workspace_id`
- [x] T028 [US5] Change `PendingParams` type from 3-tuple to 2-tuple `(Option<String>, Option<String>)` for `(session_id, workspace_id)` in `src/mcp/sse.rs`
- [x] T029 [US5] Remove `raw_channel` fallback branch from factory closure in `src/mcp/sse.rs` — resolve channel exclusively via workspace mappings
- [x] T030 [US5] Update module-level doc comment in `src/mcp/sse.rs` — document `workspace_id` as the only routing query parameter
- [x] T031 [US5] Update `resolve_channel_id()` signature in `src/config.rs` — remove `channel_id` fallback parameter
- [x] T032 [US5] Update `tests/unit/workspace_routing_tests.rs` — remove `channel_id` as second arg in `resolve_channel_id` calls
- [x] T033 [US5] Update `tests/integration/channel_override_tests.rs` — rewrite for workspace_id-only routing or remove channel_id-specific tests

### Implementation for User Story 5 — F-13

- [x] T034 [US5] Replace static `INIT_ID`, `SESSION_NEW_ID`, `PROMPT_ID` constants with UUID-based generation in `src/acp/handshake.rs` — use `format!("intercom-{purpose}-{}", Uuid::new_v4())`
- [x] T035 [US5] Remove `PROMPT_COUNTER` static and replace with UUID-based generation in `src/driver/acp_driver.rs` — all `resolve_clearance` and `resolve_prompt` calls use `Uuid::new_v4()`
- [x] T036 [US5] Verify all existing tests pass after F-10 and F-13 changes — run `cargo test`

**Checkpoint**: MCP endpoint accepts only `workspace_id`; all correlation IDs are UUID-based with zero collision risk.

---

<!-- SECTION:DESCRIPTION:END -->
