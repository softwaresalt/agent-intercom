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
 *   SLACK_WORKSPACE_URL  - e.g. https://myteam.slack.com
 *   SLACK_EMAIL          - bot/test-account email address
 *   SLACK_PASSWORD       - login password
 *
 * Optional:
 *   PLAYWRIGHT_SKIP_GLOBAL_AUTH=true skips global auth setup. This is useful
 *   for helper-focused specs that do not need a real Slack session.
 */
import { chromium, type Dialog, type FullConfig, type Page } from '@playwright/test';
import * as path from 'path';
import * as fs from 'fs';

import { COMPOSER_SELECTORS, MESSAGE_SELECTORS, NAV_SELECTORS } from './slack-selectors';

const AUTH_DIR = path.resolve(__dirname, '..', 'auth');
const SESSION_FILE = path.join(AUTH_DIR, 'session.json');

/** Milliseconds to wait for Slack navigation pages to settle. */
const SLACK_LOAD_TIMEOUT = 15_000;
/** Milliseconds to wait for the authenticated Slack client shell after sign-in. */
const SLACK_WORKSPACE_TIMEOUT = 60_000;

const BROWSER_HANDOFF_SELECTOR = [
  'a:has-text("Use Slack in your browser")',
  'a:has-text("use Slack in your browser")',
  'a:has-text("Continue in browser")',
  'a:has-text("continue in browser")',
  'button:has-text("Use Slack in your browser")',
  'button:has-text("use Slack in your browser")',
  'button:has-text("Continue in browser")',
  'button:has-text("continue in browser")',
  'a[href*="app.slack.com/client"]',
].join(', ');

const REDIRECT_CANCEL_SELECTOR = [
  '[role="dialog"] button:has-text("Cancel")',
  '[aria-modal="true"] button:has-text("Cancel")',
  'button:has-text("Cancel")',
  'button:has-text("Not now")',
  'button:has-text("Maybe later")',
  'button:has-text("Stay in browser")',
].join(', ');

const WORKSPACE_READY_SELECTOR = [
  NAV_SELECTORS.channelSidebar,
  NAV_SELECTORS.quickSwitcher,
  MESSAGE_SELECTORS.messageList,
  COMPOSER_SELECTORS.composerInput,
  '.p-message_pane__content',
  'input[aria-label="Search"]',
].join(', ');

/** Ensure the auth directory exists. */
function ensureAuthDir(): void {
  if (!fs.existsSync(AUTH_DIR)) {
    fs.mkdirSync(AUTH_DIR, { recursive: true });
  }
}

/**
 * Dismiss the Slack "open the installed app" prompt when it appears.
 * Returns true when a visible cancel-like control was found and clicked.
 */
export async function dismissAppRedirectPromptIfVisible(page: Page): Promise<boolean> {
  const cancelButton = page.locator(REDIRECT_CANCEL_SELECTOR).first();
  const isVisible = await cancelButton.isVisible({ timeout: 1_000 }).catch(() => false);

  if (!isVisible) {
    return false;
  }

  await cancelButton.click();
  await page.waitForTimeout(250);
  return true;
}

/**
 * Click the Slack browser handoff link when the sign-in flow prefers the
 * installed Slack app. Returns true when a matching link or button was found.
 */
export async function continueInBrowserIfPrompted(page: Page): Promise<boolean> {
  const handoffTarget = page.locator(BROWSER_HANDOFF_SELECTOR).first();
  const isVisible = await handoffTarget.isVisible({ timeout: 1_000 }).catch(() => false);

  if (!isVisible) {
    return false;
  }

  const currentUrl = page.url();
  await handoffTarget.click();
  await Promise.race([
    page.waitForURL((url) => url.toString() !== currentUrl, { timeout: 2_000 }),
    handoffTarget.waitFor({ state: 'hidden', timeout: 2_000 }),
    page.locator(WORKSPACE_READY_SELECTOR).first().waitFor({ state: 'visible', timeout: 2_000 }),
  ]).catch(() => undefined);
  await page.waitForLoadState('domcontentloaded', { timeout: 10_000 }).catch(() => undefined);
  return true;
}

async function buildWorkspaceTimeoutDetails(page: Page): Promise<string> {
  const pageTitle = await page.title().catch(() => 'unknown');
  const bodyPreview = await page
    .locator('body')
    .innerText({ timeout: 2_000 })
    .then((text) => text.replace(/\s+/g, ' ').slice(0, 500))
    .catch(() => 'unavailable');

  return `Current URL: ${page.url()} | Title: ${pageTitle} | Body preview: ${bodyPreview}`;
}

/**
 * Wait for Slack's authenticated browser client to become usable.
 *
 * The workspace can arrive either directly after sign-in or through an
 * intermediate handoff page that asks the user to continue in the browser
 * instead of the installed desktop app.
 */
export async function waitForSlackWorkspace(
  page: Page,
  timeout = SLACK_WORKSPACE_TIMEOUT,
): Promise<void> {
  const deadline = Date.now() + timeout;

  while (Date.now() < deadline) {
    const remaining = deadline - Date.now();
    const probeTimeout = Math.max(250, Math.min(1_000, remaining));

    const workspaceVisible = await page
      .locator(WORKSPACE_READY_SELECTOR)
      .first()
      .isVisible({ timeout: probeTimeout })
      .catch(() => false);

    if (workspaceVisible) {
      return;
    }

    const dismissedRedirectPrompt = await dismissAppRedirectPromptIfVisible(page);
    if (dismissedRedirectPrompt) {
      continue;
    }

    const clickedBrowserLink = await continueInBrowserIfPrompted(page);
    if (clickedBrowserLink) {
      const loadTimeout = Math.max(1_000, Math.min(10_000, deadline - Date.now()));
      await page.waitForLoadState('networkidle', { timeout: loadTimeout }).catch(async () => {
        await page.waitForLoadState('domcontentloaded', { timeout: loadTimeout }).catch(
          () => undefined,
        );
      });
      continue;
    }

    await page.waitForTimeout(250);
  }

  const timeoutDetails = await buildWorkspaceTimeoutDetails(page);
  throw new Error(`Timed out waiting for Slack workspace UI. ${timeoutDetails}`);
}

/**
 * Perform the Slack email/password sign-in flow.
 *
 * Slack's login page at `<workspace>/sign_in_with_password` accepts a two-step
 * form: first the email address is submitted, then the password on the next
 * screen. This helper handles both steps and waits until the workspace UI is
 * visible before returning.
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

  page.on('dialog', async (dialog: Dialog) => {
    await dialog.dismiss().catch(() => undefined);
  });

  try {
    await page.goto(`${workspaceUrl}/sign_in_with_password`, {
      waitUntil: 'networkidle',
      timeout: SLACK_LOAD_TIMEOUT,
    });

    const emailInput = page.locator('input[data-qa="login_email"], input[type="email"]').first();
    await emailInput.waitFor({ state: 'visible', timeout: 10_000 });
    await emailInput.fill(email);

    const continueBtn = page
      .locator('button[data-qa="submit_button"], button:has-text("Continue")')
      .first();
    if (await continueBtn.isVisible({ timeout: 2_000 }).catch(() => false)) {
      await continueBtn.click();
    }

    const passwordInput = page
      .locator('input[data-qa="login_password"], input[type="password"]')
      .first();
    await passwordInput.waitFor({ state: 'visible', timeout: 10_000 });
    await passwordInput.fill(password);

    const signInBtn = page
      .locator('button[data-qa="signin_button"], button:has-text("Sign In"), button[type="submit"]')
      .first();
    await signInBtn.click();

    await waitForSlackWorkspace(page);

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
 * is not set - useful for rapid local iteration when the session is still valid.
 */
export default async function globalSetup(_config: FullConfig): Promise<void> {
  if (process.env.PLAYWRIGHT_SKIP_GLOBAL_AUTH === 'true') {
    console.log('[slack-auth] PLAYWRIGHT_SKIP_GLOBAL_AUTH=true - skipping authentication.');
    return;
  }

  const workspaceUrl = process.env.SLACK_WORKSPACE_URL;
  const email = process.env.SLACK_EMAIL;
  const password = process.env.SLACK_PASSWORD;

  if (!workspaceUrl || !email || !password) {
    console.warn(
      '[slack-auth] Missing SLACK_WORKSPACE_URL, SLACK_EMAIL, or SLACK_PASSWORD - ' +
        'skipping authentication. Visual tests requiring login will fail.',
    );
    return;
  }

  const forceAuth = process.env.PLAYWRIGHT_FORCE_AUTH === 'true';
  if (!forceAuth && fs.existsSync(SESSION_FILE)) {
    console.log(
      '[slack-auth] Existing session found - skipping re-auth. Set PLAYWRIGHT_FORCE_AUTH=true to re-authenticate.',
    );
    return;
  }

  await signInToSlack(workspaceUrl, email, password);
}
