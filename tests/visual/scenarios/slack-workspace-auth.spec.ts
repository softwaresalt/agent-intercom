import { test, expect } from '@playwright/test';

import { waitForSlackWorkspace } from '../helpers/slack-auth';
import { COMPOSER_SELECTORS, MESSAGE_SELECTORS, NAV_SELECTORS } from '../helpers/slack-selectors';

const WORKSPACE_READY_SELECTOR = [
  NAV_SELECTORS.channelSidebar,
  NAV_SELECTORS.quickSwitcher,
  MESSAGE_SELECTORS.messageList,
  COMPOSER_SELECTORS.composerInput,
  '.p-message_pane__content',
  'input[aria-label="Search"]',
].join(', ');

test('live Slack auth reaches the authenticated workspace shell', async ({ page }) => {
  const workspaceUrl = process.env.SLACK_WORKSPACE_URL;
  if (!workspaceUrl) {
    test.skip();
    return;
  }

  await page.goto(`${workspaceUrl}/messages`, { waitUntil: 'domcontentloaded' });
  await waitForSlackWorkspace(page, 45_000);

  await expect(page.locator(WORKSPACE_READY_SELECTOR).first()).toBeVisible();
});
