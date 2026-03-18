---
name: hitl-test
description: "Usage: Run HITL tests. Executes human-in-the-loop test scenarios against a live agent-intercom server with real Slack integration."
version: 3.1
maturity: experimental
---

# HITL Integration Test Skill

Executes structured test scenarios against a live agent-intercom server to
validate the full Slack-mediated approval workflow, operator steering queue,
task inbox, modal instruction capture, text-only thread @-mention replies (US17),
file attachments, audit logging, detail levels, stall detection, and (in ACP mode)
session lifecycle, threading, and workspace routing — all with a real human operator.

## US17 — Text-Only Thread Prompts (Important Behavior Change)

When the server session has a `session_thread_ts` set (always in ACP sessions,
and in MCP sessions with an active thread), the following tools change behavior:

| Tool | Old behavior (non-threaded) | New behavior (threaded, US17) |
|------|-----------------------------|---------------------------------|
| `transmit` | Posts block-kit card with Continue/Refine/Stop buttons | Posts plain text in thread; waits for `@agent-intercom continue/refine/stop` @-mention reply |
| `standby` | Posts block-kit card with Resume with Instructions button | Posts plain text in thread; waits for `@agent-intercom resume <instruction>` @-mention reply |
| `check_clearance` | Posts block-kit card with Approve/Reject buttons | Posts plain text in thread; waits for `@agent-intercom approve/reject <reason>` @-mention reply |

**Main channel messages (no thread) are unchanged** — they still use block-kit buttons.

### @-mention decision keywords

| Tool | Keywords | Example |
|------|----------|---------|
| `transmit` / `forward_prompt` | `continue`, `refine <text>`, `stop` | `@agent-intercom refine focus on error handling` |
| `standby` / `wait_for_instruction` | `resume <text>`, `stop` | `@agent-intercom resume switch to integration tests` |
| `check_clearance` / `ask_approval` | `approve`, `reject <reason>` | `@agent-intercom reject too risky` |

- The first whitespace-delimited word is the decision keyword (case-insensitive).
- Everything after the keyword is the instruction/reason text.
- An empty reply (just `@agent-intercom`) defaults to `"continue"`.
- Only the session owner's @-mention is accepted; unauthorized mentions are silently ignored.

## Prerequisites

* `agent-intercom` is running via `run_debug.ps1` (or the VS Code task "Run agent-intercom (debug)")
* The server is connected to a real Slack workspace with a configured `channel_id` in `.vscode/mcp.json`
* The operator is actively monitoring the Slack channel on a device
* The workspace may have `.intercom/settings.json` with auto-approve rules (Scenario 3 will baseline this)
* The slash command is registered in the Slack app:
  - MCP mode: `/acom` prefix (e.g., `/acom steer`, `/acom task`)
  - ACP mode: `/arc` prefix (e.g., `/arc session-start`, `/arc steer`)

### Additional ACP Prerequisites

* The server was started with `--mode acp`
* `config.toml` has valid `host_cli` and `host_cli_args` fields pointing to an ACP-capable agent binary
* At least one `[[workspace]]` mapping exists in `config.toml` for the test channel

## Required Steps

### Step 1: Server Health Check

Call `ping` with `status_message: "HITL test suite starting"` to confirm the MCP server is reachable.

* If `ping` returns `acknowledged: true`, report success and proceed.
* If `ping` returns an error or times out, halt immediately — the server is not running or the MCP connection is not configured. Instruct the user to start the server and ensure `.vscode/mcp.json` points to the correct SSE endpoint.

### Step 2: Select Test Mode

Ask the operator which mode the server is running in. This determines which
scenario file to load:

| Mode | Scenario file | Slash prefix | Description |
|------|---------------|--------------|-------------|
| **MCP** | `scenarios-mcp.md` | `/acom` | Default MCP transport — tests approval workflow, steering, task inbox, terminal command gate, audit, stall detection |
| **ACP** | `scenarios-acp.md` | `/arc` | ACP transport — tests session lifecycle (start/stop/restart), threading, workspace routing, plus core approval/steering/task flows |

**How to select:**
1. Present the user with a choice: *"Which mode is the server running in? (MCP or ACP)"*
2. If the user specifies **MCP**, load `.github/skills/hitl-test/scenarios-mcp.md`
3. If the user specifies **ACP**, load `.github/skills/hitl-test/scenarios-acp.md`
4. If the user says **"both"**, run MCP scenarios first, then inform the user they
   need to restart the server in ACP mode before running the ACP suite

### Step 3: Notify Operator and Confirm Readiness

Call `broadcast` with:
- `message`: `"[HITL TEST] Starting {mode} test suite. Please monitor Slack for approval requests. Tests will instruct you to APPROVE or REJECT each proposal."`
- `level`: `"info"`

(Replace `{mode}` with `MCP` or `ACP` based on the selected mode.)

Then call `transmit` with:
- `prompt_text`: `"HITL test suite ({mode} mode) is ready to begin. Are you monitoring the Slack channel? Reply 'go' to start."`

Wait for the operator's response. If the operator confirms, proceed. If the response indicates they are not ready, wait and re-prompt.

### Step 4: Execute Scenarios

Load scenarios from the mode-specific file selected in Step 2:
- MCP: `.github/skills/hitl-test/scenarios-mcp.md`
- ACP: `.github/skills/hitl-test/scenarios-acp.md`

For each scenario:

1. **Announce in chat:** Output `"▶ Scenario {N}: {name}"` in the chat response so the developer has visibility regardless of MCP state.
2. **Best-effort MCP log:** Call `broadcast` with `message: "[TEST] Starting scenario {N}: {name}"`, `level: "info"`. If **this call fails or hangs**, skip it — do not let a logging failure stall the suite.
3. Execute the scenario steps exactly as defined in the loaded scenario file.
4. Validate the response against the expected outcomes listed in the scenario.
5. Record the result as PASS or FAIL with details about what matched or diverged.
6. **Report result in chat first**, then best-effort via `broadcast` with `message: "[TEST] Result: {PASS|FAIL} — {name} — {details}"`, `level: "success"` for PASS or `level: "warning"` for FAIL.
7. If a scenario fails, **continue to the next scenario** — do not halt the suite.

**Critical rule:** Chat output is the primary reporting channel. `broadcast` calls during scenario execution are supplementary. If any `broadcast` call returns an error, log the error in chat and proceed — never retry a failed `broadcast` or block on it.

### Step 5: Produce Summary

After all scenarios complete, produce a markdown summary table:

```markdown
## HITL Test Results — {mode} Mode — {date}

| # | Scenario | Expected | Actual | Status |
|---|----------|----------|--------|--------|
| 1 | ...      | ...      | ...    | PASS   |
| 2 | ...      | ...      | ...    | FAIL   |
```

Include a final count: `{passed}/{total} scenarios passed`.

Post the summary via `broadcast` with `level: "info"` so it appears in the Slack channel for the operator to see.

Also output the summary in the chat for the developer to review.

### Step 6: Cleanup

1. Call `switch_freq` with `mode: "remote"` to restore the default mode if it was changed during testing. If this call fails, note it in chat and continue.
2. Call `broadcast` with `message: "[HITL TEST] {mode} suite complete. {passed}/{total} passed."`, `level: "success"` if all passed or `level: "warning"` if any failed. If this call fails, the chat summary from Step 5 is sufficient.
3. **ACP mode only:** If any ACP test sessions remain active, instruct the operator to terminate them via `/arc session-clear`.

**If the MCP server became unresponsive during the suite**, skip all Step 6 calls entirely — they will also hang. The chat summary is the authoritative record.

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
| ACP session-start fails (handshake timeout) | Record as FAIL, skip scenarios that depend on an active session |

### Error Containment Principle

Every MCP tool call during scenario execution is **independently failable**. A failure in one call must never prevent the agent from:
1. Recording the result in chat
2. Moving to the next scenario
3. Producing the final summary

If the MCP server becomes entirely unresponsive mid-suite, the agent should:
1. Record all remaining scenarios as SKIP (server unresponsive)
2. Output the summary table in chat
3. Do **not** attempt Step 6 cleanup calls — they will also hang

## Scenario File Reference

| File | Mode | Scenarios | Key features tested |
|------|------|-----------|---------------------|
| `scenarios-mcp.md` | MCP | 22 | Ping, broadcast, auto-approve, approval/rejection, modal capture, wait-for-instruction, mode toggle, state recovery, double-submission prevention, risk levels, file attachments, steering queue, task inbox, modal dismiss, auto-approve suggestion, detail levels, audit log, stall detection, terminal command gate |
| `scenarios-acp.md` | ACP | 22 | Ping, broadcast, session-start, session threading, sessions list, pause/resume, approval/rejection, modal capture, wait-for-instruction, steering queue, task inbox, checkpoints, session-stop, session-restart, max sessions, ACP-only commands, workspace mapping, stall detection, session-clear, audit log |

## Notes

* This is not a fully automated test harness. It requires active human participation.
* The primary value is testing the real Slack rendering, button interactions, and end-to-end async flow that unit/integration tests cannot cover.
* Results can be compared across runs by saving the summary table to `logs/hitl-results-{mode}-{timestamp}.md`.
* When running "both" modes, the MCP suite runs first. The operator must restart the server with `--mode acp` before the ACP suite can begin.
