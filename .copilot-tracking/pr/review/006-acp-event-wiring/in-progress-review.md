<!-- markdownlint-disable-file -->
# PR Review Status: 006-acp-event-wiring

## Review Status

* Phase: 4 — Complete
* Last Updated: 2026-03-08 (auto)
* Summary: ACP event consumer wired to Slack approval pipeline — 3 findings; 2 applied, 1 deferred

## Branch and Metadata

* Normalized Branch: `006-acp-event-wiring`
* Source Branch: `006-acp-event-wiring`
* Base Branch: `main`
* Linked Work Items: Spec at `specs/006-acp-event-wiring/spec.md`
* Total Commits Ahead: 15 (8 feature, 7 docs/tracking)

## Quality Gates (Pre-Review Verification)

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all -- --check` | ✅ Clean |
| Lint | `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ 0 warnings |
| Tests | `cargo test` | ✅ 996 passed, 0 failed |
| Working tree | `git status` | ✅ Clean |

## Diff Mapping — Production Files Only

| File | Type | New Lines | Old Lines | Focus Area |
|------|------|-----------|-----------|------------|
| `src/slack/blocks.rs` | Modified | 385–525 | — | Shared block builders added (D1) |
| `src/main.rs` | Modified | 965–1131 | — | `handle_clearance_requested` handler |
| `src/main.rs` | Modified | 1133–1256 | — | `handle_prompt_forwarded` handler |
| `src/main.rs` | Modified | 770–826 | — | Match arm dispatch wiring |
| `src/mcp/tools/ask_approval.rs` | Modified | 1–22 | — | Import refactor to use shared blocks |
| `src/mcp/tools/forward_prompt.rs` | Modified | 1–22 | — | Import refactor to use shared blocks |
| `src/mcp/tools/util.rs` | Modified | 1–32 | — | `truncate_text` re-exported from blocks |
| `src/models/approval.rs` | Modified | 96–109 | — | `parse_risk_level` added |
| `src/models/prompt.rs` | Modified | 59–74 | — | `parse_prompt_type` added |
| `src/persistence/approval_repo.rs` | Modified | 204–220 | — | `update_slack_ts` added |

## Instruction Files Reviewed

* `.github/instructions/constitution.instructions.md` (`applyTo: "**"`): Core principles apply — Safety-First Rust, Test-First Development, Security Boundary Enforcement, Structured Observability

## Phase 2 Analysis Summary

### Phase 2 Actions Performed

1. Generated full diff against `main` — 871 lines of diff across 8 production files
2. Read all production source files in full — `blocks.rs` (526 lines), `main.rs` (handlers ~291 lines), `approval_repo.rs`, `models/approval.rs`, `models/prompt.rs`, `util.rs`
3. Traced `AgentEvent::ClearanceRequested` through `acp/reader.rs` → `driver/mod.rs` → `main.rs` → `blocks.rs` → Slack
4. Cross-referenced MCP `ask_approval.rs` path vs. ACP `handle_clearance_requested` path for behavioral equivalence
5. Matched constitution requirements: no `unwrap`/`expect`, pedantic clippy clean, doc comments, parameterized SQL

### Key Behaviors Verified ✅

* `approval.id = request_id.to_owned()` — ACP JSON-RPC correlation correct
* `prompt.id = prompt_id.to_owned()` — ACP JSON-RPC correlation correct
* `set_thread_ts` uses `WHERE thread_ts IS NULL` — idempotent first-write semantics
* D2 conditional posting: `post_message_direct` when `thread_ts=None`, `enqueue` when Some
* SC-003: DB persist before driver registration; early return on persist failure
* `parse_risk_level` / `parse_prompt_type`: no `match_same_arms` violations
* `update_slack_ts`: parameterized SQL, `Result<()>` return

### High-Risk Areas Identified

1. **MEDIUM** — "Diff uploaded as file" message displayed in ACP handler without actual file upload
2. **LOW** — Empty description (`""`) always wrapped as `Some("")` instead of `None`  
3. **LOW** — Agent-supplied `title`/`description` embedded unescaped in Slack mrkdwn

## Review Items

### ✅ Approved / Applied

#### RI-001: "Diff uploaded as file" message shown without actual upload in ACP handler

* File: `src/slack/blocks.rs` lines 424–431 / `src/main.rs` (handle_clearance_requested)
* Category: Reliability / Correctness
* Severity: Medium
* **Decision**: Applied — Option A: added `slack.upload_file(...)` call in ACP handler when diff ≥ `INLINE_DIFF_THRESHOLD` (20 lines), mirroring MCP `ask_approval` behavior.
* **Outcome**: `cargo check` ✅, `cargo clippy` ✅, `cargo test` ✅ (996 passed)

---

#### RI-002: Empty description always passed as `Some("")` to block builder

* File: `src/main.rs` (handle_clearance_requested)
* Category: Code Quality
* Severity: Low
* **Decision**: Applied — added `let description_opt = if description.is_empty() { None } else { Some(description) };` before `build_approval_blocks` call.
* **Outcome**: Passes all gates; `cargo fmt --all` applied for line-length compliance.

---

### ❌ Deferred / No Action

#### RI-003: Agent-supplied title/description embedded unescaped in Slack mrkdwn

* File: `src/slack/blocks.rs` lines 416–418
* Category: Security (defense-in-depth)
* Severity: Low
* **Decision**: Deferred — ACP agents are trusted operator-controlled processes; exploitation risk is low. Documented as a follow-up recommendation. No code change applied in this PR.
* **Rationale**: Adding escaping would require a shared `slack_escape()` utility and touches the MCP path as well. Out of scope for this feature branch.

---

## Next Steps

* [x] RI-001 Applied
* [x] RI-002 Applied
* [x] RI-003 Deferred
* [x] All gates pass (fmt ✅, clippy ✅, 996 tests ✅)
* [x] Generate handoff.md
* [x] Commit and push
