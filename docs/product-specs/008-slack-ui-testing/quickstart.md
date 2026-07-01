# Quickstart: Slack UI Automated Testing

**Feature**: 008-slack-ui-testing

## Tier 1 — Offline Tests (CI-safe)

```powershell
# Runs automatically with the standard test suite
cargo test
```

No configuration needed. All Tier 1 tests use in-memory SQLite and mock AppState.

## Tier 2 — Live Slack API Tests

### Prerequisites

1. A Slack workspace with the agent-intercom app installed
2. A dedicated test channel (e.g., `#intercom-test`)
3. Bot and app tokens with appropriate scopes

### Configuration

Set environment variables:

```powershell
$env:SLACK_TEST_BOT_TOKEN = "xoxb-..."
$env:SLACK_TEST_CHANNEL_ID = "C_TEST_CHANNEL"
```

### Run

```powershell
cargo test --features live-slack-tests
```

## Tier 3 — Visual Browser Tests

### Prerequisites

1. Node.js 18+ installed
2. A dedicated Slack test account (email/password login, no 2FA)
3. The agent-intercom server running and connected to the test workspace

### Setup

```powershell
cd tests/visual
npm install
npx playwright install chromium
```

### Configuration

Copy `tests/visual/.env.example` to `tests/visual/.env` and fill in the values,
or export the variables directly:

```powershell
$env:SLACK_WORKSPACE_URL = "https://myworkspace.slack.com"
$env:SLACK_EMAIL = "test@example.com"
$env:SLACK_PASSWORD = "..."
$env:SLACK_TEST_CHANNEL = "agent-intercom-test"
```

### First Run (authenticates and persists session)

```powershell
npx playwright test --project=setup
```

### Run Visual Tests

```powershell
npx playwright test
```

### View Report

```powershell
npx playwright show-report reports/
```

Screenshots are saved to `tests/visual/screenshots/`.
