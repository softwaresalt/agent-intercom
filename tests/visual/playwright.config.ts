import { defineConfig, devices } from '@playwright/test';
import * as dotenv from 'dotenv';
import * as path from 'path';

dotenv.config({ path: path.resolve(__dirname, '.env') });

const reportsDir = path.resolve(__dirname, 'reports');
const testResultsDir = path.resolve(__dirname, 'test-results');
const authStatePath = path.resolve(__dirname, 'auth', 'session.json');

/**
 * Playwright configuration for agent-intercom Tier 3 visual tests.
 *
 * Environment variables (via .env or shell):
 *   SLACK_WORKSPACE_URL  — e.g. https://myteam.slack.com
 *   SLACK_EMAIL          — bot/test-account email address
 *   SLACK_PASSWORD       — login password
 *   SLACK_TEST_CHANNEL   — channel name to navigate to during tests
 *   PLAYWRIGHT_TIMEOUT   — optional override for default action timeout (ms)
 */
export default defineConfig({
  testDir: './scenarios',
  outputDir: testResultsDir,

  /* Run tests sequentially — Slack UI is stateful and cannot run in parallel safely. */
  workers: 1,
  fullyParallel: false,
  retries: 0,

  timeout: parseInt(process.env.PLAYWRIGHT_TIMEOUT ?? '60000', 10),
  expect: {
    timeout: 10_000,
  },

  reporter: [
    ['html', { outputFolder: reportsDir, open: 'never' }],
    ['list'],
  ],

  use: {
    /* Chromium-only — Slack web client is tested against Chromium. */
    ...devices['Desktop Chrome'],
    headless: process.env.PLAYWRIGHT_HEADLESS !== 'false',
    /*
     * Always capture screenshots so every test step is included in the HTML
     * report and the gallery (S-T3-012, FR-029).
     */
    screenshot: 'on',
    video: 'off',
    trace: 'off',
    baseURL: process.env.SLACK_WORKSPACE_URL,
    storageState: authStatePath,
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
  globalTeardown: './helpers/generate-gallery.ts',
});
