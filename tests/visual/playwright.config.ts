import { defineConfig, devices } from '@playwright/test';
import * as dotenv from 'dotenv';
import * as path from 'path';

dotenv.config({ path: path.resolve(__dirname, '.env') });

/**
 * Playwright configuration for agent-intercom Tier 3 visual tests.
 *
 * Environment variables (via .env or shell):
 *   SLACK_WORKSPACE_URL  — e.g. https://myteam.slack.com
 *   SLACK_EMAIL          — bot/test-account email address
 *   SLACK_PASSWORD       — bot/test-account password
 *   SLACK_TEST_CHANNEL   — channel name to navigate to during tests
 *   PLAYWRIGHT_TIMEOUT   — optional override for default action timeout (ms)
 */
export default defineConfig({
  testDir: './scenarios',
  outputDir: './test-results',

  /* Run tests sequentially — Slack UI is stateful and cannot run in parallel safely. */
  workers: 1,
  fullyParallel: false,
  retries: 0,

  timeout: parseInt(process.env.PLAYWRIGHT_TIMEOUT ?? '60000', 10),
  expect: {
    timeout: 10_000,
  },

  reporter: [
    ['html', { outputFolder: 'reports', open: 'never' }],
    ['list'],
  ],

  use: {
    /* Chromium-only — Slack web client is tested against Chromium. */
    ...devices['Desktop Chrome'],
    headless: process.env.PLAYWRIGHT_HEADLESS !== 'false',
    screenshot: 'only-on-failure',
    video: 'off',
    trace: 'off',
    baseURL: process.env.SLACK_WORKSPACE_URL,
    storageState: 'auth/session.json',
  },

  projects: [
    {
      name: 'setup',
      testMatch: /global\.setup\.ts/,
      use: { storageState: undefined },
    },
    {
      name: 'visual',
      testMatch: /\.spec\.ts/,
      dependencies: ['setup'],
    },
  ],

  globalSetup: './helpers/slack-auth.ts',
});
