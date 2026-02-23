---
name: hitl-test
description: "Usage: Run HITL tests. Executes human-in-the-loop test scenarios against a live monocoque-agent-rc server with real Slack integration."
version: 1.0
maturity: experimental
---

# HITL Integration Test Skill

Executes structured test scenarios against a live monocoque-agent-rc MCP server to validate the full Slack-mediated approval workflow with a real human operator.

## Prerequisites

* `monocoque-agent-rc` is running via `run_debug.ps1` (or the VS Code task "Run monocoque-agent-rc (debug)")
* The server is connected to a real Slack workspace with a configured `channel_id` in `.vscode/mcp.json`
* The operator is actively monitoring the Slack channel on a device
* The workspace may have `.agentrc/settings.json` with auto-approve rules (Scenario 2 will baseline this)

## Required Steps

### Step 1: Server Health Check

Call `heartbeat` with `status_message: "HITL test suite starting"` to confirm the MCP server is reachable.

* If `heartbeat` returns `acknowledged: true`, report success and proceed.
* If `heartbeat` returns an error or times out, halt immediately — the server is not running or the MCP connection is not configured. Instruct the user to start the server and ensure `.vscode/mcp.json` points to the correct SSE endpoint.

### Step 2: Notify Operator and Confirm Readiness

Call `remote_log` with:
- `message`: `"[HITL TEST] Starting test suite. Please monitor Slack for approval requests. Tests will instruct you to APPROVE or REJECT each proposal."`
- `level`: `"info"`

Then call `forward_prompt` with:
- `prompt_text`: `"HITL test suite is ready to begin. Are you monitoring the Slack channel? Reply 'go' to start."`

Wait for the operator's response. If the operator confirms, proceed. If the response indicates they are not ready, wait and re-prompt.

### Step 3: Execute Scenarios

Load scenarios from `.github/skills/hitl-test/scenarios.md`.

For each scenario:

1. Call `remote_log` with `message: "[TEST] Starting scenario {N}: {name}"`, `level: "info"`
2. Execute the scenario steps exactly as defined in scenarios.md
3. Validate the response against the expected outcomes listed in the scenario
4. Record the result as PASS or FAIL with details about what matched or diverged
5. Call `remote_log` with `message: "[TEST] Result: {PASS|FAIL} — {name} — {details}"`, `level: "success"` for PASS or `level: "warning"` for FAIL
6. If a scenario fails, **continue to the next scenario** — do not halt the suite

### Step 4: Produce Summary

After all scenarios complete, produce a markdown summary table:

```markdown
## HITL Test Results — {date}

| # | Scenario | Expected | Actual | Status |
|---|----------|----------|--------|--------|
| 1 | ...      | ...      | ...    | PASS   |
| 2 | ...      | ...      | ...    | FAIL   |
```

Include a final count: `{passed}/{total} scenarios passed`.

Post the summary via `remote_log` with `level: "info"` so it appears in the Slack channel for the operator to see.

Also output the summary in the chat for the developer to review.

### Step 5: Cleanup

1. Call `set_operational_mode` with `mode: "remote"` to restore the default mode if it was changed during testing.
2. Call `remote_log` with `message: "[HITL TEST] Suite complete. {passed}/{total} passed."`, `level: "success"` if all passed or `level: "warning"` if any failed.

## Error Handling

| Error | Action |
|---|---|
| `heartbeat` fails | Halt — server not running |
| Operator does not respond to readiness prompt | Re-prompt once after 30 seconds, then halt |
| Individual scenario tool call fails | Record as FAIL, continue to next scenario |
| `accept_diff` returns `patch_conflict` | Record as FAIL with details, continue |
| Operator rejects when approval was expected (or vice versa) | Record as FAIL — this tests the operator following instructions |

## Notes

* This is not a fully automated test harness. It requires active human participation.
* The primary value is testing the real Slack rendering, button interactions, and end-to-end async flow that unit/integration tests cannot cover.
* Results can be compared across runs by saving the summary table to `logs/hitl-results-{timestamp}.md`.
