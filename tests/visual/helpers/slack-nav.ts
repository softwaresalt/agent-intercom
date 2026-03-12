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
  // Ensure we are on the Slack workspace. When a test starts from a fresh
  // browser context with a stored session the page may still be at about:blank.
  const currentUrl = page.url();
  if (!currentUrl.includes('slack.com')) {
    const workspaceUrl = process.env.SLACK_WORKSPACE_URL ?? 'https://app.slack.com';
    await page.goto(workspaceUrl, { waitUntil: 'domcontentloaded' });
  }

  // Strategy 0: Slack sets the browser tab title to the current channel name once the
  // SPA has finished routing. Poll briefly so the SPA has time to restore the
  // last-visited channel from session storage before trying harder strategies.
  const onTarget = await page
    .waitForFunction(
      (name: string) => document.title.toLowerCase().includes(name.toLowerCase()),
      channelName,
      { timeout: 2_000 },
    )
    .then(() => true)
    .catch(() => false);
  if (onTarget) {
    return;
  }

  // Dismiss any open search overlay from a prior strategy attempt.
  const searchOverlayClose = page.locator(
    '[data-qa="search_modal_close"], button[aria-label="Close"], [data-qa="close_search"]',
  ).first();
  if (await searchOverlayClose.isVisible({ timeout: 500 }).catch(() => false)) {
    await searchOverlayClose.click();
    await page.waitForTimeout(300);
  }

  // Strategy 1: Direct URL navigation — most reliable, no DOM selector guessing.
  // Slack routes /{workspace}/messages/{channel-name} to the channel view.
  const workspaceBase = (() => {
    const wsUrl = process.env.SLACK_WORKSPACE_URL;
    if (wsUrl) {
      return wsUrl.replace(/\/$/, '');
    }
    // Derive from current URL, dropping any client path
    return page.url().replace(/\/client\/.*/, '').replace(/\/$/, '');
  })();

  try {
    await page.goto(`${workspaceBase}/messages/${channelName}`, {
      waitUntil: 'domcontentloaded',
      timeout: 20_000,
    });
    // Give Slack a moment to settle, then check the URL — if Slack redirected us
    // into a client path we are in the workspace and can proceed immediately.
    await page.waitForTimeout(2_000);
    const finalUrl = page.url();
    if (
      finalUrl.includes('slack.com') &&
      !finalUrl.includes('/signin') &&
      !finalUrl.includes('/error') &&
      !finalUrl.includes('/landing')
    ) {
      // Non-fatal: if the channel doesn't load in time, fall through to sidebar/search.
      const loaded = await waitForChannelLoad(page).then(() => true).catch(() => false);
      // Also accept a title-based confirmation — guards against stale data-qa selectors.
      const titleOk = await page
        .waitForFunction(
          (name: string) => document.title.toLowerCase().includes(name.toLowerCase()),
          channelName,
          { timeout: 5_000 },
        )
        .then(() => true)
        .catch(() => false);
      if (loaded || titleOk) {
        return;
      }
    }
  } catch {
    // URL navigation failed; fall through to DOM-based strategies.
  }

  // Strategy 2: click the channel name in the sidebar.
  // Slack renders sidebar items as [role="treeitem"], not as <a> links. Use a filter
  // on the text content so we match the exact channel name without partial matches.
  const sidebarItem = page
    .locator('[role="treeitem"]')
    .filter({ hasText: new RegExp(`^\\s*${channelName}\\s*$`, 'i') })
    .or(page.getByRole('link', { name: new RegExp(`\\b${channelName}\\b`, 'i') }))
    .first();

  const sidebarVisible = await sidebarItem
    .waitFor({ state: 'visible', timeout: 4_000 })
    .then(() => true)
    .catch(() => false);
  if (sidebarVisible) {
    await sidebarItem.click();
    await waitForChannelLoad(page);
    return;
  }

  // Strategy 3: click the Slack search bar (top navigation, not Ctrl+K).
  // In newer Slack the search area is a button that opens a search overlay on click.
  const searchTrigger = page
    .locator(
      [
        '[data-qa="top_nav_search"]',
        '[data-qa="search_input_wrapper"]',
        '[aria-label*="Search"]',
        'button:has-text("Search")',
        '[placeholder*="Search"]',
      ].join(', '),
    )
    .first();

  const searchVisible = await searchTrigger
    .waitFor({ state: 'visible', timeout: 3_000 })
    .then(() => true)
    .catch(() => false);
  if (searchVisible) {
    await searchTrigger.click();
    // After clicking the trigger an actual input should appear.
    // Use a non-fatal wait so a changed Slack DOM falls through to Strategy 4.
    const searchInput = page
      .locator('input[type="search"], input[role="combobox"], [data-qa="search_input"]')
      .first();
    const searchInputVisible = await searchInput
      .waitFor({ state: 'visible', timeout: 5_000 })
      .then(() => true)
      .catch(() => false);
    if (searchInputVisible) {
      await searchInput.fill(channelName);
      const result = page
        .locator(
          '[data-qa="channel_search_result_item"], .c-search_autocomplete__item, [role="option"]',
        )
        .first();
      const resultVisible = await result
        .waitFor({ state: 'visible', timeout: 8_000 })
        .then(() => true)
        .catch(() => false);
      if (resultVisible) {
        await result.click();
        await waitForChannelLoad(page);
        return;
      }
    }
    // Input or result did not appear — dismiss the overlay and fall through to Ctrl+K.
    await page.keyboard.press('Escape').catch(() => undefined);
    await page.waitForTimeout(300);
  }

  // Strategy 4: Ctrl+K / Ctrl+P quick-switcher — works across all Slack versions.
  // Try both Ctrl+K (standard) and Ctrl+P (some Slack builds) before giving up.
  for (const shortcut of ['Control+K', 'Control+P']) {
    await page.keyboard.press(shortcut);
    const switcher = page
      .locator(
        [
          '[data-qa="channel_search_input"]',
          '[placeholder="Jump to..."]',
          '[placeholder*="Find or start"]',
          '[aria-label*="quick switcher"]',
          // Newer Slack replaces the quick-switcher with a floating input.
          'input[placeholder*="jump"]',
          'input[placeholder*="search"]',
          '[data-qa="quick_switcher_input"]',
        ].join(', '),
      )
      .first();
    const switcherVisible = await switcher
      .waitFor({ state: 'visible', timeout: 6_000 })
      .then(() => true)
      .catch(() => false);
    if (switcherVisible) {
      await switcher.fill(channelName);
      const switcherResult = page
        .locator(
          '[data-qa="channel_search_result_item"], .c-search_autocomplete__item, [role="option"]',
        )
        .first();
      await switcherResult.waitFor({ state: 'visible', timeout: 8_000 });
      await switcherResult.click();
      await waitForChannelLoad(page);
      return;
    }
    // Close the overlay if the shortcut opened something unexpected.
    await page.keyboard.press('Escape').catch(() => undefined);
    await page.waitForTimeout(200);
  }

  throw new Error(
    `navigateToChannel: all navigation strategies failed for channel "${channelName}". ` +
      'Check SLACK_WORKSPACE_URL and Slack DOM selectors.',
  );
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
  // Wait for any recognisable channel-is-loaded indicator. Slack versions differ
  // in which data-qa attributes they expose so we check several in parallel.
  const channelIndicator = page.locator(
    [
      '[data-qa="message_list"]',
      '.p-message_pane__content',
      '[data-qa="message_input"]',
      '[data-qa="texty_compose_placeholder"]',
      '[data-qa="channel_header_container"]',
      // Slack uses a Quill contenteditable editor with data-placeholder
      '[data-placeholder*="Message"]',
      'div.ql-editor[contenteditable="true"]',
      '[contenteditable="true"][data-placeholder]',
    ].join(', '),
  );
  await channelIndicator.first().waitFor({
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
  // Click "Jump to present" if visible — only match the dedicated jump-to-present
  // button, not generic "new message" text that matches the Slack "New" divider.
  const jumpToPresent = page.locator(
    '[data-qa="jump_to_present_button"], .c-message_pane__jump_btn',
  );
  if (await jumpToPresent.first().isVisible({ timeout: 1_500 }).catch(() => false)) {
    await jumpToPresent.first().click().catch(() => undefined);
    const loaded = await waitForChannelLoad(page).then(() => true).catch(() => false);
    if (loaded) {
      return;
    }
    // Click did not land — fall through to keyboard scroll.
  }

  // Try to focus the Quill editor (the message composer) and then press Escape so
  // focus moves back to the channel body, then press End to scroll to the bottom.
  const editor = page.locator('div.ql-editor[contenteditable="true"], [data-placeholder*="Message"]').first();
  const editorVisible = await editor.isVisible({ timeout: 1_000 }).catch(() => false);
  if (editorVisible) {
    await editor.click();
    await page.keyboard.press('Escape');
  }

  // Pressing End in the Slack client scrolls the active channel to the bottom.
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
