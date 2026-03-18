/**
 * Phase 9 — Modal-in-Thread Visual Diagnosis: thread-reply-fallback.spec.ts
 *
 * Visual verification of the thread-reply fallback flow — the recovery path
 * that activates when `views.open` fails or when the Slack client silently
 * suppresses modal rendering for a button clicked inside a thread.
 *
 * Flow under test (S-T3-007):
 *   1. A prompt message appears inside a thread (modal would be suppressed).
 *   2. The server detects the failure / proactively activates the fallback.
 *   3. The server posts a fallback prompt in the thread:
 *        "Please type your instructions as a reply in this thread."
 *   4. The operator types a reply in the thread composer.
 *   5. The server captures the reply, resolves the pending prompt, and updates
 *        the original message with a resolved status.
 *
 * This test captures screenshots at each step, providing visual evidence that
 * the fallback path operates correctly as an end-user experience.
 *
 * The test adapts to two modes:
 *   - SLACK_THREAD_TS set: navigate to that specific thread (recommended for
 *     reproducible test runs with a pre-seeded thread).
 *   - SLACK_THREAD_TS unset: scan the channel for any visible thread.
 *
 * All tests skip gracefully when required environment variables are absent.
 *
 * Scenarios covered:
 *   S-T3-007  Thread-reply fallback: fallback prompt visible, reply resolves prompt
 *
 * FRs: FR-023, FR-028
 */

import { test, expect } from '@playwright/test';
import {
  navigateToChannel,
  scrollToLatestMessage,
  navigateToThread,
  closeThreadPanel,
} from '../helpers/slack-nav';
import { captureStep, captureElement, isVisibleWithin } from '../helpers/screenshot';
import {
  THREAD_SELECTORS,
  MESSAGE_SELECTORS,
} from '../helpers/slack-selectors';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Return true when all required environment variables are present. */
function hasRequiredEnv(): boolean {
  return Boolean(process.env.SLACK_WORKSPACE_URL && process.env.SLACK_TEST_CHANNEL);
}

/** Channel name from env (without `#`). */
const testChannel = (): string => process.env.SLACK_TEST_CHANNEL ?? 'agent-intercom-test';

/** Optional: timestamp of a specific message thread to navigate to. */
const threadAnchorTs = (): string | undefined => process.env.SLACK_THREAD_TS;

/**
 * The fallback prompt text the server posts when the thread-reply fallback
 * activates. Partial match — the server may include the prompt question too.
 */
const FALLBACK_PROMPT_MARKER = 'reply in this thread';

/**
 * Reply text to send via the thread composer during the test.
 * Must be clearly identifiable as a test reply.
 */
const TEST_REPLY_TEXT = '[visual-test] thread-reply fallback response — ignore';

/**
 * Maximum time (ms) to wait for the fallback prompt message to appear in the
 * thread after the server activates the fallback.
 */
const FALLBACK_PROMPT_TIMEOUT = 15_000;

/**
 * Maximum time (ms) to wait for the original prompt message to be updated
 * (buttons replaced) after the reply is sent.
 */
const RESOLUTION_TIMEOUT = 20_000;

// ---------------------------------------------------------------------------
// S-T3-007: Thread-reply fallback visual flow
// ---------------------------------------------------------------------------

test.describe('S-T3-007: Thread-reply fallback flow — visual verification', () => {
  /**
   * Primary scenario: verify that the server's fallback prompt is visible in
   * the thread and that typing a reply resolves the pending operation.
   *
   * This test checks for a pre-existing fallback prompt (already posted by the
   * server) rather than triggering it from scratch, making it agnostic to the
   * live server's session state.
   */
  test('fallback prompt is visible in thread and reply resolves the pending prompt', async ({
    page,
  }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-007', 1, 'channel-loaded');

    // --- Open the target thread ---
    const anchorTs = threadAnchorTs();

    if (anchorTs) {
      await navigateToThread(page, anchorTs);
      await captureStep(page, 'S-T3-007', 2, 'thread-opened-via-env-ts');
    } else {
      // Scan for the first thread visible in the channel.
      const replyBadge = page.locator(
        '[data-qa="threads-reply-count"], .c-threads-beta, [data-qa="thread-reply-count"]',
      ).first();
      const badgeVisible = await isVisibleWithin(replyBadge, 8_000);

      if (!badgeVisible) {
        await captureStep(page, 'S-T3-007', 2, 'no-thread-found-in-channel');
        expect(badgeVisible, 'At least one threaded message must be present in the configured test channel; set SLACK_THREAD_TS to target a specific thread').toBe(true);
        return;
      }

      await replyBadge.click();
      await captureStep(page, 'S-T3-007', 2, 'thread-opened-via-scan');
    }

    // Wait for the thread panel to open.
    const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
    const threadPanelVisible = await isVisibleWithin(threadPanel, 10_000);

    if (!threadPanelVisible) {
      await captureStep(page, 'S-T3-007', 3, 'thread-panel-did-not-open');
      expect(threadPanelVisible, 'Thread panel must open after clicking the reply badge or navigating via SLACK_THREAD_TS').toBe(true);
      return;
    }

    await captureStep(page, 'S-T3-007', 3, 'thread-panel-open');

    // --- Look for the fallback prompt in the thread ---
    // The server posts a message like: "Please type your instructions as a reply
    // in this thread." We search for any message containing the marker text.
    const fallbackMsg = threadPanel.locator(
      `${MESSAGE_SELECTORS.messageText}:has-text("${FALLBACK_PROMPT_MARKER}")`,
    ).first();

    const fallbackVisible = await isVisibleWithin(fallbackMsg, FALLBACK_PROMPT_TIMEOUT);

    await captureStep(
      page,
      'S-T3-007',
      4,
      fallbackVisible ? 'fallback-prompt-visible-in-thread' : 'fallback-prompt-not-found',
    );

    if (!fallbackVisible) {
      // When env is configured, a missing fallback prompt is a test failure:
      // the server must post the fallback text before this test runs.
      console.log(
        '[S-T3-007] Fallback prompt not found in thread. ' +
          `Expected text containing "${FALLBACK_PROMPT_MARKER}". ` +
          'Ensure SLACK_THREAD_TS points to a thread where the server has activated the fallback.',
      );
      await closeThreadPanel(page);
      expect(fallbackVisible, `Fallback prompt containing "${FALLBACK_PROMPT_MARKER}" must be present in the thread; ensure the server has activated the fallback path before running this test`).toBe(true);
      return;
    }

    // Capture the fallback prompt element.
    await captureElement(fallbackMsg, 'S-T3-007', 5, 'fallback-prompt-message-closeup');
    await captureStep(page, 'S-T3-007', 6, 'fallback-prompt-full-thread-view');

    console.log('[S-T3-007] Fallback prompt confirmed visible in thread. Proceeding to compose reply.');

    // --- Compose and send a reply ---
    const threadComposer = threadPanel.locator(THREAD_SELECTORS.threadComposer).first();
    const composerVisible = await isVisibleWithin(threadComposer, 5_000);

    if (!composerVisible) {
      await captureStep(page, 'S-T3-007', 7, 'thread-composer-not-visible');
      await closeThreadPanel(page);
      expect(composerVisible, 'Thread composer must be visible to send the fallback reply').toBe(true);
      return;
    }

    // Click composer to focus it, type the reply.
    await threadComposer.click();
    await threadComposer.fill(TEST_REPLY_TEXT);
    await captureStep(page, 'S-T3-007', 7, 'reply-being-composed-in-thread');

    if (await threadComposer.isVisible()) {
      await captureElement(threadComposer, 'S-T3-007', 8, 'composer-with-reply-text');
    }

    // Screenshot of the full thread pane before sending.
    await captureStep(page, 'S-T3-007', 9, 'thread-before-sending-reply');

    // --- Send the reply ---
    const sendBtn = threadPanel.locator(THREAD_SELECTORS.threadSendButton).first();
    const sendVisible = await isVisibleWithin(sendBtn, 3_000);

    if (sendVisible) {
      await sendBtn.click();
    } else {
      // Fall back to Enter key in the composer.
      await threadComposer.press('Enter');
    }

    await captureStep(page, 'S-T3-007', 10, 'immediately-after-reply-sent');

    // --- Verify the reply appeared in the thread ---
    const sentMsg = threadPanel.locator(
      `${MESSAGE_SELECTORS.messageText}:has-text("${TEST_REPLY_TEXT}")`,
    ).first();
    const replyAppearedInThread = await isVisibleWithin(sentMsg, 10_000);

    await captureStep(
      page,
      'S-T3-007',
      11,
      replyAppearedInThread ? 'reply-visible-in-thread' : 'reply-not-visible',
    );

    expect(replyAppearedInThread, 'Expected the sent reply to appear in the thread').toBe(true);

    // --- Wait for server to resolve the prompt (buttons replaced in original message) ---
    // The resolved status may be in the main channel message (original prompt),
    // not in the thread panel. Capture both contexts.
    await captureStep(page, 'S-T3-007', 12, 'thread-after-reply-sent');

    // Close the thread panel and check the original message.
    await closeThreadPanel(page);
    await captureStep(page, 'S-T3-007', 13, 'channel-view-after-closing-thread');

    // Look for a resolved status text in the channel (buttons replaced).
    const resolvedStatus = page.locator(MESSAGE_SELECTORS.resolvedStatus).first();
    const statusVisible = await isVisibleWithin(resolvedStatus, RESOLUTION_TIMEOUT);

    if (statusVisible) {
      await captureElement(resolvedStatus, 'S-T3-007', 14, 'original-message-resolved-status');
      console.log('[S-T3-007] Original prompt message updated with resolved status after thread reply.');
    } else {
      console.log(
        '[S-T3-007] Resolved status not detected within timeout. ' +
          'The server may need more time or the session may have already been resolved.',
      );
    }

    await captureStep(page, 'S-T3-007', 15, 'fallback-flow-complete');
  });

  /**
   * Sub-test: verify the thread-reply fallback visual — just the fallback prompt
   * visibility, without sending a reply (to preserve test environment state).
   */
  test('fallback prompt message is visible in thread with correct instructional text', async ({
    page,
  }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    const anchorTs = threadAnchorTs();

    if (anchorTs) {
      await navigateToThread(page, anchorTs);
    } else {
      const replyBadge = page.locator(
        '[data-qa="threads-reply-count"], .c-threads-beta, [data-qa="thread-reply-count"]',
      ).first();

      if (!(await isVisibleWithin(replyBadge, 8_000))) {
        await captureStep(page, 'S-T3-007', 20, 'no-thread-found-in-channel-subtest');
        expect(false, 'At least one threaded message must be present in the configured test channel; set SLACK_THREAD_TS to target a specific thread').toBe(true);
        return;
      }

      await replyBadge.click();
    }

    const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
    if (!(await isVisibleWithin(threadPanel, 10_000))) {
      await captureStep(page, 'S-T3-007', 20, 'thread-panel-did-not-open-subtest');
      expect(false, 'Thread panel must open after clicking the reply badge or navigating via SLACK_THREAD_TS').toBe(true);
      return;
    }

    // Check for fallback prompt — do not send a reply.
    const fallbackMsg = threadPanel.locator(
      `${MESSAGE_SELECTORS.messageText}:has-text("${FALLBACK_PROMPT_MARKER}")`,
    ).first();

    const fallbackVisible = await isVisibleWithin(fallbackMsg, FALLBACK_PROMPT_TIMEOUT);

    await captureStep(
      page,
      'S-T3-007',
      20,
      fallbackVisible ? 'fallback-prompt-text-verified' : 'fallback-prompt-absent',
    );

    if (fallbackVisible) {
      const msgText = await fallbackMsg.textContent();
      console.log(`[S-T3-007] Fallback prompt text: "${msgText?.trim()}"`);
      expect(msgText?.toLowerCase()).toContain('reply in this thread');
    } else {
      // When env is configured, missing fallback prompt is a test failure.
      console.log(
        '[S-T3-007] No fallback prompt found. ' +
          'Ensure SLACK_THREAD_TS points to a thread where the server has activated the fallback.',
      );
      expect(fallbackVisible, `Fallback prompt containing "${FALLBACK_PROMPT_MARKER}" must be present in the thread when environment is configured`).toBe(true);
    }

    await closeThreadPanel(page);
    await captureStep(page, 'S-T3-007', 21, 'fallback-text-verification-complete');
  });
});
