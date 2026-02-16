<!-- markdownlint-disable-file -->
# Memory: phase1-workspace-scaffold

**Created:** 2026-02-09 | **Last Updated:** 2026-02-09

## Task Overview

Implement Phase 1 (Setup) of the MCP Remote Agent Server feature spec (`specs/001-mcp-remote-agent-server/`). The project is a Rust MCP server providing remote I/O capabilities to local AI agents via Slack. Phase 1 covers tasks T001–T004: Cargo workspace creation, dependency wiring, directory skeleton, and lint/format config. Phase 2 (Foundational) and beyond remain pending.

**Success criteria**: `cargo check` passes, all module stubs compile, task items marked complete in `tasks.md`.

## Current State

Phase 1 is **complete**. All four tasks (T001–T004) are implemented, verified with `cargo check`, and marked `[x]` in `specs/001-mcp-remote-agent-server/tasks.md`.

### Files Created

* `Cargo.toml` — workspace with two `[[bin]]` targets (`monocoque-agent-rem`, `monocoque-ctl`), all dependencies, workspace-level clippy lints
* `Cargo.lock` — generated lockfile
* `rustfmt.toml` — `max_width=100`, `edition=2021`
* `src/main.rs` — server entry point placeholder
* `src/lib.rs` — `AppError` enum, `Result` alias, module re-exports for config, diff, ipc, mcp, models, orchestrator, persistence, policy, slack
* `src/config.rs` — stub
* `src/models/mod.rs` — declares approval, checkpoint, policy, prompt, session, stall submodules
* `src/models/{approval,checkpoint,policy,prompt,session,stall}.rs` — stubs
* `src/mcp/mod.rs` — declares resources, server, tools
* `src/mcp/server.rs` — stub
* `src/mcp/tools/mod.rs` — declares all 9 tool handlers
* `src/mcp/tools/{accept_diff,ask_approval,check_auto_approve,forward_prompt,heartbeat,recover_state,remote_log,set_operational_mode,wait_for_instruction}.rs` — stubs
* `src/mcp/resources/mod.rs` — declares slack_channel
* `src/mcp/resources/slack_channel.rs` — stub
* `src/slack/mod.rs` — declares blocks, client, commands, events
* `src/slack/{blocks,client,commands,events}.rs` — stubs
* `src/persistence/mod.rs` — declares approval_repo, checkpoint_repo, db, prompt_repo, session_repo
* `src/persistence/{approval_repo,checkpoint_repo,db,prompt_repo,session_repo}.rs` — stubs
* `src/orchestrator/mod.rs` — declares session_manager, spawner, stall_detector
* `src/orchestrator/{session_manager,spawner,stall_detector}.rs` — stubs
* `src/policy/mod.rs` — declares evaluator, watcher
* `src/policy/{evaluator,watcher}.rs` — stubs
* `src/diff/mod.rs` — declares applicator
* `src/diff/applicator.rs` — stub
* `src/ipc/mod.rs` — declares socket
* `src/ipc/socket.rs` — stub
* `ctl/main.rs` — CLI binary placeholder
* `tests/contract/`, `tests/integration/`, `tests/unit/` — empty directories

### Files Modified

* `.gitignore` — added patterns for `release`, `*.rlib`, `*.prof*`, editor/OS noise, logs, `.env*`
* `specs/001-mcp-remote-agent-server/tasks.md` — added YAML frontmatter, marked T001–T004 `[x]`

### Git State

* Branch `feat/phase1-setup` created from `001-mcp-remote-agent-server`
* One commit: `feat(build): scaffold workspace for mcp server` (52 files, +6377 lines)
* Not pushed to remote yet
* Remaining unstaged: `.github/agents/rust-engineer.agent.md` (pre-existing, not part of this work)

## Important Discoveries

* **Decisions:**
  * `slack-morphism` v2.17 used instead of spec's v1.12 — v1.x `socket-mode` feature does not exist; v2.x exposes `hyper` feature which bundles WebSocket/tokio-tungstenite transport
  * `kv-rocksdb` feature on `surrealdb` requires `libclang` (bindgen) on Windows — resolved by installing LLVM via `choco install llvm -y`
  * Workspace-level clippy lints set to `pedantic = "deny"`, `unwrap_used = "deny"`, `expect_used = "deny"`, `unsafe_code = "forbid"`

* **Failed Approaches:**
  * Initial `cargo check` failed with `slack-morphism` feature `socket-mode` not found — fixed by switching to v2.17 with `hyper` feature
  * Second `cargo check` failed with `Unable to find libclang` for `zstd-sys` (transitive dep of rocksdb) — fixed by installing LLVM

## Next Steps

1. **Phase 2 — Foundational (T005–T017, T089–T090)**: Implement core infrastructure (GlobalConfig, AppError expansion, tracing, SurrealDB schema, Session model/repo, path validation, MCP server handler, Slack client, Block Kit builders, Axum transport, server wiring)
2. Run `cargo clippy -- -D warnings` after Phase 2 to validate lint compliance
3. **Phase 3 — US1 (T018–T025)**: Remote code review and approval via Slack
4. Push branch and open PR when Phase 2+ checkpoint passes

## Context to Preserve

* **Sources:** crates.io API: `slack-morphism` v2.17 features = `{axum, default, hyper, signature-verifier}`
* **Sources:** spec: `specs/001-mcp-remote-agent-server/spec.md`, plan: `plan.md`, data model: `data-model.md`, contracts: `contracts/mcp-tools.json`, `contracts/mcp-resources.json`
* **Questions:** Should `Cargo.lock` be committed for binary crate? (Currently committed — correct for binary projects per Cargo guidance)
* **Questions:** `lib/hve-core` submodule shows as modified — may need submodule update before PR
