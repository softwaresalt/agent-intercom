---
id: TASK-001.10
title: "001 - User Story 6 — Workspace Auto-Approve Policy (Priority: P2)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001
dependencies: []
ordinal: 1100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Workspace policy file auto-approves pre-trusted operations, reducing Slack notification noise

**Independent Test**: Create `.agentrc/settings.json` with "cargo test" auto-approved, invoke `check_auto_approve`, verify returns `auto_approved: true`

### Tests (Constitution Principle III)

- [X] T116 Write unit tests for policy loader in `tests/unit/policy_tests.rs`: valid policy file parsing, malformed file fallback to deny-all, `auto_approve_commands` preserved from workspace policy (global allowlist gate removed per ADR-0012), missing policy file returns deny-all
- [X] T117 Write unit tests for policy evaluator in `tests/unit/policy_evaluator_tests.rs`: command matching, tool matching, file pattern glob matching, risk_level_threshold enforcement, global config supersedes workspace config
- [X] T118 Write contract tests for `check_auto_approve` tool in `tests/contract/check_auto_approve_tests.rs`: validate input/output schemas per mcp-tools.json

### Implementation for User Story 6

- [X] T061 [US6] Implement policy file loader in `src/policy/loader.rs`: parse `.agentrc/settings.json` from a given `workspace_root` into `WorkspacePolicy` struct; on parse error, fall back to "require approval for everything" and emit tracing warning (edge case from spec). ~~validate `commands` entries exist in global `config.commands` allowlist (FR-011)~~ → global allowlist gate removed per ADR-0012; workspace policy is self-contained.
- [X] T062 [US6] Implement policy evaluator in `src/policy/evaluator.rs`: `check_auto_approve(tool_name, context, workspace_policy, global_config) -> AutoApproveResult` matching against commands, tools, file_patterns, and risk_level_threshold; return matched rule name or `auto_approved: false`
- [X] T063 [US6] Implement policy hot-reload via `notify` file watcher in `src/policy/watcher.rs`: watch `.agentrc/settings.json` for each active workspace_root using `notify::RecommendedWatcher`; on change event, reload policy via loader; register/unregister watchers as sessions start/terminate (FR-010)
- [X] T064 [US6] Implement `check_auto_approve` MCP tool handler in `src/mcp/tools/check_auto_approve.rs`: accept `tool_name` and `context` per mcp-tools.json contract; load policy for session's workspace_root; evaluate via `PolicyEvaluator`; return `{auto_approved, matched_rule}` per contract
- [X] T065 [US6] Create `src/policy/mod.rs` re-exporting loader, evaluator, watcher
- [X] T066 [US6] Add tracing spans to policy evaluation: span with tool_name, matched_rule, auto_approved attributes

**Checkpoint**: Auto-approve policy functional — trusted operations bypass Slack round-trip

---

<!-- SECTION:DESCRIPTION:END -->
