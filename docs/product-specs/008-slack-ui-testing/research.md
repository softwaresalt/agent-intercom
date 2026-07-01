# Research: Slack UI Automated Testing

**Feature**: 008-slack-ui-testing | **Date**: 2026-03-09

## Research Topics

### R1: Tier 1 — Testing `slack-morphism` Block Kit Types

**Decision**: Test Block Kit builder functions by calling them with representative inputs and asserting the returned `SlackBlock` structures via serialization to JSON, then pattern-matching on expected keys/values.

**Rationale**: The `slack-morphism` types implement `Serialize` but not `PartialEq` for all variants. Serializing to `serde_json::Value` and asserting on JSON structure is the most reliable approach. This is already the pattern used in the existing `blocks_tests.rs` (e.g., `instruction_modal_preserves_callback_id` serializes to JSON and checks for substring presence).

**Alternatives considered**:
- Direct field assertion via pattern matching — rejected because `slack-morphism` types have complex nested structures and not all fields are public.
- Snapshot testing (insta) — considered but adds a new dependency for limited benefit; JSON assertion is sufficient.

### R2: Tier 1 — Simulating Slack Interactive Payloads

**Decision**: Construct `SlackInteractionActionInfo` structs directly (they're public in `slack-morphism`) and pass them to the handler functions (`handle_prompt_action`, `handle_wait_action`, `handle_nudge_action`, `handle_approval_action`). Mock `SlackService` by setting `state.slack = None` and verifying state changes via the in-memory database and oneshot channel resolution.

**Rationale**: The handler functions accept discrete parameters (action, user_id, trigger_id, channel, message, state) — they can be called directly in tests without needing a real Socket Mode connection. The `state.slack = None` path is already handled gracefully in production code (button replacement is skipped when no Slack service is available).

**Alternatives considered**:
- Full event dispatcher simulation (constructing `SlackInteractionEvent` envelopes) — useful for integration tests but heavyweight for unit-level handler testing.
- Mock trait for `SlackService` — would require refactoring production code to use a trait; too invasive for this feature.

### R3: Tier 2 — Live Slack API Testing Strategy

**Decision**: Tier 2 tests use the real `SlackService` (with actual bot/app tokens) to post messages to a dedicated test channel, then verify via `conversations.history` and `conversations.replies`. Interactive payloads are simulated by constructing the same JSON structures the Slack client would send and dispatching them through the server's event handlers.

**Rationale**: Slack does not provide a "click button" API — buttons can only be clicked in the Slack client, which triggers a webhook payload. However, the server receives these payloads via Socket Mode and dispatches them identically regardless of origin. Constructing synthetic interaction payloads and dispatching them through the handler pipeline is functionally equivalent to a real button click from the server's perspective.

**Alternatives considered**:
- Slack test API / sandbox — Slack does not offer an official test/sandbox API for simulating interactive payloads.
- Running a second Slack app instance — unnecessary complexity; synthetic payloads dispatched through handlers are equivalent.

**Limitation**: Tier 2 cannot verify that modals actually render in the client — it can only verify that `views.open` returns success. The modal-in-thread issue specifically requires Tier 3 visual verification.

### R4: Tier 2 — Feature Gating for Live Tests

**Decision**: Live tests are feature-gated behind `#[cfg(feature = "live-slack-tests")]` in `Cargo.toml`. Running `cargo test` without the feature flag skips all Tier 2 tests. Running `cargo test --features live-slack-tests` executes them. Test workspace credentials are read from environment variables (`SLACK_TEST_BOT_TOKEN`, `SLACK_TEST_APP_TOKEN`, `SLACK_TEST_CHANNEL_ID`).

**Rationale**: Feature gates are the standard Rust mechanism for conditional compilation. This ensures Tier 2 tests never accidentally run in CI without credentials, avoiding false failures.

**Alternatives considered**:
- `#[ignore]` attribute with manual `--include-ignored` — less explicit, harder to discover, and doesn't prevent compilation of test code that depends on external types.
- Separate binary/crate — over-engineering for a test module.

### R5: Tier 3 — Playwright for Slack Web Client Automation

**Decision**: Use Playwright (TypeScript) in a standalone `tests/visual/` project. Playwright automates the Slack web client at `app.slack.com` via Chromium. Screenshots are captured at each interaction step. Session cookies are persisted to avoid repeated login.

**Rationale**: Playwright is the industry-standard browser automation tool with excellent screenshot support, DOM inspection, and wait-for-element semantics. Slack's web client is a React application with `data-qa` attributes on many interactive elements, which provides reasonably stable selectors. Playwright's `page.screenshot()` and configurable `waitForSelector` are exactly what's needed for modal detection timeout testing (FR-030).

**Alternatives considered**:
- Selenium/WebDriver — Playwright has better defaults for modern SPAs, built-in auto-wait, and native screenshot support.
- Puppeteer — Chromium-only (same as Playwright default) but fewer features for test reporting.
- Rust headless browser (fantoccini/thirtyfour) — much less mature, poor screenshot and wait semantics, would require significant custom code.
- Cypress — designed for testing apps you own (same-origin), not third-party web apps like Slack.

### R6: Tier 3 — Slack Web Client Authentication

**Decision**: Authenticate via Slack workspace email/password login in the browser, then persist the browser storage state (cookies + localStorage) to `tests/visual/auth/`. Subsequent test runs reuse the stored state, skipping login entirely. If the session expires, the login flow is re-executed.

**Rationale**: Playwright's `storageState` feature natively supports persisting and reloading authentication cookies. A dedicated test Slack account (without 2FA) enables fully automated login. This is the standard Playwright pattern for authenticated testing.

**Alternatives considered**:
- Token injection via cookies — fragile, Slack rotates tokens and uses multiple cookie domains.
- Slack API `xoxc` tokens — undocumented/unsupported, could break at any time.
- 2FA-enabled account with TOTP — adds complexity (TOTP library dependency); a dedicated test account without 2FA is simpler.

### R7: Tier 3 — Slack DOM Selector Strategy

**Decision**: Use Slack's `data-qa` attributes as primary selectors (e.g., `[data-qa="message_container"]`, `[data-qa="message_actions"]`). Fall back to `aria-label` attributes for interactive elements. Wrap all selectors in a `slack-selectors.ts` module so they can be updated in one place when Slack changes its DOM.

**Rationale**: Slack uses `data-qa` attributes extensively in its web client, which are more stable than class names (which are minified/hashed). Centralizing selectors in one file makes maintenance tractable when Slack updates its client.

**Alternatives considered**:
- CSS class selectors — too fragile; Slack uses hashed class names that change between deployments.
- XPath — verbose and brittle; `data-qa` attributes are simpler and more stable.
- Visual AI matching (e.g., Applitools Eyes) — adds a commercial dependency; screenshot comparison with manual review is sufficient for our scale.

### R8: Modal-in-Thread Diagnosis Strategy

**Decision**: Create a dedicated Playwright test (`modal-in-thread.spec.ts`) that performs a controlled comparison:
1. Server posts a prompt message with Refine button as a **top-level** channel message → click Refine → screenshot → verify modal appears
2. Server posts a prompt message with Refine button **in a thread** → click Refine → screenshot → wait 5s → screenshot again → document whether modal appeared

If the modal fails in threads, capture:
- The Slack API response from `views.open` (via server logs)
- The browser DOM state after the click (any error overlays, console errors)
- Screenshots before/during/after the button click

**Rationale**: A controlled A/B comparison (threaded vs. non-threaded) isolates the variable. The 5-second wait distinguishes "slow rendering" from "silent failure." Capturing both API and visual evidence enables root cause diagnosis.
