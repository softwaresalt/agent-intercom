---
name: build-feature
description: "Implements a requested feature by continuously looping a fast worker agent against a strict, compiling, but failing test harness until success is achieved."
version: 2.0
maturity: stable
input:
  properties:
    task-id:
      type: string
      description: "The unique Beads task ID."
    harness-cmd:
      type: string
      description: "The cargo test command defining the strict compiler harness boundary."
  required:
    - task-id
    - harness-cmd
---

# Build Feature Skill

Implements a requested feature by continuously looping a fast worker agent against a strict, compiling, but failing test harness until success is achieved. The harness defines the contract; the compiler is the critic.

## Prerequisites

* The test harness defined by `${input:harness-cmd}` compiles (green compilation, red tests)
* The structural stubs in `src/` exist with `unimplemented!()` markers
* The project compiles before starting (`cargo check` passes)

## Remote Operator Integration (agent-intercom)

When the agent-intercom MCP server is reachable, status updates and file modifications route through it so the remote operator can follow progress via Slack.

### Availability Detection

At the start of execution, call `ping` with a brief status message. If the call succeeds, agent-intercom is active — follow all remote workflow rules below. If it fails or times out, fall back to local-only operation.

### Status Broadcasting

Use `broadcast` (non-blocking) throughout execution to keep the operator informed.

| When | Tool | Level | Message Pattern |
|---|---|---|---|
| Skill start | `broadcast` | `info` | `[BUILD] Starting task {task-id}: {harness-cmd}` |
| Each iteration start | `broadcast` | `info` | `[LOOP] Attempt {N}/5 — running harness` |
| File created | `broadcast` | `info` | `[FILE] created: {file_path}` — include full content in body |
| File modified | `broadcast` | `info` | `[FILE] modified: {file_path}` — include unified diff in body |
| Harness passes | `broadcast` | `success` | `[BUILD] Harness passed on attempt {N}` |
| Harness fails | `broadcast` | `warning` | `[LOOP] Attempt {N} failed — {error_summary}` |
| Circuit breaker hit | `broadcast` | `error` | `[BUILD] Circuit breaker — 5 attempts exhausted, task blocked` |
| Workspace test pass | `broadcast` | `success` | `[BUILD] Workspace tests pass — task {task-id} complete` |
| Task complete | `broadcast` | `success` | `[BUILD] Task {task-id} complete — commit {short_hash}` |

Post the first `broadcast` as a new top-level message and capture the returned `ts`. Use that `ts` as `thread_ts` for all subsequent messages.

### File Change Workflow

File creation and modification proceed with direct writes. After each file write, call `broadcast` at `info` level with the change details.

For **destructive operations** (file deletion, directory removal), route through the approval workflow:

1. `auto_check` — Check if workspace policy allows the operation.
2. `check_clearance` — Submit proposal and block until operator responds.
3. `check_diff` — Execute only after `status: "approved"`.

## Execution Steps

### Step 1: Context Isolation

1. Read the test file targeted by `${input:harness-cmd}`. Carefully read the embedded `// GIVEN`, `// WHEN`, `// THEN` BDD comments to fully internalize the human intent behind the test.
2. Identify the domain structs, functions, and traits referenced in the test to locate the corresponding `src/` stubs containing `unimplemented!()` markers.
3. Read the `src/` stub files to understand the exact function signatures, types, and module boundaries that the worker must implement within.
4. Read `.github/copilot-instructions.md` and `.github/agents/rust-engineer.agent.md` for project coding standards and Rust-specific conventions.
5. `broadcast` at `info` level: `[BUILD] Starting task {task-id}: {harness-cmd}` with a summary of the test scenarios and stub files.

### Step 2: Mechanical Feedback Loop (Actor-Critic)

Execute the following loop with a **hard limit of 5 attempts**:

1. **Run** the targeted `${input:harness-cmd}`.
2. **If it passes** (exit code 0): proceed to Step 3.
3. **If it fails** (exit code != 0):
   a. Capture the raw `stderr` output (compiler errors, type mismatches, or panic traces).
   b. `broadcast` the failure summary at `warning` level.
   c. Analyze the error output and implement the fix:
      * **Compiler errors**: Fix type mismatches, missing imports, incorrect signatures in the `src/` stubs.
      * **Panic traces** (`unimplemented!()` or assertion failures): Implement the underlying logic inside the `src/` stubs to make the harness pass. Replace the `unimplemented!()` macros with real logic.
      * **Test assertion failures**: Fix the implementation logic (not the test itself, unless the test setup has a compilation error).
   d. Apply all project coding standards:
      * All fallible operations return `Result<T, AppError>` — never `unwrap()` or `expect()`.
      * Default visibility `pub(crate)` unless wider access is needed.
      * `///` doc comments on public items, `//!` on modules.
      * Run `cargo check` after each fix to verify compilation before re-running the harness.
   e. After each file write, `broadcast` the change at `info` level with the unified diff.
   f. **Do not modify the test file itself** unless fixing a compilation error in the test setup.
   g. Return to step 1 of this loop.

4. **Circuit breaker**: If 5 attempts are exhausted without the harness passing:
   * `broadcast` at `error` level: `[BUILD] Circuit breaker — 5 attempts exhausted, task blocked`.
   * Run `bd update ${input:task-id} --status blocked` to mark the task as blocked for human review.
   * Halt execution. Do not retry automatically.

### Step 3: Verification & State Update

Once the isolated harness passes:

1. **Workspace verification**: Run `cargo test` to verify no existing peripheral tests were broken.
   * If new failures appear, diagnose and fix them. Re-run until the full workspace test suite passes.
   * `broadcast` at `success` level: `[BUILD] Workspace tests pass — task {task-id} complete`.

2. **Lint verification**: Run `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`. Fix any violations.

3. **Commit**: Stage and commit validated changes:
   * `git add -A`
   * `git commit -m "feat: implement passing harness for ${input:task-id}"`
   * `broadcast` at `success` level: `[BUILD] Task {task-id} complete — commit {short_hash}`.

4. **State update**: Mark the task complete in Beads:
   * `bd close ${input:task-id} --reason "Harness passes, workspace tests green"`
