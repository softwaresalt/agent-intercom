# Phase 8 Session Memory — Workspace Auto-Approve Policy

**Feature**: 001-mcp-remote-agent-server
**Phase**: 8 (User Story 6 — Workspace Auto-Approve Policy)
**Date**: 2026-02-11

## Task Overview

Phase 8 implements workspace-level auto-approve policies that let operators pre-authorize
safe operations (running tests, linting, reading files) to bypass the Slack approval gate.
This reduces notification noise and improves agent throughput for trusted operations.

9 tasks total: 3 test tasks (T116-T118) + 6 implementation tasks (T061-T066).

## Current State

All 9 tasks completed successfully. All toolchain gates pass:

| Gate | Status |
|------|--------|
| `cargo check` | PASS |
| `cargo clippy -- -D warnings -D clippy::pedantic` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo test` | PASS — 176 tests (77 contract + 25 integration + 74 unit) |

### Files Created

| File | Task | Purpose |
|------|------|---------|
| `src/policy/loader.rs` | T061 | Load `.monocoque/settings.json`, validate commands against global allowlist (FR-011), deny-all fallback |
| `tests/unit/policy_tests.rs` | T116 | 8 unit tests for policy loader: valid/malformed/empty/missing file, allowlist filtering |
| `tests/unit/policy_evaluator_tests.rs` | T117 | 16 unit tests for evaluator: command/tool/file-pattern matching, risk threshold, disabled policy |
| `tests/contract/check_auto_approve_tests.rs` | T118 | 9 contract tests validating input/output schemas per mcp-tools.json |

### Files Modified

| File | Task | Change |
|------|------|--------|
| `src/policy/mod.rs` | T065 | Added `pub mod loader;` and module-level doc comment |
| `src/policy/evaluator.rs` | T062, T066 | Replaced placeholder with full evaluator: command/tool/file-pattern matching with risk gate and tracing spans |
| `src/policy/watcher.rs` | T063 | Replaced placeholder with `notify`-based hot-reload watcher with `PolicyCache` and register/unregister lifecycle |
| `src/mcp/tools/check_auto_approve.rs` | T064, T066 | Replaced placeholder with MCP tool handler: resolves session, loads policy, evaluates, returns `{auto_approved, matched_rule}` |
| `src/mcp/handler.rs` | T064 | Wired `check_auto_approve` into tool router (was falling through to "not implemented") |
| `tests/unit.rs` | T116-T117 | Registered `policy_tests` and `policy_evaluator_tests` modules |
| `tests/contract.rs` | T118 | Registered `check_auto_approve_tests` module |
| `Cargo.toml` | T062 | Added `glob = "0.3"` dependency for file pattern matching |

## Important Discoveries

### glob Crate for File Pattern Matching

The `glob` crate (0.3) was added for evaluating file pattern rules in `WorkspacePolicy.file_patterns`.
The `glob::Pattern` type provides simple glob matching without requiring full path expansion. This is
sufficient for matching relative file paths against patterns like `src/**/*.rs`.

### Policy Evaluation Order

The evaluator checks rules in this priority order:
1. Disabled policy → deny all
2. Risk level gate → deny if exceeds threshold (critical always denied)
3. Command matching (must also be in global allowlist per FR-011)
4. Tool matching
5. File pattern matching (write patterns for write-type tools, read for read-type)
6. No match → deny

### Deny-All Fallback Design

`PolicyLoader::load()` never returns an error for missing or malformed policy files. It always
returns `Ok(WorkspacePolicy::default())` (which has `enabled: false`), logging a warning via
tracing. This makes policy loading non-fatal — a corrupted settings.json doesn't crash the server.

### Watcher Sync/Async Boundary

The `notify` crate's callback runs in a synchronous context. The `PolicyCache` uses `tokio::sync::RwLock`
which supports `blocking_write()` for use in sync contexts. This avoids the need for a channel-based
bridge between the sync watcher callback and the async runtime.

## Next Steps

- **Phase 9 (US7)**: Session orchestration — uses `PolicyWatcher` to register/unregister workspace
  watchers as sessions start and terminate.
- **Phase 14 (Polish)**: Wire `PolicyWatcher` into the server bootstrap for hot-reload lifecycle,
  potentially integrate auto-approve checks into `ask_approval` flow to skip Slack for
  pre-authorized operations.

## Context to Preserve

- `PolicyWatcher` exposes `register()` and `unregister()` methods tied to session lifecycle.
  These should be called from `SessionManager` (Phase 9) when sessions start/terminate.
- `PolicyEvaluator::check()` is a pure function with no side effects beyond tracing. It can be
  called from any context. The `check_auto_approve` MCP tool loads policy fresh each call;
  for the hot-reload watcher path, callers would read from `PolicyCache` instead.
- The `check_auto_approve` tool is now wired in the handler router alongside the 5 other
  implemented tools. Remaining placeholder tools: `recover_state`, `set_operational_mode`,
  `wait_for_instruction`.
