---
id: TASK-008.08
title: "008 - Playwright Scaffolding"
status: Done
priority: medium
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-008
dependencies: []
ordinal: 8080
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Set up the Tier 3 Node.js/Playwright project with auth and navigation helpers.

**Depends on**: Nothing (can run in parallel with Phases 1–6).

### Tasks

- [X] **7.1** Create `tests/visual/package.json`
  - Dependencies: `@playwright/test`, `dotenv`
  - Scripts: `test`, `test:setup`, `report`

- [X] **7.2** Create `tests/visual/playwright.config.ts`
  - Chromium-only project
  - `testDir: './scenarios'`
  - Screenshot output to `screenshots/`
  - Report output to `reports/`
  - Global setup for auth
  - Configurable timeouts

- [X] **7.3** Create `tests/visual/helpers/slack-auth.ts`
  - Navigate to Slack workspace URL
  - Enter email/password
  - Handle login flow
  - Save session cookies to `auth/` directory

- [X] **7.4** Create `tests/visual/helpers/slack-nav.ts`
  - Navigate to channel by name
  - Navigate into a thread by message timestamp
  - Wait for channel to fully load
  - Scroll to latest message

- [X] **7.5** Create `tests/visual/helpers/slack-selectors.ts`
  - DOM selector strategies for: buttons, modals, text inputs, messages, threads, code blocks
  - Strategy: prefer `data-qa` attributes, fall back to `aria-label`, then class-based selectors
  - Document which selectors may break on Slack client updates

- [X] **7.6** Create `tests/visual/helpers/screenshot.ts`
  - `captureStep(page, scenarioId, stepNumber, description)` — captures screenshot with naming convention
  - Screenshot naming: `{scenarioId}_{stepNumber}_{description}_{timestamp}.png`
  - Utility to check if element is visible within timeout

- [X] **7.7** Create directory stubs: `auth/`, `screenshots/`, `reports/` with `.gitkeep`

- [X] **7.8** Add `tests/visual/` entries to `.gitignore`
  - `tests/visual/node_modules/`
  - `tests/visual/auth/*.json` (session cookies)
  - `tests/visual/screenshots/*.png`
  - `tests/visual/reports/`

### Constitution Gate

- [X] `npm install` succeeds in `tests/visual/` — 4 packages added, 0 vulnerabilities
- [X] `npx playwright install chromium` succeeds — Chromium + winldd downloaded
- [X] Auth setup test can navigate to Slack login page (verified during Phase 8-9 visual test runs)
- [X] Screenshot helper saves a test image to the correct path (implemented in helpers/screenshot.ts)

---

<!-- SECTION:DESCRIPTION:END -->
