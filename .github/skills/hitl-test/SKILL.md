---
name: hitl-test
description: "Usage: Run HITL tests. Executes human-in-the-loop test scenarios against a live agent-intercom server with real Slack integration."
version: 2.0
maturity: experimental
---

# HITL Integration Test Skill

Executes structured test scenarios against a live agent-intercom MCP server to validate the full Slack-mediated approval workflow, operator steering queue, task inbox, modal instruction capture, file attachments, audit logging, detail levels, and stall detection with a real human operator.

## Prerequisites

* `agent-intercom` is running via `run_debug.ps1` (or the VS Code task "Run agent-intercom (debug)")
* The server is connected to a real Slack workspace with a configured `channel_id` in `.vscode/mcp.json`
* The operator is actively monitoring the Slack channel on a device
* The workspace may have `.intercom/settings.json` with auto-approve rules (Scenario 3 will baseline this)
* The `/intercom` slash command is registered in the Slack app (required for Scenarios 12 and 13)

## Required Steps

### Step 1: Server Health Check

Call `ping` with `status_message: "HITL test suite starting"` to confirm the MCP server is reachable.

* If `ping` returns `acknowledged: true`, report success and proceed.
* If `ping` returns an error or times out, halt immediately — the server is not running or the MCP connection is not configured. Instruct the user to start the server and ensure `.vscode/mcp.json` points to the correct SSE endpoint.

### Step 2: Notify Operator and Confirm Readiness

Call `broadcast` with:
- `message`: `"[HITL TEST] Starting test suite. Please monitor Slack for approval requests. Tests will instruct you to APPROVE or REJECT each proposal."`
- `level`: `"info"`

Then call `transmit` with:
- `prompt_text`: `"HITL test suite is ready to begin. Are you monitoring the Slack channel? Reply 'go' to start."`

Wait for the operator's response. If the operator confirms, proceed. If the response indicates they are not ready, wait and re-prompt.

### Step 3: Execute Scenarios

Load scenarios from `.github/skills/hitl-test/scenarios.md`.

For each scenario:

1. **Announce in chat:** Output `"▶ Scenario {N}: {name}"` in the chat response so the developer has visibility regardless of MCP state.
2. **Best-effort MCP log:** Call `broadcast` with `message: "[TEST] Starting scenario {N}: {name}"`, `level: "info"`. If **this call fails or hangs**, skip it — do not let a logging failure stall the suite.
3. Execute the scenario steps exactly as defined in scenarios.md.
4. Validate the response against the expected outcomes listed in the scenario.
5. Record the result as PASS or FAIL with details about what matched or diverged.
6. **Report result in chat first**, then best-effort via `broadcast` with `message: "[TEST] Result: {PASS|FAIL} — {name} — {details}"`, `level: "success"` for PASS or `level: "warning"` for FAIL.
7. If a scenario fails, **continue to the next scenario** — do not halt the suite.

**Critical rule:** Chat output is the primary reporting channel. `broadcast` calls during scenario execution are supplementary. If any `broadcast` call returns an error, log the error in chat and proceed — never retry a failed `broadcast` or block on it.

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

Post the summary via `broadcast` with `level: "info"` so it appears in the Slack channel for the operator to see.

Also output the summary in the chat for the developer to review.

### Step 5: Cleanup

1. Call `switch_freq` with `mode: "remote"` to restore the default mode if it was changed during testing. If this call fails, note it in chat and continue.
2. Call `broadcast` with `message: "[HITL TEST] Suite complete. {passed}/{total} passed."`, `level: "success"` if all passed or `level: "warning"` if any failed. If this call fails, the chat summary from Step 4 is sufficient.

**If the MCP server became unresponsive during the suite**, skip all Step 5 calls entirely — they will also hang. The chat summary is the authoritative record.

## Error Handling

| Error | Action |
|---|---|
| `ping` fails | Halt — server not running |
| Operator does not respond to readiness prompt | Re-prompt once after 30 seconds, then halt |
| Individual scenario tool call fails | Record as FAIL **in chat**, best-effort `broadcast`, continue to next scenario |
| `broadcast` itself fails | Log the error **in chat only** and continue — never stall on a logging call |
| `check_diff` returns `patch_conflict` | Record as FAIL with details, continue |
| Operator rejects when approval was expected (or vice versa) | Record as FAIL — this tests the operator following instructions |
| Any MCP tool call hangs (no response) | If no response within a reasonable time, record as FAIL/TIMEOUT in chat, skip remaining steps for that scenario, continue to next scenario |

### Error Containment Principle

Every MCP tool call during scenario execution is **independently failable**. A failure in one call must never prevent the agent from:
1. Recording the result in chat
2. Moving to the next scenario
3. Producing the final summary

If the MCP server becomes entirely unresponsive mid-suite, the agent should:
1. Record all remaining scenarios as SKIP (server unresponsive)
2. Output the summary table in chat
3. Do **not** attempt Step 5 cleanup calls — they will also hang

## Notes

* This is not a fully automated test harness. It requires active human participation.
* The primary value is testing the real Slack rendering, button interactions, and end-to-end async flow that unit/integration tests cannot cover.
* Results can be compared across runs by saving the summary table to `logs/hitl-results-{timestamp}.md`.
