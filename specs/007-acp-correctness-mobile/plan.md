# Implementation Plan: ACP Correctness Fixes and Mobile Operator Accessibility

**Branch**: `007-acp-correctness-mobile` | **Date**: 2026-03-08 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/007-acp-correctness-mobile/spec.md`

## Summary

Fix four targeted ACP correctness issues identified during adversarial review (steering
message consumption, session capacity enforcement, MCP query parameter cleanup, prompt
correlation ID collisions) and conduct desk research on Slack modal behavior on iOS to
determine whether a thread-reply input fallback is needed for mobile operators. The
correctness fixes are independent surgical changes to existing modules; the mobile track
is research-first with conditional implementation gated on findings.

## Technical Context

**Language/Version**: Rust stable, edition 2021
**Primary Dependencies**: rmcp 0.13 (MCP), slack-morphism 2.17 (Slack), sqlx 0.8 (SQLite), tokio 1.37 (async), uuid 1.7 (ID generation)
**Storage**: SQLite via sqlx (file-based prod, in-memory tests)
**Testing**: `cargo test` (TDD — tests first, then implementation). Three tiers: unit, contract, integration
**Target Platform**: Windows workstation (primary), macOS/Linux (secondary)
**Project Type**: Single Rust workspace, two binaries (`agent-intercom`, `agent-intercom-ctl`)
**Performance Goals**: N/A — correctness-focused feature, no new performance-sensitive paths
**Constraints**: clippy pedantic deny, no `unwrap()`/`expect()`, `#![forbid(unsafe_code)]`
**Scale/Scope**: 4 independent code fixes + 1 research task + 2 conditional implementation tasks

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Safety-First Rust | ✅ Pass | All changes use `Result`/`AppError`, no unsafe code, uuid crate already in workspace |
| II. MCP Protocol Fidelity | ✅ Pass | No MCP tool surface changes; F-10 removes a deprecated query param but does not alter the MCP protocol layer |
| III. Test-First Development | ✅ Pass | Each fix has tests written before implementation; test tiers maintained |
| IV. Security Boundary Enforcement | ✅ Pass | No path, credential, or authorization changes |
| V. Structured Observability | ✅ Pass | F-10 removes a deprecation warning path; no degradation of tracing coverage |
| VI. Single-Binary Simplicity | ✅ Pass | No new dependencies — uuid already in Cargo.toml |
| VII. CLI Workspace Containment | ✅ Pass | No filesystem operation changes |
| VIII. Destructive Command Approval | ✅ Pass | Not applicable to this feature |

## Project Structure

### Documentation (this feature)

```text
specs/007-acp-correctness-mobile/
├── spec.md              # Feature specification (complete)
├── plan.md              # This file
├── research.md          # Phase 0: mobile modal research + codebase analysis
├── data-model.md        # Phase 1: affected entity models and state transitions
├── SCENARIOS.md         # Behavioral matrix (speckit.behavior output)
├── tasks.md             # Phased task breakdown (speckit.tasks output)
└── checklists/
    └── requirements.md  # Specification quality checklist (complete)
```

### Source Code (repository root)

```text
src/
├── acp/
│   ├── reader.rs         # F-06: steering message delivery-before-consume fix
│   └── handshake.rs      # F-13: UUID-based correlation IDs for handshake
├── driver/
│   └── acp_driver.rs     # F-13: UUID-based prompt correlation IDs (replace PROMPT_COUNTER)
├── mcp/
│   └── sse.rs            # F-10: remove channel_id query parameter, workspace_id only
├── persistence/
│   └── session_repo.rs   # F-07: add count_active_acp() method
├── slack/
│   ├── commands.rs        # F-07: use count_active_acp() in ACP session start
│   ├── handlers/
│   │   ├── thread_reply.rs  # F-16 (conditional): thread-reply input fallback handler
│   │   ├── prompt.rs        # F-16 (conditional): modal fallback path
│   │   ├── wait.rs          # F-16 (conditional): modal fallback path
│   │   └── approval.rs      # F-16 (conditional): modal fallback path
│   └── blocks.rs            # F-16 (conditional): thread-reply prompt builder
└── config.rs               # F-10: update resolve_channel_id signature

tests/
├── unit/
│   ├── acp_reader_steering_*.rs    # F-06 steering retry tests
│   ├── session_repo_count_acp.rs   # F-07 capacity query tests
│   ├── sse_workspace_only.rs       # F-10 query param tests
│   └── correlation_id_*.rs         # F-13 uniqueness tests
├── contract/
│   ├── acp_capacity_contract.rs    # F-07 contract tests
│   └── mcp_no_channel_id.rs        # F-10 contract tests
└── integration/
    └── channel_override_tests.rs   # F-10: update for workspace_id-only routing
```

**Structure Decision**: Single Rust workspace. All changes target existing modules — no new
top-level directories. The conditional F-16 thread-reply handler is the only potential new
file (`src/slack/handlers/thread_reply.rs`), created only if mobile research warrants it.

## Complexity Tracking

No constitution violations. All changes are surgical fixes within existing module boundaries.

## Phase 0: Research

### R-01: Codebase Impact Analysis

**Findings**:

| Fix ID | File | Current Behavior | Root Cause |
|---|---|---|---|
| F-06 | `src/acp/reader.rs:455-464` | `mark_consumed()` called unconditionally after `send_prompt()`, even on error | Missing error-gate before consumption |
| F-07 | `src/slack/commands.rs:484` | `count_active()` queries `WHERE status = 'active'` only | Query excludes `created` state and doesn't filter by protocol |
| F-10 | `src/mcp/sse.rs:123-139, 468-513` | `channel_id` extracted from URL and used as fallback when `workspace_id` absent | Legacy backward-compat path; project fully migrated to `workspace_id` |
| F-13 | `src/acp/handshake.rs:46` + `src/driver/acp_driver.rs:54` | Static `"intercom-prompt-1"` collides with `PROMPT_COUNTER` starting at 1 | Counter-based IDs not unique across sessions/restarts |

**Pre-flight exclusions**:
- F-08 (workspace resolution): Already fixed — `commands.rs:498-506` uses `state.workspace_mappings.read()` with T154 hot-reload lock
- F-09 (deregister_session leak): Fixed in commit `b402824` — `deregister_session` now cleans up `pending_clearances` and `pending_prompts_acp`

### R-02: Mobile Modal Research (F-15)

**Decision**: Desk research only; HITL testing post-build.

**Research questions**:
1. Do Slack `views.open` / `views.push` API calls render modals on Slack iOS?
2. Does `plain_text_input` element accept input on iOS?
3. Are there known community-reported issues with modal input on Slack mobile?

**Findings**: To be documented in `research-f15-mobile-modals.md` during Phase 5.

### R-03: UUID Strategy for Correlation IDs

**Decision**: Use `Uuid::new_v4()` for all correlation IDs.
**Rationale**: Eliminates collision risk across sessions and server restarts without shared state. The `uuid` crate is already a workspace dependency (v1.7).
**Alternatives considered**:
- Start counter at 1000: Simpler but still has theoretical collision risk across restarts.
- Use `{session_id}-{counter}`: Session-scoped but verbose and still counter-based.

### R-04: `channel_id` Removal Scope

**Decision**: Remove `channel_id` query parameter from `/mcp` endpoint entirely.
**Rationale**: Project fully migrated to `workspace_id`-based routing. No external consumers use `channel_id`.
**Scope**:
- Remove: URL query param extraction, legacy fallback code path, `PendingParams` channel_id slot
- Keep: `[slack] channel_id` in config.toml (default channel — different concern)
- Keep: `IntercomServer::with_channel_override()` internal API (receives channel resolved from workspace mapping)
- Update: `resolve_channel_id()` in config.rs to drop `channel_id` param
- Update: Tests referencing bare `channel_id` query param behavior

## Phase 1: Design

### Affected Entities

See `data-model.md` for entity details.

### API Contracts

No new API contracts — all changes are internal to existing modules. The MCP tool surface
is unchanged. The only externally visible change is the removal of the `?channel_id=` query
parameter on the `/mcp` HTTP endpoint (F-10).

### Dependency Map

```text
F-06 (reader.rs) ────────────────┐
F-07 (session_repo + commands) ──┤  Independent — any order
F-10 (sse.rs + config.rs) ───────┤
F-13 (handshake + acp_driver) ───┘
F-15 (research) ──► F-16 (conditional) ──► F-17 (conditional)
```
