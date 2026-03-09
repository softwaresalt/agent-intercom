# Data Model: Slack UI Automated Testing

**Feature**: 008-slack-ui-testing | **Date**: 2026-03-09

## Overview

This feature adds test infrastructure — no new persistent entities or schema changes. The data model describes the test-time structures used across the three tiers.

## Test Infrastructure Entities

### TestScenario (conceptual — no Rust struct needed)

Represented as individual `#[test]` or `#[tokio::test]` functions in Rust (Tiers 1–2) and as `.spec.ts` files in Playwright (Tier 3).

| Attribute | Type | Description |
|---|---|---|
| scenario_id | string | Unique identifier (e.g., `T1_blocks_approval`, `T2_live_modal_thread`) |
| tier | 1 \| 2 \| 3 | Which testing tier |
| preconditions | text | Required state before execution |
| expected_outcome | text | What constitutes pass/fail |

### BlockKitAssertion (Tier 1 test helper)

A reusable assertion utility for verifying Block Kit JSON structure.

| Attribute | Type | Description |
|---|---|---|
| blocks | `Vec<SlackBlock>` | The Block Kit payload to verify |
| expected_block_types | `Vec<&str>` | Expected sequence of block types (section, actions, divider) |
| expected_action_ids | `Vec<&str>` | Expected `action_id` values in actions blocks |
| expected_text_patterns | `Vec<&str>` | Substring patterns that must appear in text content |

### LiveTestConfig (Tier 2 runtime configuration)

Read from environment variables at test startup.

| Attribute | Source | Description |
|---|---|---|
| bot_token | `SLACK_TEST_BOT_TOKEN` | Bot token for the test workspace |
| app_token | `SLACK_TEST_APP_TOKEN` | App token for Socket Mode |
| channel_id | `SLACK_TEST_CHANNEL_ID` | Dedicated test channel |
| authorized_user_id | `SLACK_TEST_USER_ID` | User ID for authorized interaction tests |

### VisualTestConfig (Tier 3 — in `playwright.config.ts`)

| Attribute | Source | Description |
|---|---|---|
| slack_workspace_url | env `SLACK_TEST_WORKSPACE_URL` | Slack workspace URL (e.g., `https://myworkspace.slack.com`) |
| slack_email | env `SLACK_TEST_EMAIL` | Test account email |
| slack_password | env `SLACK_TEST_PASSWORD` | Test account password |
| channel_name | env `SLACK_TEST_CHANNEL_NAME` | Channel name for navigation |
| screenshot_dir | config | Output directory for screenshots |
| modal_wait_timeout | config | Seconds to wait for modal detection (default: 5) |

## Existing Entities Used (no changes)

These existing domain entities are test targets — not modified by this feature:

- **ApprovalRequest** (`models/approval.rs`) — tested via approval flow scenarios
- **PromptRecord** (`models/prompt.rs`) — tested via prompt interaction scenarios
- **Session** (`models/session.rs`) — tested via session lifecycle and threading scenarios
- **StallAlert** (`models/stall.rs`) — tested via stall alert scenarios
- **AppState** (`mcp/handler.rs`) — constructed in test helpers with in-memory DB

## State Transitions Tested

The test suite verifies these existing state transitions (no new transitions added):

```
ApprovalRequest: pending → approved | rejected
PromptRecord: pending → continue | refine | stop
StallAlert: active → nudged | resolved
Session: created → active → paused → terminated
```

## No Schema Changes

This feature adds no database tables, columns, or migrations. All test data is created in-memory and discarded after each test.
