# ADR-0012: Workspace-Self-Contained Auto-Approve Policy (Removal of Global Allowlist Gate)

**Status**: Accepted
**Date**: 2026-02-23
**Phase**: 001-002-integration-test (post-merge clean-up)

## Context

The original design (FR-011, spec `001-mcp-remote-agent-server`) required that
any command listed in a workspace's `.agentrc/settings.json` `commands` array
also appear in the server's global `config.toml` `[commands]` registry before
auto-approve would be granted. The intent was to prevent a compromised or
malicious workspace policy from expanding permissions beyond what the server
administrator had explicitly sanctioned.

In practice this two-tier gate created friction:

* Operators had to maintain two lists in sync: workspace policy **and** server
  config, for every command pattern they wanted to auto-approve.
* The `config.commands` registry is designed for a distinct purpose — Slack
  slash-command aliases (FR-014) — not for policy allowlisting. Conflating
  the two responsibilities made both harder to reason about.
* Workspace policy is already scoped to a specific workspace root and is owned
  by the operator who controls both the repository and the server config. The
  threat model of a "malicious workspace policy" does not apply when the
  operator is the same party in both roles.
* The regex upgrade to `auto_approve_commands` (replacing glob `commands`)
  makes patterns more expressive, and regex patterns would be awkward to
  mirror verbatim into a TOML allowlist.

## Decision

Remove the global allowlist gate from the auto-approve evaluation path:

* `PolicyLoader::load()` no longer accepts or uses `global_commands`; it
  simply parses `.agentrc/settings.json` into `WorkspacePolicy` as-is.
* `PolicyEvaluator::evaluate()` no longer accepts or uses `global_commands`;
  command matching is decided solely by `WorkspacePolicy.auto_approve_commands`
  (a `Vec<String>` of regular expressions).
* `GlobalConfig.commands` (`HashMap<String, String>`) is retained but is now
  exclusively used by `src/slack/commands.rs` for Slack slash-command alias
  execution (FR-014). It has no effect on MCP auto-approve policy.

The `commands` field in workspace policy JSON is renamed to
`auto_approve_commands` (with a backward-compatible `#[serde(alias = "commands")]`
so existing `.agentrc/settings.json` files continue to work).

## Consequences

### Positive

- Single source of truth for auto-approve rules: the workspace
  `.agentrc/settings.json` file alone determines what is auto-approved.
- No synchronization burden between workspace policy and server config.
- `GlobalConfig.commands` has a clear, single responsibility: Slack slash-
  command aliases.
- Regex patterns in `auto_approve_commands` are more expressive than the
  previous glob-based `commands` list.

### Negative

- A workspace policy file can now grant auto-approve for any command pattern
  the operator writes, with no server-side allowlist as a backstop. This is
  acceptable because the operator who writes the workspace policy also controls
  the server and its configuration; the threat model does not include a
  workspace-policy attacker who is distinct from the server operator.

### Risks

- If the threat model changes (e.g., multi-tenant deployment where different
  parties own the workspace vs. the server), the global allowlist gate should
  be re-introduced. The implementation is straightforward to restore: add
  `global_commands: &HashMap<String, String>` back to both `PolicyLoader::load`
  and `PolicyEvaluator::evaluate`, and filter/check against it before approving.

## Updated Requirements

- **FR-011 (superseded)**: The original requirement ("workspace policy MUST NOT
  expand permissions beyond global config") is superseded by this decision.
  The updated intent of FR-011 is: workspace auto-approve policy is entirely
  self-contained in `.agentrc/settings.json`; `config.commands` is exclusively
  for Slack slash-command aliases (FR-014).
