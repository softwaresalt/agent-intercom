<!-- markdownlint-disable-file -->
# PR Review Handoff: 006-acp-event-wiring

## PR Overview

Wires ACP agent events (`ClearanceRequested`, `PromptForwarded`) through the Slack approval
pipeline. Adds two new private async handlers in `src/main.rs`, shared Block Kit builders in
`src/slack/blocks.rs`, helper functions in domain models and approval repo, and 79 tests across
unit, contract, and integration tiers. The PR review identified 3 findings; 2 were applied as
fixes (committed `15997fa`) and 1 was deferred as a follow-up recommendation.

* Branch: `006-acp-event-wiring`
* Base Branch: `main`
* Total Files Changed: 10 production + 3 test files
* Total Review Comments: 3 (2 applied, 1 deferred)

## Quality Gate Results (Post-Review)

| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all -- --check` | ✅ Clean |
| Lint | `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ 0 warnings |
| Tests | `cargo test` | ✅ 996 passed, 0 failed |
| Working tree | `git status` | ✅ Clean (pushed `15997fa`) |

## PR Comments Ready for Submission

### File: `src/main.rs`

#### Comment 1 — RI-001 (Applied, commit `15997fa`)

* Category: Reliability / Correctness
* Severity: ⚠️ Medium

The `handle_clearance_requested` function called `blocks::build_approval_blocks`, which renders
a "📎 Diff uploaded as file (N lines)" section block whenever the diff exceeds
`INLINE_DIFF_THRESHOLD` (20 lines), but the ACP handler never called `slack.upload_file`.
This produced a misleading Slack message: the block promised an attached file that did not exist.

The MCP path (`ask_approval.rs` lines 227–243) correctly calls `upload_file` before building
the blocks. The fix mirrors that behaviour in the ACP handler.

**Applied Change** (`src/main.rs`, inside `handle_clearance_requested`)

```rust
// Upload the diff as a file when it exceeds the inline threshold,
// matching the behaviour of the MCP ask_approval tool.
let diff_line_count = diff_content.lines().count();
if diff_line_count >= blocks::INLINE_DIFF_THRESHOLD {
    let filename = format!(
        "{}.diff",
        effective_file_path.replace(['/', '.', '\\'], "_")
    );
    slack
        .upload_file(
            SlackChannelId(channel_id.clone()),
            &filename,
            &diff_content,
            session_thread_ts.clone(),
            Some("text"),
        )
        .await?;
}
```

---

#### Comment 2 — RI-002 (Applied, commit `15997fa`)

* Category: Code Quality
* Severity: 💡 Low

When an ACP agent omits the `description` field, `acp/reader.rs` produces an empty string via
`params.description.unwrap_or_default()`. The handler previously wrapped this unconditionally as
`Some(description)`, causing `build_approval_blocks` to render an empty section block in the
Slack message — a blank, invisible element that still occupies layout space.

**Applied Change** (`src/main.rs`, inside `handle_clearance_requested`)

```rust
// RI-002: treat empty description as absent so build_approval_blocks does
// not render a blank section block in the Slack message.
let description_opt = if description.is_empty() {
    None
} else {
    Some(description)
};
```

---

### File: `src/slack/blocks.rs`

#### Comment 3 — RI-003 (Deferred — follow-up recommendation)

* Category: Security (defense-in-depth)
* Severity: 💡 Low

`build_approval_blocks` embeds `title` and `description` from the ACP event directly into
Slack `mrkdwn` strings without escaping. An agent that sends a specially crafted title such as
`<https://evil.com|click here>` would cause Slack to render it as a clickable hyperlink.
`file_path` is safe because it is wrapped in backticks.

The exploitation risk is low in the current trusted-agent model, but a shared `slack_escape()`
utility should be added in a follow-up PR and applied to all user-supplied strings in block
builders. This would harden both the MCP and ACP paths consistently.

**Recommended Future Change** (new helper in `src/slack/blocks.rs` or `src/slack/util.rs`)

```rust
/// Escapes special Slack mrkdwn characters in user-supplied strings.
fn slack_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
```

Then apply in `build_approval_blocks` and `build_prompt_blocks` for `title` and `description`.

---

## Review Summary by Category

* Security Issues: 0 blocking (1 low-severity deferred — RI-003)
* Reliability / Correctness: 1 fixed — RI-001 (diff upload)
* Code Quality: 1 fixed — RI-002 (empty description guard)
* Convention Violations: 0
* Documentation: 0
* Performance: 0

## Instruction Compliance

* ✅ `constitution.instructions.md` (`applyTo: "**"`): Safety-First Rust upheld (no `unwrap`/`expect`), Test-First Development met (79 tests written first), Security Boundary Enforcement verified (path safety checked in `handle_clearance_requested`), Structured Observability met (`tracing::info!`/`error!` spans present in both handlers).

## Open Follow-ups (Not Blocking PR)

1. **RI-003 — Slack mrkdwn escaping**: Add `slack_escape()` utility and apply to all
   user-supplied strings (`title`, `description`) in `build_approval_blocks` and
   `build_prompt_blocks`. Affects both MCP and ACP paths — coordinate as a separate PR.
2. **AcpDriver resource leak**: `pending_clearances` and `pending_prompts_acp` HashMap entries
   are not cleaned up by `deregister_session`. Entries for pending requests on a terminated
   session leak indefinitely. Low risk (bounded by session count); track as technical debt.

## Commit Sequence

| Commit | Description |
|--------|-------------|
| `5c34d89` | Phase 1: setup baseline (919 tests) |
| `3b886ff` | Phase 2: shared block builders in `blocks.rs` |
| `00b611f` | Phase 3: `handle_clearance_requested` wired (US1) |
| `cf79ce1` | Phase 4: `handle_prompt_forwarded` wired (US2) |
| `4c4ee93` | Phase 5: integration tests S036–S041 (US3 thread continuity) |
| `3114d21` | Phase 6: concurrent/lifecycle integration tests |
| `5a8427a` | Phase 6: tracking files |
| `ba13287` | Phase 6: tracking commit |
| `ca27d23` | chore: backlog update |
| `15997fa` | fix: PR review fixes — diff upload and empty description guard |

