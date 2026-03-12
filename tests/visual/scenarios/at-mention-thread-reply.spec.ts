/**
 * Phase 11 — @-Mention Thread Reply Fix: automated visual validation.
 *
 * Validates the fix shipped in commit `480aaab`:
 *   - When a Refine/Resume-with-Instructions button is clicked inside a Slack
 *     thread, the server skips `views.open` and instead posts an @-mention
 *     prompt directly in the thread.
 *   - The operator replies with `@agent-intercom <instructions>` and the
 *     server routes the stripped text to the pending prompt waiter.
 *
 * This suite self-seeds its own fixtures using the Slack Web API so it runs
 * without a prior HITL pass.
 *
 * ## Tests
 *
 * S-T3-AUTO-006 — @-mention prompt text is visible in the seeded thread.
 * S-T3-AUTO-007 — Refine button is visible in the seeded in-thread prompt.
 *
 * ## Environment
 *
 * All tests skip gracefully when `hasAtMentionEnv()` returns false.
 * Required: SLACK_WORKSPACE_URL, SLACK_EMAIL, SLACK_PASSWORD,
 *   SLACK_TEST_CHANNEL, SLACK_TEST_CHANNEL_ID,
 *   SLACK_BOT_TOKEN or SLACK_TEST_BOT_TOKEN.
 *
 * FRs: FR-033, FR-034, FR-035
 * SCs: SC-010, SC-011
 */

import { test, expect } from '@playwright/test';

import { navigateToChannel, navigateToThread, scrollToLatestMessage } from '../helpers/slack-nav';
import { captureElement, captureStep, isVisibleWithin } from '../helpers/screenshot';
import { BUTTON_SELECTORS, MESSAGE_SELECTORS, THREAD_SELECTORS } from '../helpers/slack-selectors';
import {
  SlackFixtureClient,
  hasAtMentionEnv,
  type AtMentionFixtures,
} from '../helpers/slack-fixtures';

const AT_MENTION_SCENARIO = 'S-T3-AUTO-006';
const REFINE_IN_THREAD_SCENARIO = 'S-T3-AUTO-007';

const testChannel = (): string => process.env.SLACK_TEST_CHANNEL ?? 'agent-intercom-test';

let fixtureClient: SlackFixtureClient | null = null;
let fixtures: AtMentionFixtures | null = null;

function envReady(): boolean {
  return hasAtMentionEnv();
}

function requireFixtures(): AtMentionFixtures {
  if (!fixtures) {
    throw new Error('@-mention fixtures were not seeded');
  }
  return fixtures;
}

test.describe('@-mention thread reply fix — automated visual validation', () => {
  test.beforeAll(async () => {
    if (!envReady()) {
      return;
    }
    fixtureClient = SlackFixtureClient.fromEnv();
    fixtures = await fixtureClient.seedAtMentionThreadFixture();
  });

  test.afterAll(async () => {
    if (fixtureClient && fixtures) {
      await fixtureClient.deleteMessages(fixtures.cleanupTs);
    }
  });

  /**
   * S-T3-AUTO-006: Verify the seeded @-mention prompt text is visible in the
   * thread and contains the expected bot-mention marker.
   */
  test(
    'S-T3-AUTO-006: @-mention prompt text visible in thread with @agent-intercom marker',
    async ({ page }) => {
      if (!envReady()) {
        test.skip();
        return;
      }

      const f = requireFixtures();

      await navigateToChannel(page, testChannel());
      await scrollToLatestMessage(page);
      await captureStep(page, AT_MENTION_SCENARIO, 1, 'channel-loaded');

      // Open the thread seeded for this run.
      await navigateToThread(page, f.anchorTs);
      await captureStep(page, AT_MENTION_SCENARIO, 2, 'thread-opened');

      const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
      const panelVisible = await isVisibleWithin(threadPanel, 10_000);
      expect(panelVisible, 'Thread panel must open after navigating to anchor').toBe(true);

      await captureStep(page, AT_MENTION_SCENARIO, 3, 'thread-panel-visible');

      // Locate the @-mention prompt message by searching for the identifying text.
      const atMentionMsg = threadPanel
        .locator(`${MESSAGE_SELECTORS.messageText}:has-text("mentioning @agent-intercom")`)
        .first();

      const msgVisible = await isVisibleWithin(atMentionMsg, 15_000);

      await captureStep(
        page,
        AT_MENTION_SCENARIO,
        4,
        msgVisible ? 'at-mention-prompt-visible' : 'at-mention-prompt-not-found',
      );

      expect(
        msgVisible,
        'S-T3-AUTO-006: @-mention prompt text must be visible in the thread panel',
      ).toBe(true);

      // Verify text contains the @agent-intercom marker.
      const msgText = await atMentionMsg.textContent();
      expect(msgText?.toLowerCase()).toContain('@agent-intercom');

      await captureElement(atMentionMsg, AT_MENTION_SCENARIO, 5, 'at-mention-message-closeup');
      await captureStep(page, AT_MENTION_SCENARIO, 6, 'at-mention-text-verified');

      console.log(
        `[${AT_MENTION_SCENARIO}] @-mention prompt text confirmed: "${msgText?.trim()}"`,
      );
    },
  );

  /**
   * S-T3-AUTO-007: Verify the seeded in-thread prompt has the Refine button
   * visible inside the thread pane (Block Kit renders correctly in threads).
   */
  test(
    'S-T3-AUTO-007: Refine button is visible in seeded in-thread prompt',
    async ({ page }) => {
      if (!envReady()) {
        test.skip();
        return;
      }

      const f = requireFixtures();

      await navigateToChannel(page, testChannel());
      await scrollToLatestMessage(page);
      await captureStep(page, REFINE_IN_THREAD_SCENARIO, 1, 'channel-loaded');

      await navigateToThread(page, f.anchorTs);
      await captureStep(page, REFINE_IN_THREAD_SCENARIO, 2, 'thread-opened');

      const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
      const panelVisible = await isVisibleWithin(threadPanel, 10_000);
      expect(panelVisible, 'Thread panel must be visible').toBe(true);

      await captureStep(page, REFINE_IN_THREAD_SCENARIO, 3, 'thread-panel-open');

      // Locate the Refine button inside the thread panel.
      const refineButton = threadPanel.locator(BUTTON_SELECTORS.refineButton).first();
      const refineVisible = await isVisibleWithin(refineButton, 15_000);

      await captureStep(
        page,
        REFINE_IN_THREAD_SCENARIO,
        4,
        refineVisible ? 'refine-button-visible-in-thread' : 'refine-button-not-found',
      );

      expect(
        refineVisible,
        'S-T3-AUTO-007: Refine button must be visible in the in-thread prompt',
      ).toBe(true);

      await captureElement(refineButton, REFINE_IN_THREAD_SCENARIO, 5, 'refine-button-closeup');
      await captureStep(page, REFINE_IN_THREAD_SCENARIO, 6, 'refine-button-verified');

      console.log(`[${REFINE_IN_THREAD_SCENARIO}] Refine button confirmed visible in thread.`);
    },
  );
});
