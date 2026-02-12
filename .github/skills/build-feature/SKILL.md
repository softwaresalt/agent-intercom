---
name: build-feature
description: "Usage: Build feature {spec-name} phase {phase-number}. Implements a single phase from the spec's task plan, iterating build-test cycles until the phase passes its constitution gate, then records memory, logs decisions, and commits."
version: 1.0
maturity: stable
input:
  properties:
    spec-name:
      type: string
      description: "Directory name of the feature spec under specs/ (e.g., 001-mcp-remote-agent-server)."
    phase-number:
      type: integer
      description: "Phase number to build from the spec's tasks.md (e.g., 6 for Phase 6)."
  required:
    - spec-name
    - phase-number
---

# Build Feature Skill

Implements a single phase from a feature specification's task plan. The workflow iterates through build-test cycles until the phase satisfies its constitution gate, then records session memory, logs architectural decisions, and commits all changes.

## Prerequisites

* A feature spec directory exists at `specs/${input:spec-name}/` containing `plan.md`, `spec.md`, and `tasks.md`
* The target phase exists in `tasks.md` with defined tasks
* The project compiles before starting (`cargo check` passes)
* The `.github/copilot-instructions.md` constitution and coding standards are accessible

## Quick Start

Invoke the skill with both required parameters:

```text
Build feature 001-mcp-remote-agent-server phase <phase-number>
```

The skill runs autonomously through all required steps, halting only on unrecoverable errors or constitution violations requiring human judgment.

## Parameters Reference

| Parameter      | Required | Type    | Description                                                        |
| -------------- | -------- | ------- | ------------------------------------------------------------------ |
| `spec-name`    | Yes      | string  | Directory name under `specs/` containing the feature specification |
| `phase-number` | Yes      | integer | Phase number from `tasks.md` to implement                          |

## Required Steps

### Step 1: Load Phase Context

* Read `specs/${input:spec-name}/tasks.md` and extract all tasks for the specified phase.
* Read `specs/${input:spec-name}/plan.md` for architecture, tech stack, and project structure.
* Read `specs/${input:spec-name}/spec.md` for user stories and acceptance scenarios relevant to this phase.
* Read `specs/${input:spec-name}/data-model.md` if it exists, for entity definitions and relationships.
* Read `specs/${input:spec-name}/contracts/` if it exists, for MCP tool JSON-RPC specifications and error contracts.
* Read `specs/${input:spec-name}/research.md` if it exists, for technical decisions and constraints.
* Read `.github/copilot-instructions.md` for the project constitution, coding standards, and session memory requirements.
* Read `.github/agents/rust-eng.implement.agent.md` for language-specific engineering standards.
* Build a task execution list respecting dependencies: sequential tasks run in order, tasks marked `[P]` can run in parallel.
* Identify which tasks are tests and which are implementation; TDD order means test tasks execute before their corresponding implementation tasks.
* Report a summary of the phase scope: task count, estimated files affected, and user story coverage.

### Step 2: Check Constitution Gate

* Read `specs/${input:spec-name}/plan.md` and locate the Constitution Check table.
* Verify every principle listed in the table is satisfied for the work about to begin.
* If `specs/${input:spec-name}/checklists/` exists, scan all checklist files and verify completion status.
* If any constitution principle is violated or any required checklist is incomplete, halt and report the violation with actionable remediation steps.
* If all gates pass, proceed to Step 3.

### Step 3: Build Phase (Iterative)

Execute tasks in dependency order following TDD discipline:

1. For each task group (tests first, then implementation):
   * Classify the task type to determine which coding constraints apply:
     * Persistence tasks (touching `persistence/`, `models/`, schema): apply Database and Error Handling constraints from the Coding Standards section below.
     * MCP tasks (touching `mcp/server.rs`, `mcp/tools/`, `mcp/resources/`): apply MCP Tools and Error Handling constraints.
     * Slack tasks (touching `slack/`): apply Slack and Error Handling constraints.
     * Orchestrator tasks (touching `orchestrator/`): apply Async and Error Handling constraints.
     * Diff/Policy tasks (touching `diff/`, `policy/`): apply General Rust and Error Handling constraints.
     * IPC tasks (touching `ipc/`): apply General Rust and Error Handling constraints.
   * Read any existing source files that the task modifies.
   * Implement the task following the coding standards from the rust-eng.implement agent, injecting only the task-type-specific constraints identified above.
   * After implementing each task, run `cargo check` to verify compilation.
   * If compilation fails, diagnose the error, fix it, and re-run `cargo check` until it passes.
   * Mark the completed task as `[X]` in `specs/${input:spec-name}/tasks.md`.

2. Follow these implementation rules:
   * Setup tasks first (project structure, dependencies, configuration).
   * Test tasks before their corresponding implementation tasks (TDD).
   * Respect `[P]` markers: parallel tasks touching different files can be implemented together.
   * Sequential tasks (no `[P]` marker) must complete in listed order.
   * Tasks affecting the same files must run sequentially regardless of markers.

3. Track architectural decisions made during implementation for recording in Step 6.

### Step 4: Test Phase (Iterative)

Run the full test suite and iterate until all tests pass:

1. Run `cargo test` to execute all test suites.
2. If any test fails:
   * Diagnose the failure from the test output.
   * Fix the implementation (not the test, unless the test itself has a bug).
   * Re-run `cargo test` to verify the fix.
   * Repeat until all tests pass.
3. Run `cargo clippy -- -D warnings -D clippy::pedantic` to verify lint compliance.
4. If clippy reports warnings or errors, fix them and re-run until clean.
5. Run `cargo fmt --all -- --check` to verify formatting.
6. If formatting violations exist, run `cargo fmt --all` and verify.
7. Report final test results: suite counts, pass rates, and any notable findings.

Return to Step 3 if test failures reveal missing implementation work. Continue iterating between Step 3 and Step 4 until both build and test pass cleanly.

### Step 5: Constitution Validation

Re-check the constitution after implementation is complete:

* Verify `#![forbid(unsafe_code)]` is maintained; no `unsafe` blocks introduced.
* Verify no `unwrap()` or `expect()` calls exist in library code paths (workspace lint `clippy::unwrap_used = "deny"` and `clippy::expect_used = "deny"`).
* Verify all new public items have `///` doc comments.
* Verify error handling uses `AppError` variants from `src/errors.rs` with descriptive lowercase messages.
* Verify any new async code follows tokio patterns (no mutex guards held across `.await` points, `CancellationToken` for shutdown coordination).
* Verify all Slack message posting routes through the rate-limited message queue.
* Verify all file path operations validate against the workspace root via `starts_with()`.
* If any violation is found, return to Step 3 to remediate before proceeding.

### Step 6: Record Architectural Decisions

For each significant decision made during the build phase:

* Create an ADR file in `docs/adrs/` following the naming convention `NNNN-{short-title}.md` where `NNNN` is the next sequential number (zero-padded to 4 digits).
* Each ADR includes:
  * Title describing the decision
  * Status (Accepted)
  * Context explaining the problem or situation
  * Decision made and rationale
  * Consequences (positive, negative, and risks)
  * Date and the phase/task that prompted the decision
* Decisions worth recording include: dependency choices, MCP tool design trade-offs, data model changes, SurrealDB workarounds, Slack interaction patterns, rmcp SDK patterns, error handling strategies, and performance trade-offs.
* Skip this step if no significant architectural decisions were made during the phase.

### Step 7: Record Session Memory

Persist the full session details to `.copilot-tracking/memory/` following the project's session memory requirements:

* Create a memory file at `.copilot-tracking/memory/{YYYY-MM-DD}/{spec-name}-phase-{N}-memory.md` where the date is today and N is the phase number.
* The memory file includes:
  * Task Overview: phase scope and objectives
  * Current State: all tasks completed, files modified, test results
  * Important Discoveries: decisions made, failed approaches, SurrealDB or rmcp or slack-morphism quirks encountered
  * Next Steps: what the next phase should address, any open questions, known issues
  * Context to Preserve: source file references, agent references, unresolved questions
* Use the existing memory files in `.copilot-tracking/memory/` as format examples.

### Step 8: Pre-Commit Review

1. Review all changes made during the phase to ensure they align with the completed tasks and constitution.
2. Review the ADRs created in Step 6 for clarity and completeness.
3. Review all steps to ensure that no steps have been missed and address any missing steps in the sequence before proceeding.
4. Review the session memory file for completeness and accuracy.

### Step 9: Stage, Commit, and Sync

Finalize all changes with a Git commit:

1. Accept all current diff changes (no interactive review).
2. Run `git add -A` to stage all modified, created, and deleted files.
3. Compose a commit message following these conventions:
   * Format: `feat({spec-name}): complete phase {N} - {phase title}`
   * Body: list of completed task IDs and a brief summary of what was built
   * Footer: reference the spec path and any relevant ADR numbers
4. Run `git commit` with the composed message.
5. Run `git push` to sync the commit to the remote repository.
6. Report the commit hash and a summary of changes committed.

## Troubleshooting

### SurrealDB embedded behavioral differences

Refer to session memory at `.copilot-tracking/memory/` for documented workarounds including `Thing` deserialization via `*Row` structs, `SCHEMAFULL` table DDL patterns, and record ID serialization from `surrealdb::sql::Thing` to `String`.

### Tests pass locally but fail in CI

Verify `rust-toolchain.toml` matches the CI configuration in `.github/workflows/ci.yml`. Check that the `[[test]]` entries in `Cargo.toml` include all external test files.

### Constitution violation detected

Return to Step 3 and fix the violation before proceeding. Common violations include `unwrap()` usage in library code, missing doc comments on public items, and `unsafe` blocks.

### Slack Socket Mode reconnection issues

If Slack interactions fail after reconnect, verify the `hello` event handler in `slack/client.rs` re-posts pending approval requests and continuation prompts per ADR-0011.

## Coding Standards

These rules are injected into each task based on its type classification in Step 3.

### General Rust

* `#![forbid(unsafe_code)]`
* Prefer borrowing over cloning
* Default to `pub(crate)`
* All public items require `///` doc comments

### Error Handling

* Use the `AppError` enum from `src/errors.rs` for all domain errors
* Variants: `Config`, `Db`, `Slack`, `Mcp`, `Diff`, `Policy`, `Ipc`, `PathViolation`, `PatchConflict`, `NotFound`, `Unauthorized`, `AlreadyConsumed`
* Map external errors via `From` impls or explicit `.map_err()`
* Error messages are lowercase and do not end with a period

### Database (SurrealDB)

* All DB access goes through `persistence/` repository modules (`approval_repo`, `session_repo`, `checkpoint_repo`, `prompt_repo`)
* No raw SurrealDB queries outside `persistence/`
* Use `kv-rocksdb` for production, `kv-mem` for tests
* Schema bootstrap uses `SCHEMAFULL` tables with idempotent DDL

### MCP Tools (rmcp)

* Tools follow the handler flow: validate session → parse params → execute domain logic → update session `last_tool`/`updated_at` → return JSON
* All 9 tools are always registered and visible; inapplicable calls return descriptive errors
* Blocking tools (`ask_approval`, `forward_prompt`, `wait_for_instruction`) use `tokio::sync::oneshot` channels
* Tool definitions use `#[tool]`/`#[tool_router]` macros from `rmcp` 0.5

### Slack (slack-morphism)

* Socket Mode with outbound-only WebSocket (no inbound firewall ports)
* All message posting routes through the rate-limited in-memory queue with exponential backoff
* Respect `Retry-After` headers from the Slack API
* Use Block Kit builders from `slack/blocks.rs` for all Slack messages
* Centralized authorization guard in `slack/events.rs` dispatcher — unauthorized users silently ignored
* Double-submission prevention via `chat.update` replacing buttons before handler dispatch

### Async (tokio)

* Target `tokio` 1 with the `full` feature set
* Use `tokio::task::spawn_blocking` for CPU-bound or blocking I/O (e.g., `keyring` credential lookups)
* Drop `MutexGuard`/`RwLockGuard` before `.await` points
* Use `tokio_util::sync::CancellationToken` for graceful shutdown coordination
* Use `tokio::process::Command` with `kill_on_drop(true)` for agent session processes

### Architecture Awareness

| Concern           | Approach                                                                                     |
| ----------------- | -------------------------------------------------------------------------------------------- |
| MCP SDK           | `rmcp` 0.5 — `ServerHandler` trait, `#[tool]`/`#[tool_router]` macros                       |
| Transport (stdio) | stdio via `rmcp` for direct agent connections                                                |
| Transport (HTTP)  | axum 0.8 with `StreamableHttpService` on `/mcp` for HTTP/SSE sessions                       |
| Slack             | `slack-morphism` Socket Mode                                                                 |
| Database          | SurrealDB embedded — `kv-rocksdb` production, `kv-mem` tests, `SCHEMAFULL` tables            |
| Configuration     | TOML (`config.toml`) → `GlobalConfig`, credentials via keyring with env fallback             |
| Workspace policy  | JSON auto-approve rules (`.monocoque/settings.json`), hot-reloaded via `notify` file watcher |
| Diff safety       | `diffy` 0.4 for unified diff parsing, `sha2` for integrity hashing, atomic writes via tempfile |
| Path security     | All paths canonicalized and validated via `starts_with(workspace_root)`                       |
| IPC               | `interprocess` crate — named pipes (Windows) / Unix domain sockets for `monocoque-ctl`       |
| Shutdown          | `CancellationToken` — persist state, notify Slack, terminate children gracefully              |

---

Proceed with the user's request by executing the Required Steps in order for the specified `spec-name` and `phase-number`.
