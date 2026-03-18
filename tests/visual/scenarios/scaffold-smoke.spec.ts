/**
 * Phase 7 scaffold smoke test.
 *
 * Verifies that the Playwright project is correctly configured and can
 * navigate to the Slack workspace login page. This test does NOT require
 * an authenticated session — it runs without `storageState` to confirm
 * basic browser launch and navigation work.
 *
 * Part of scenario S-T3-001 (workspace navigation baseline).
 */
import { test, expect } from '@playwright/test';
import { captureStep, isVisibleWithin } from '../helpers/screenshot';
import { NAV_SELECTORS } from '../helpers/slack-selectors';

test.use({ storageState: undefined });

test('scaffold: navigate to Slack login page', async ({ page }) => {
  const workspaceUrl = process.env.SLACK_WORKSPACE_URL;
  if (!workspaceUrl) {
    test.skip();
    return;
  }

  await page.goto(`${workspaceUrl}/sign_in_with_password`);

  // Capture the login page screenshot as visual baseline.
  await captureStep(page, 'S-T3-001', 1, 'login-page');

  // Verify the page loaded (email or workspace selector is visible).
  const emailInput = page.locator('input[type="email"], input[data-qa="login_email"]').first();
  const isLoginPage = await isVisibleWithin(emailInput, 10_000);

  if (!isLoginPage) {
    // We may already be redirected to the workspace — check for the sidebar.
    const sidebarVisible = await isVisibleWithin(
      page.locator(NAV_SELECTORS.channelSidebar),
      5_000,
    );
    expect(
      sidebarVisible,
      'Expected either the login email input or the channel sidebar to be visible',
    ).toBe(true);
  }

  await captureStep(page, 'S-T3-001', 2, 'login-page-confirmed');
});
