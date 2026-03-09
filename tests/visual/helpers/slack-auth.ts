/**
 * Slack authentication helper for Playwright visual tests.
 *
 * This module serves as both:
 *  - The Playwright globalSetup function (called once before all tests)
 *  - A standalone helper imported by individual spec files
 *
 * It navigates to the Slack workspace, performs the email/password login flow,
 * and persists the resulting browser session to `auth/session.json` so that
 * all subsequent test projects can reuse authenticated storage state.
 *
 * Environment variables required:
 *   SLACK_WORKSPACE_URL  — e.g. https://myteam.slack.com
 *   SLACK_EMAIL          — bot/test-account email address
 *   SLACK_PASSWORD       — bot/test-account password
 */
import { chromium, type FullConfig } from '@playwright/test';
import * as path from 'path';
import * as fs from 'fs';

const AUTH_DIR = path.resolve(__dirname, '..', 'auth');
const SESSION_FILE = path.join(AUTH_DIR, 'session.json');

/** Milliseconds to wait for Slack's SPA to settle after navigation. */
const SLACK_LOAD_TIMEOUT = 15_000;

/** Ensure the auth directory exists. */
function ensureAuthDir(): void {
  if (!fs.existsSync(AUTH_DIR)) {
    fs.mkdirSync(AUTH_DIR, { recursive: true });
  }
}

/**
 * Perform the Slack email/password sign-in flow.
 *
 * Slack's login page at `<workspace>/sign_in_with_password` accepts a two-step
 * form: first the email address is submitted, then the password on the next
 * screen. This helper handles both steps and waits until the workspace
 * channel list is visible before returning.
 *
 * @param workspaceUrl - Base URL of the Slack workspace (e.g. https://myteam.slack.com)
 * @param email        - Login email address
 * @param password     - Login password
 */
export async function signInToSlack(
  workspaceUrl: string,
  email: string,
  password: string,
): Promise<void> {
  ensureAuthDir();

  const browser = await chromium.launch({ headless: process.env.PLAYWRIGHT_HEADLESS !== 'false' });
  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    // Navigate to the workspace sign-in page.
    await page.goto(`${workspaceUrl}/sign_in_with_password`, {
      waitUntil: 'networkidle',
      timeout: SLACK_LOAD_TIMEOUT,
    });

    // Step 1 — Enter email address.
    const emailInput = page.locator('input[data-qa="login_email"], input[type="email"]').first();
    await emailInput.waitFor({ state: 'visible', timeout: 10_000 });
    await emailInput.fill(email);

    // Some Slack workspaces show a "Continue" button before the password field.
    const continueBtn = page.locator('button[data-qa="submit_button"], button:has-text("Continue")').first();
    if (await continueBtn.isVisible({ timeout: 2_000 }).catch(() => false)) {
      await continueBtn.click();
    }

    // Step 2 — Enter password.
    const passwordInput = page.locator('input[data-qa="login_password"], input[type="password"]').first();
    await passwordInput.waitFor({ state: 'visible', timeout: 10_000 });
    await passwordInput.fill(password);

    // Submit the login form.
    const signInBtn = page.locator(
      'button[data-qa="signin_button"], button:has-text("Sign In"), button[type="submit"]',
    ).first();
    await signInBtn.click();

    // Wait for the workspace to load — the channel sidebar is the reliable indicator.
    await page.waitForSelector(
      '[data-qa="channel_sidebar"], .p-channel_sidebar, #channels-list',
      { timeout: SLACK_LOAD_TIMEOUT },
    );

    // Persist authenticated cookies and storage to disk.
    await context.storageState({ path: SESSION_FILE });
    console.log(`[slack-auth] Session saved to ${SESSION_FILE}`);
  } finally {
    await context.close();
    await browser.close();
  }
}

/**
 * Playwright globalSetup entry point.
 *
 * Called automatically by Playwright before any test project runs. Reads
 * credentials from environment variables and delegates to `signInToSlack`.
 *
 * Skips auth if `auth/session.json` already exists and `PLAYWRIGHT_FORCE_AUTH`
 * is not set — useful for rapid local iteration when the session is still valid.
 */
export default async function globalSetup(_config: FullConfig): Promise<void> {
  const workspaceUrl = process.env.SLACK_WORKSPACE_URL;
  const email = process.env.SLACK_EMAIL;
  const password = process.env.SLACK_PASSWORD;

  if (!workspaceUrl || !email || !password) {
    console.warn(
      '[slack-auth] Missing SLACK_WORKSPACE_URL, SLACK_EMAIL, or SLACK_PASSWORD — ' +
        'skipping authentication. Visual tests requiring login will fail.',
    );
    return;
  }

  const forceAuth = process.env.PLAYWRIGHT_FORCE_AUTH === 'true';
  if (!forceAuth && fs.existsSync(SESSION_FILE)) {
    console.log('[slack-auth] Existing session found — skipping re-auth. Set PLAYWRIGHT_FORCE_AUTH=true to re-authenticate.');
    return;
  }

  await signInToSlack(workspaceUrl, email, password);
}
