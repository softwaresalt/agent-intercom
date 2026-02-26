# Phase 3 Memory: Operator Steering Queue (T014-T023)

Date: 2026-02-25
Spec: 004-intercom-advanced-features
Commit: 9bf1302

## What Was Built

Operator steering queue enabling proactive message delivery to running agents via `ping`/heartbeat.

### New Files
- `src/models/steering.rs` — `SteeringMessage`, `SteeringSource` (Slack/Ipc/Ctl)
- `src/persistence/steering_repo.rs` — `SteeringRepo` with insert/fetch_unconsumed/mark_consumed/purge
- `src/slack/handlers/steer.rs` — shared ingestion logic for Slack app mentions and slash commands
- `tests/unit/steering_repo_tests.rs` — 9 unit tests (S001-S011)
- `tests/contract/ping_contract_tests.rs` — 3 contract tests (S002-S003)
- `tests/integration/steering_flow_tests.rs` — 10 integration tests (S001-S009)

### Modified Files
- `src/mcp/tools/heartbeat.rs` — fetch pending steering, deliver in `pending_steering`, mark consumed; extracted `update_session_progress` and `fetch_and_consume_steering` helpers to pass clippy too_many_lines
- `src/slack/commands.rs` — `/intercom steer <text>` slash command
- `src/slack/events.rs` — `app_mention` → `steer::handle_app_mention`
- `src/ipc/server.rs` — `steer` IPC command
- `ctl/main.rs` — `steer` subcommand
- `src/slack/handlers/mod.rs` — registered steer module

## Key Decisions
- `strip_mention` in steer.rs uses `.trim_start()` on result to handle variable whitespace after `>`
- `heartbeat.handle` factored into helpers to stay under clippy `too_many_lines` (100-line limit)
- `store_from_slack` already returns `crate::Result<String>` — no `.map_err` needed in dispatch_command

## Test Count
- Before: 570 tests
- After: 592 tests (+22)

## Gates
- ✅ cargo check
- ✅ cargo test (592/592)
- ✅ cargo clippy --all-targets -- -D warnings -D clippy::pedantic
- ✅ cargo fmt --all -- --check
- ✅ git commit + push (9bf1302)
