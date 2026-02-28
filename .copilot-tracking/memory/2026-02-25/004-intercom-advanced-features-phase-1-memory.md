# Phase 1 Memory — 004-intercom-advanced-features

**Date**: 2026-02-25
**Spec**: `004-intercom-advanced-features`
**Phase**: 1 — Setup (Project initialization — new modules and schema)
**Branch**: `004-intercom-advanced-features`

---

## Task Overview

Phase 1 is the Setup phase — no user story logic, only foundational scaffolding:
new data models, database DDL, and the audit logging module. All 6 tasks
completed successfully. This phase is a hard prerequisite for Phase 2 (Foundational).

---

## What Was Done

### Tasks Completed

| Task | Description | Result |
|---|---|---|
| T001 | Add steering_message + task_inbox DDL to persistence/schema.rs | ✅ |
| T002 | Create src/models/steering.rs — SteeringMessage struct | ✅ |
| T003 | Create src/models/inbox.rs — TaskInboxItem struct | ✅ |
| T004 | Create src/audit/mod.rs — AuditLogger trait + AuditEntry | ✅ |
| T005 | Create src/audit/writer.rs — JsonlAuditWriter (daily rotation) | ✅ |
| T006 | Register modules in src/models/mod.rs and src/lib.rs | ✅ |

### Files Modified/Created

| File | Action | Purpose |
|---|---|---|
| `src/persistence/schema.rs` | Modified | Added steering_message + task_inbox DDL + indexes |
| `src/models/steering.rs` | Created | SteeringMessage model with SteeringSource enum |
| `src/models/inbox.rs` | Created | TaskInboxItem model with InboxSource enum |
| `src/audit/mod.rs` | Created | AuditLogger trait, AuditEntry struct with builder methods |
| `src/audit/writer.rs` | Created | JsonlAuditWriter with std::sync::Mutex daily rotation |
| `src/models/mod.rs` | Modified | Added pub mod inbox + pub mod steering |
| `src/lib.rs` | Modified | Added pub mod audit |
| `specs/004-intercom-advanced-features/tasks.md` | Modified | Marked T001-T006 as [X] |
| `docs/adrs/0013-audit-logger-sync-trait-no-async-dependency.md` | Created | ADR for sync trait design |

### Quality Gate Results

| Gate | Command | Result |
|---|---|---|
| Compilation | `cargo check` | ✅ Clean |
| Tests | `cargo test` | ✅ 570 tests, 0 failures |
| Lint | `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ Clean (2 violations fixed: ptr_arg, unnecessary_map_or) |
| Format | `cargo fmt --all -- --check` | ✅ Clean (auto-fixed via cargo fmt --all) |

---

## Important Discoveries

### check_diff Workspace Root Mismatch

The agent-intercom MCP server's `check_diff` tool fails with a path error:
```
failed to read file for patching \\?\D:\Source\GitHub\agent-intercom_defaultworkspaceroot\...
```
The server workspace root is configured as `agent-intercom_defaultworkspaceroot` but the
actual workspace is `agent-intercom`. All operator approvals were obtained via `check_clearance`;
files were applied directly due to this server-side path mismatch.

**Action required**: The `workspace_root` in agent-intercom's config.toml or session
initialization needs to be corrected to point to `D:\Source\GitHub\agent-intercom`.

### AuditLogger Sync Design (ADR-0013)

`AuditLogger` was implemented as a synchronous trait to avoid the `async-trait` crate
dependency. `JsonlAuditWriter` uses `std::sync::Mutex<Option<WriterState>>` for
thread-safe access. Async callers should use `tokio::task::spawn_blocking` when
performance demands it.

### Clippy Pedantic Fixes Applied

Two pedantic violations were caught and fixed in `writer.rs`:
1. `clippy::ptr_arg` — used `&Path` instead of `&PathBuf` in `open_for_date`
2. `clippy::unnecessary_map_or` — replaced `.map_or(true, ...)` with `.is_none_or(...)`

### Rustfmt Condensing

`rustfmt` with `max_width = 100` condensed some multi-line closures in `writer.rs`
and one function signature in `inbox.rs` to single lines. These are cosmetic but
important to run `cargo fmt --all` before any commit.

---

## Data Model Summary

### steering_message table
- id TEXT PK (prefixed `steer:`)
- session_id TEXT NOT NULL
- channel_id TEXT (nullable)
- message TEXT NOT NULL
- source TEXT CHECK IN ('slack','ipc')
- created_at TEXT NOT NULL
- consumed INTEGER DEFAULT 0
- Index: idx_steering_session_consumed(session_id, consumed)

### task_inbox table
- id TEXT PK (prefixed `task:`)
- channel_id TEXT (nullable, scope for delivery)
- message TEXT NOT NULL
- source TEXT CHECK IN ('slack','ipc')
- created_at TEXT NOT NULL
- consumed INTEGER DEFAULT 0
- Index: idx_inbox_channel_consumed(channel_id, consumed)

---

## Next Steps (Phase 2: Foundational)

Phase 2 must be completed before any user story work. It includes:
- T007: Create persistence/steering_repo.rs (insert, fetch_unconsumed, mark_consumed, purge)
- T008: Create persistence/inbox_repo.rs (insert, fetch_unconsumed_by_channel, mark_consumed, purge)
- T009: Add CompiledWorkspacePolicy to models/policy.rs
- T010: Update policy/loader.rs to return CompiledWorkspacePolicy
- T011: Add slack_detail_level field to GlobalConfig
- T012: Wire PolicyCache and AuditLogger into AppState
- T013: Register new repos in persistence/mod.rs

**Critical**: Phase 2 blocks all user story phases (3-12). Until it's complete, no
user story code should be written.

---

## Context to Preserve

- `src/audit/mod.rs` — re-exports `JsonlAuditWriter` from `writer` submodule
- `AuditLogger` trait requires `Send + Sync` — implementors must be safe across threads
- `SteeringMessage::new()` generates ID with prefix `steer:` + UUID
- `TaskInboxItem::new()` generates ID with prefix `task:` + UUID
- Both enum types (`SteeringSource`, `InboxSource`) use `#[serde(rename_all = "snake_case")]`
- ADR-0013 documents the sync trait design decision
