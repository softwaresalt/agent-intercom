---
name: automated-test-harness
description: "Usage: Run automated tests. Executes Rust API coverage and a self-seeding Playwright UX suite so routine validation does not require the manual HITL skill."
version: 1.0
maturity: experimental
input:
  properties:
    suite:
      type: string
      enum: [all, api, visual]
      description: "Which automated suites to run. `all` runs API coverage plus the Playwright UX harness."
    include-live-slack:
      type: boolean
      description: "When true, also run the feature-gated live Slack API tests."
    server-mode:
      type: string
      enum: [mcp, acp]
      description: "Server mode to use if the harness needs to start agent-intercom for visual coverage."
    skip-server-start:
      type: boolean
      description: "Assume any required server is already running."
    bootstrap-visual-deps:
      type: boolean
      description: "Install Playwright project dependencies when `tests/visual/node_modules` is missing."
  required: []
---

# Automated Test Harness Skill

Runs the automated regression harness that combines:

* Rust unit + contract tests
* Rust integration tests
* Optional live Slack API tests
* A self-seeding Playwright UX suite under `tests/visual`

Use this skill instead of `hitl-test` for routine API plus browser regression
coverage. The manual HITL skill remains the fallback for flows that genuinely
need a human operator in Slack.

## Prerequisites

* The repository builds locally (`cargo check` should already pass or be close to passing).
* PowerShell 7+ (`pwsh`) is available.
* For the automated Playwright UX suite:
  * `tests/visual/.env` exists, or equivalent environment variables are set.
  * Slack login variables are configured:
    * `SLACK_WORKSPACE_URL`
    * `SLACK_EMAIL`
    * `SLACK_PASSWORD`
    * `SLACK_TEST_CHANNEL`
  * Slack Web API fixture variables are configured:
    * `SLACK_TEST_BOT_TOKEN`
    * `SLACK_TEST_CHANNEL_ID`
* If `bootstrap-visual-deps` is not requested, `tests/visual/node_modules`
  already exists.

## Coverage Model

The harness deliberately separates responsibilities:

| Layer | Command | Purpose |
|---|---|---|
| Rust API | `cargo test --lib --test unit --test contract` | Contract and unit validation |
| Rust API | `cargo test --test integration` | Handler and repository integration coverage |
| Live Slack API (optional) | `cargo test --features live-slack-tests --test live` | Real Slack Web API verification without browser automation |
| Playwright UX | `npm run test:automated` in `tests/visual` | Self-seeding Slack UI rendering and thread-navigation coverage |

The Playwright harness is intentionally **self-seeding**. It posts its own
fixture messages to Slack, validates how they render in the real Slack web
client, and deletes them afterward. That keeps the browser layer automated
instead of depending on pre-existing HITL artifacts in the channel.

## Required Steps

### Step 1: Load the Harness Context

Read:

* `scripts/run_automated_test_harness.ps1`
* `tests/visual/scenarios/automated-harness.spec.ts`
* `tests/visual/helpers/slack-fixtures.ts`

These files define the executable workflow, the seeded browser scenarios, and
the Slack fixture contract used by the automated visual layer.

### Step 2: Resolve Requested Coverage

Determine the script arguments from the user request:

* Default to `suite = all`.
* Set `include-live-slack = true` only when the user asks for real Slack API
  coverage or when you are explicitly doing a full Slack-backed regression run.
* Set `server-mode` only when the harness needs to start agent-intercom for
  supplemental visual coverage. The self-seeding Playwright suite does not
  require live HITL state.
* Use `bootstrap-visual-deps = true` only when `tests/visual/node_modules` is
  missing and the user wants the harness to bootstrap the Playwright project.

### Step 3: Detect agent-intercom Availability

If the `agent-intercom` MCP server is reachable, call `ping` with a short
status message such as `"automated test harness starting"`.

* If `ping` succeeds, use `broadcast` for non-blocking status updates.
* If `ping` fails, continue in local-only mode. This harness must not depend on
  operator responses.

### Step 4: Run the Harness Script

Run the PowerShell harness directly:

```text
pwsh -File scripts/run_automated_test_harness.ps1 -Suite <suite>
```

Add switches as needed:

* `-IncludeLiveSlack`
* `-ServerMode mcp|acp`
* `-SkipServerStart`
* `-BootstrapVisualDeps`

Do not replace the script with a long ad-hoc sequence of terminal commands
unless the script itself is broken and you are actively repairing it.

### Step 5: Inspect and Report the Summary

The script prints a phase-by-phase summary table. Surface that summary directly
to the user:

* `PASS` — the phase completed successfully.
* `FAIL` — the command ran and returned a failing exit code or runtime error.
* `SKIP` — the harness deliberately skipped the phase due to missing
  prerequisites or because that layer was not requested.

If a phase is skipped, include the exact missing prerequisites in your response.

### Step 6: Escalate Only When Automation Truly Runs Out

Recommend `hitl-test` only when one of the following is true:

* The user explicitly asks for human-verified operator behavior.
* The automated harness cannot reproduce the requested scenario with seeded
  fixtures and live API coverage.
* The browser layer needs a real approval or prompt session that is not yet
  scriptable without operator input.

When escalating, explain why the automated harness was insufficient.

## Error Handling

| Problem | Action |
|---|---|
| `cargo` test phase fails | Report the failing phase and surface the failing command output |
| Live Slack env vars missing | Mark the live Slack phase as `SKIP` and list the missing variables |
| Playwright `.env` or fixture env vars missing | Mark the visual phase as `SKIP` and list the missing variables |
| `tests/visual/node_modules` missing | Skip the visual phase unless `bootstrap-visual-deps` was requested |
| Browser automation fails after fixtures are seeded | Report as `FAIL` and preserve the Playwright report location |
| `agent-intercom` is unavailable | Continue locally; do not block on Slack availability |

## Notes

* The automated Playwright suite is complementary to the broader manual visual
  suite already under `tests/visual/scenarios/`. It focuses on deterministic,
  self-seeded browser coverage suitable for repeatable agent-driven runs.
* The legacy `hitl-test` skill remains valuable for manual acceptance testing,
  but it is no longer the default recommendation for routine API plus UX
  regression coverage.
