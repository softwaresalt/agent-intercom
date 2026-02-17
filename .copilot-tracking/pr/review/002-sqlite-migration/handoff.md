<!-- markdownlint-disable-file -->
# PR Review Handoff: 002-sqlite-migration

## PR Overview

Replaces SurrealDB with SQLite (via sqlx 0.8) across the entire persistence layer while preserving the existing repository API surface, domain models, and test coverage. All 5 repository modules, schema bootstrap, retention purge, and test tiers were rewritten to use SQLite with WAL mode and a single-writer connection pool.

* Branch: `002-sqlite-migration`
* Base Branch: `main`
* Commits: 8 (b2b9a58..fcd5aec)
* Total Files Changed: 88 (+4699, -3495)
* Total Review Comments: 2 (both accepted and fixed)

## CI Gate Results (Post-Fix)

| Gate | Command | Status |
|------|---------|--------|
| fmt | `cargo fmt --all -- --check` | ✅ Pass |
| clippy | `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ Pass |
| test | `cargo test --all-targets` | ✅ Pass (339 tests: 17 inline + 138 contract + 45 integration + 139 unit) |
| audit | `cargo audit` | ✅ Pass (414 crates, 0 vulnerabilities) |

## PR Comments Ready for Submission

### File: src/models/session.rs + src/persistence/session_repo.rs

#### Comment 1 — RI-001: Session state transition inconsistency (Lines session.rs 98-113, session_repo.rs 132-145)

* Category: Functional Correctness
* Severity: Medium
* Status: ✅ Fixed in fcd5aec

The model's `can_transition_to()` and the repository's `is_valid_transition()` defined different valid transitions. The model allowed `Paused → Interrupted` but not `Interrupted → Active`; the repo allowed `Interrupted → Active` but not `Paused → Interrupted`.

Both are needed:
- `Interrupted → Active`: Crash recovery re-activates interrupted sessions
- `Paused → Interrupted`: Shutdown handler marks paused sessions as interrupted (main.rs L211)

**Resolution**: Unified both to include all valid paths:
- `Created | Paused | Interrupted → Active`
- `Active → Paused | Interrupted | Terminated`
- `Paused → Terminated | Interrupted`

### File: src/persistence/schema.rs

#### Comment 2 — RI-002: Missing indexes on session_id columns (Lines 18-96)

* Category: Performance
* Severity: Low
* Status: ✅ Fixed in fcd5aec

All four child tables (`approval_request`, `checkpoint`, `continuation_prompt`, `stall_alert`) are queried by `session_id` in repository methods and retention purge, but no indexes existed beyond primary keys.

**Resolution**: Added four indexes:

```sql
CREATE INDEX IF NOT EXISTS idx_approval_session ON approval_request(session_id);
CREATE INDEX IF NOT EXISTS idx_checkpoint_session ON checkpoint(session_id);
CREATE INDEX IF NOT EXISTS idx_prompt_session ON continuation_prompt(session_id);
CREATE INDEX IF NOT EXISTS idx_stall_session ON stall_alert(session_id);
```

## Review Summary by Category

* Security Issues: 0
* Functional Correctness: 1 (RI-001 — fixed)
* Performance: 1 (RI-002 — fixed)
* Convention Violations: 0
* Documentation: 0

## Instruction Compliance

* ✅ `.github/copilot-instructions.md`: All rules followed — no `unwrap()`/`expect()`, `pub(crate)` visibility, `AppError::Db` for all fallible DB operations, pedantic clippy clean, doc comments on public items
* ✅ `002-sqlite-migration/spec.md`: All 20 functional requirements satisfied
* ✅ `002-sqlite-migration/contracts/schema.sql.md`: Schema matches contract (with index additions)
* ✅ `002-sqlite-migration/contracts/repository-api.md`: API surface preserved

## Success Criteria Verified

| Criterion | Status |
|-----------|--------|
| SC-001 All tests pass | ✅ 339/339 |
| SC-002 SurrealDB absent from Cargo.toml/Cargo.lock | ✅ |
| SC-003 Auto schema bootstrap on startup | ✅ |
| SC-006 In-memory CRUD tests | ✅ |
| SC-007 Retention purge tested | ✅ |
| SC-008 No surrealdb references in source | ✅ |

## Outstanding Notes

* The `.github/agents/speckit.tasks.agent.md` file has an unrelated local modification (not staged or committed with this PR).
* Constitution Principle VI (Single-Binary Simplicity) may need a post-merge amendment to reference SQLite instead of SurrealDB, per the spec's migration plan.
