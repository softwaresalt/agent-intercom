---
id: TASK-006.04
title: "006 - Foundational — Shared Block Builder Extraction"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-006
dependencies: []
ordinal: 6040
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Purpose**: Extract Slack block-building functions from MCP tool handlers to the shared `src/slack/blocks.rs` module (Design Decision D1). Both ACP event handlers depend on these shared builders.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T003 Write unit tests for `build_approval_blocks()` output structure (header, description section, risk badge, diff code block, Accept/Reject buttons), `build_prompt_blocks()` output structure (header with icon+label, prompt text section, Continue/Refine/Stop buttons), diff truncation at `INLINE_DIFF_THRESHOLD` (20 lines), prompt text truncation via `truncate_text()`, and MCP/ACP output equivalence in tests/unit/acp_event_wiring.rs (S042–S046)
- [x] T004 Extract `build_approval_blocks()`, `approval_buttons()`, and `INLINE_DIFF_THRESHOLD` from src/mcp/tools/ask_approval.rs (line ~438, ~24) and `build_prompt_blocks()`, `prompt_buttons()`, `prompt_type_label()`, `prompt_type_icon()` from src/mcp/tools/forward_prompt.rs (lines ~261, ~297, ~307) to src/slack/blocks.rs as `pub(crate)`; relocate `truncate_text()` from src/mcp/tools/util.rs (line ~13) to src/slack/blocks.rs as `pub(crate)`
- [x] T005 [P] Update src/mcp/tools/ask_approval.rs to remove local `build_approval_blocks` function and `INLINE_DIFF_THRESHOLD` constant; add `use crate::slack::blocks::{build_approval_blocks, INLINE_DIFF_THRESHOLD};`
- [x] T006 [P] Update src/mcp/tools/forward_prompt.rs to remove local `build_prompt_blocks`, `prompt_type_label`, `prompt_type_icon`; add `use crate::slack::blocks::{build_prompt_blocks, prompt_type_label, prompt_type_icon};`; update src/mcp/tools/util.rs to re-export `truncate_text` from `crate::slack::blocks` if other consumers exist, otherwise remove the local copy
- [x] T007 Run quality gates — verify shared block builder tests (S042–S046) pass and all existing MCP tool tests remain green: `cargo check && cargo clippy -- -D warnings && cargo fmt --all -- --check && cargo test`

**Checkpoint**: Shared block builders available at `crate::slack::blocks`. MCP tools unchanged in behavior. User story implementation can begin.

---

<!-- SECTION:DESCRIPTION:END -->
