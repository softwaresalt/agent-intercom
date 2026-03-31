---
id: TASK-008.05
title: "008 - Live Slack Test Harness"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-008
dependencies: []
ordinal: 8050
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Build the Tier 2 test infrastructure with feature gating.

**Depends on**: Phase 1 (test helpers can be reused).

### Tasks

- [X] **4.1** Add `live-slack-tests` feature flag to `Cargo.toml`
- `[features]` section: `live-slack-tests = []`
- FRs: FR-021

- [X] **4.2** Create `tests/live/mod.rs` with feature gate
- `#![cfg(feature = "live-slack-tests")]`
- Module declarations for all live test files

- [X] **4.3** Create `tests/live/live_helpers.rs`
- `LiveTestConfig` — loads from env vars
- `LiveSlackClient` — wrapper around `reqwest` for Slack Web API
- `post_test_message()` — post to test channel, return ts
- `get_message()` — retrieve via `conversations.history`
- `get_thread_replies()` — retrieve via `conversations.replies`
- `cleanup_test_messages()` — delete test messages after suite
- `assert_blocks_contain()` — verify block structure in API response

- [X] **4.4** Create `tests/live/live_message_tests.rs` (skeleton with 1 smoke test)
- Post a simple message, retrieve via API, verify it exists
- Scenario: S-T2-001 (partial)

### Constitution Gate

- [X] `cargo check --features live-slack-tests` compiles
- [X] Clippy clean with feature flag
- [X] Smoke test passes when credentials are available

---

<!-- SECTION:DESCRIPTION:END -->
