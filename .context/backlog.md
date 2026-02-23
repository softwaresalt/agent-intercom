# Backlog

## Feature Topics

- Ability to proactively engage an agent in active session by end-user in Slack.
- Ability to configure different detail levels of context sharing/messaging to Slack channel user context sharing.
- Enhance forward_prompt command: Both "Resume with Instructions" and "Refine" currently use placeholder strings ("(instruction via Slack)"). Slack modal support for collecting actual typed instructions is noted as future work in the handlers.
- Not currently getting notifications to Slack of Approval requests, e.g. read a file outside the current workspace.
- Not currently getting notifications to Slack of diff_acceptance approvals.
- Not currently getting notifications to Slack of agent session continuation approvals.
- Consider upgrading rmcp crate to 0.13.0; breaking changes would require a full feature refactor to implement.
- Additions to .agentrc/settings.json should hot-reload to the server memory. `PolicyWatcher` already supports `register()` / `get_policy()` / `cache()` — the remaining work is wiring `PolicyCache` into `AppState` (cascades to ~11 struct constructions across 7 test files) and switching `check_auto_approve` from `PolicyLoader::load()` to cache reads. This should be a dedicated feature spec.

