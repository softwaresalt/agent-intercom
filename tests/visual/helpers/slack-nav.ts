/**
 * Slack navigation helpers for Playwright visual tests.
 *
 * Provides high-level navigation utilities that abstract away the unstable
 * DOM structure of the Slack web client. All helpers prefer `data-qa`
 * attributes where available and fall back to aria-label or class patterns.
 */
import { type Page } from '@playwright/test';

/** Milliseconds to wait for the channel to finish loading after navigation. */
const CHANNEL_LOAD_TIMEOUT = 15_000;

/** Milliseconds to wait for a thread panel to open after clicking a message. */
const THREAD_OPEN_TIMEOUT = 10_000;

/**
 * Navigate to a Slack channel by its display name (without the `#` prefix).
 *
 * Strategy:
 *  1. Use the keyboard shortcut (Ctrl+K / Cmd+K) to open the quick switcher.
 *  2. Type the channel name and select the first matching result.
 *  3. Wait for the channel's message list to be visible.
 *
 * @param page        - Playwright page instance (must have an authenticated session)
 * @param channelName - Channel name without `#`, e.g. `agent-intercom-test`
 */
export async function navigateToChannel(page: Page, channelName: string): Promise<void> {
  // Open the channel/DM switcher.
  await page.keyboard.press('Control+K');

  const switcher = page.locator(
    '[data-qa="channel_search_input"], [placeholder="Jump to..."], input[aria-label="Search"]',
  ).first();
  await switcher.waitFor({ state: 'visible', timeout: 5_000 });
  await switcher.fill(channelName);

  // Wait for and click the first matching result.
  const result = page
    .locator('[data-qa="channel_search_result_item"], .c-search_autocomplete__item')
    .first();
  await result.waitFor({ state: 'visible', timeout: 5_000 });
  await result.click();

  // Confirm the channel loaded.
  await waitForChannelLoad(page);
}

/**
 * Open a message thread by its Slack timestamp (`ts`).
 *
 * Slack message timestamps in the UI are expressed as `p{ts_without_dot}`.
 * For example, `ts = "1700000000.123456"` → `data-item-key="1700000000.123456"`.
 * This function finds the message by its `data-item-key` attribute and clicks
 * its reply count or thread icon to open the thread panel.
 *
 * @param page - Playwright page instance navigated to the channel containing the message
 * @param ts   - Slack message timestamp in the format returned by `chat.postMessage`
 */
export async function navigateToThread(page: Page, ts: string): Promise<void> {
  // Messages are identified by their timestamp in `data-item-key`.
  const messageRow = page.locator(`[data-item-key="${ts}"]`).first();
  await messageRow.waitFor({ state: 'visible', timeout: CHANNEL_LOAD_TIMEOUT });

  // Hover to reveal the action toolbar, then click the thread/reply button.
  await messageRow.hover();

  const threadBtn = messageRow.locator(
    '[data-qa="start_thread"], [aria-label*="Reply"], [data-qa="message-actions-reply_in_thread"]',
  ).first();

  if (await threadBtn.isVisible({ timeout: 2_000 }).catch(() => false)) {
    await threadBtn.click();
  } else {
    // Fall back to clicking the reply count badge if the action toolbar is not visible.
    const replyCount = messageRow.locator('[data-qa="threads-reply-count"], .c-threads-beta').first();
    await replyCount.click();
  }

  // Wait for the thread panel to open.
  await page.locator('[data-qa="threads_flexpane"], .p-flexpane__container').waitFor({
    state: 'visible',
    timeout: THREAD_OPEN_TIMEOUT,
  });
}

/**
 * Wait for the active channel's message list to fully load.
 *
 * Considers the channel "loaded" when the virtual list container is visible
 * and no loading spinner is present.
 *
 * @param page - Playwright page instance
 */
export async function waitForChannelLoad(page: Page): Promise<void> {
  // Wait for the message list container to appear.
  await page.locator('[data-qa="message_list"], .p-message_pane__content').waitFor({
    state: 'visible',
    timeout: CHANNEL_LOAD_TIMEOUT,
  });

  // Wait until any loading spinner disappears.
  const spinner = page.locator('[data-qa="loading_spinner"], .c-infinite_scroll__loading');
  if (await spinner.isVisible({ timeout: 1_000 }).catch(() => false)) {
    await spinner.waitFor({ state: 'hidden', timeout: CHANNEL_LOAD_TIMEOUT });
  }
}

/**
 * Scroll the active channel's message list to the most recent message.
 *
 * Uses the "Jump to present" button if the channel is not already scrolled
 * to the bottom; otherwise performs a keyboard End scroll.
 *
 * @param page - Playwright page instance
 */
export async function scrollToLatestMessage(page: Page): Promise<void> {
  // Click "Jump to present" button if visible (shown when scrolled up in history).
  const jumpToPresent = page.locator('[data-qa="jump_to_present_button"], .c-message_pane__jump_btn');
  if (await jumpToPresent.isVisible({ timeout: 1_500 }).catch(() => false)) {
    await jumpToPresent.click();
    await waitForChannelLoad(page);
    return;
  }

  // Otherwise, focus the message list and press End to scroll to bottom.
  const messageList = page.locator('[data-qa="message_list"], .p-message_pane__content').first();
  await messageList.focus();
  await page.keyboard.press('End');

  // Brief pause to let virtual scroll settle.
  await page.waitForTimeout(500);
}

/**
 * Wait for the thread panel to close (if open).
 *
 * @param page - Playwright page instance
 */
export async function closeThreadPanel(page: Page): Promise<void> {
  const closeBtn = page.locator('[data-qa="close_flexpane"], [aria-label="Close"]').first();
  if (await closeBtn.isVisible({ timeout: 2_000 }).catch(() => false)) {
    await closeBtn.click();
    await page.locator('[data-qa="threads_flexpane"]').waitFor({
      state: 'hidden',
      timeout: 5_000,
    });
  }
}
