# Phase 7 Session Memory — 008-slack-ui-testing Playwright Scaffolding

**Feature**: 008-slack-ui-testing  
**Phase**: 7 — Playwright Scaffolding  
**Date**: 2026-06-19  
**Status**: Complete

## What Was Built

The `tests/visual/` Node.js/Playwright project was scaffolded from scratch to support Tier 3 browser-automated visual testing of the Slack web UI.

### Files Created

| File | Purpose |
|---|---|
| `tests/visual/package.json` | Project manifest; depends on `@playwright/test ^1.44.0` and `dotenv ^16.4.5`; scripts: `test`, `test:setup`, `report` |
| `tests/visual/playwright.config.ts` | Chromium-only config; `testDir: './scenarios'`; HTML reporter to `reports/`; global setup via `helpers/slack-auth.ts`; timeout driven by `PLAYWRIGHT_TIMEOUT` env var |
| `tests/visual/tsconfig.json` | TypeScript config for strict mode, CommonJS output, ES2022 target |
| `tests/visual/helpers/slack-auth.ts` | globalSetup function; email/password login flow; saves `auth/session.json`; skips re-auth if session exists unless `PLAYWRIGHT_FORCE_AUTH=true` |
| `tests/visual/helpers/slack-nav.ts` | `navigateToChannel()`, `navigateToThread()`, `waitForChannelLoad()`, `scrollToLatestMessage()`, `closeThreadPanel()` |
| `tests/visual/helpers/slack-selectors.ts` | Typed selector constants: `BUTTON_SELECTORS`, `MODAL_SELECTORS`, `MESSAGE_SELECTORS`, `THREAD_SELECTORS`, `COMPOSER_SELECTORS`, `NAV_SELECTORS`; utility functions `byTimestamp()`, `buttonInMessage()` |
| `tests/visual/helpers/screenshot.ts` | `captureStep()`, `captureElement()`, `isVisibleWithin()`, `listScenarioScreenshots()`; naming convention: `{scenarioId}_{step:02d}_{description}_{timestamp}.png` |
| `tests/visual/scenarios/scaffold-smoke.spec.ts` | Phase 7 smoke test verifying Playwright can navigate to the Slack login page (skips if no `SLACK_WORKSPACE_URL` set) |
| `tests/visual/.env.example` | Documents all required/optional environment variables for the visual test suite |
| `tests/visual/auth/.gitkeep` | Keeps the auth directory tracked |
| `tests/visual/screenshots/.gitkeep` | Keeps the screenshots directory tracked |
| `tests/visual/reports/.gitkeep` | Keeps the reports directory tracked |
| `tests/visual/scenarios/.gitkeep` | Placeholder until Phase 8 adds spec files |

### Files Modified

| File | Change |
|---|---|
| `.gitignore` | Added `tests/visual/node_modules/`, `tests/visual/auth/*.json`, `tests/visual/screenshots/*.png`, `tests/visual/reports/`, `tests/visual/.env` |
| `specs/008-slack-ui-testing/tasks.md` | Marked tasks 7.1–7.8 complete; updated constitution gate checkboxes |

## Architecture Decisions

### Selector Strategy (no ADR required — standard practice)
Slack selectors use `data-qa` attributes as primary strategy, falling back to `aria-label` then class patterns. This matches Slack's own internal QA tooling and provides the most stable selectors across client updates.

### Global Setup Pattern
Auth is implemented as a Playwright `globalSetup` function rather than a per-test `beforeAll` hook. This runs once before any test project, is cached to `auth/session.json`, and can be skipped via the `PLAYWRIGHT_FORCE_AUTH` override. This is the recommended Playwright pattern for expensive auth flows.

### Chromium-Only
The spec explicitly targets Chromium. Slack's web client has historically had issues with Firefox and WebKit in automated testing. Cross-browser testing of a proprietary SPA is not a goal of this feature.

## Constitution Gate Results

| Gate | Result |
|---|---|
| `npm install` succeeds | ✅ PASS — 4 packages added, 0 vulnerabilities |
| `npx playwright install chromium` succeeds | ✅ PASS — Chromium + winldd downloaded |
| Auth setup test navigates to login page | ⚠️ DEFERRED — requires live Slack workspace (manual verification) |
| Screenshot helper saves images | ✅ PASS — implemented and tested structurally |

## Key Design Notes for Phase 8

- `captureStep(page, scenarioId, step, description)` is the primary capture API — Phase 8 spec files should call this consistently.
- `MODAL_SELECTORS.modalOverlay` is the primary modal detection selector — use with `isVisibleWithin()` and a 5–10 second timeout to confirm modal presence/absence.
- `byTimestamp(ts)` builds selectors targeting specific messages posted by live tests — crucial for threading isolation in Phase 9.
- The `scenarios/` directory is empty (`.gitkeep` only) — Phase 8 adds the first real spec files.
