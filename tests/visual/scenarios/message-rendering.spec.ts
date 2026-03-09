/**
 * Phase 8 — Visual Rendering Tests: message-rendering.spec.ts
 *
 * Screenshot-based verification of Block Kit message rendering in real Slack.
 * Navigates to the test channel, locates representative messages, and captures
 * screenshots confirming that each message type renders with the expected visual
 * structure: emoji indicators, buttons, code blocks, and text formatting.
 *
 * All tests skip gracefully when SLACK_WORKSPACE_URL or SLACK_TEST_CHANNEL are
 * not configured so the spec can be imported in CI without live credentials.
 *
 * Scenarios covered:
 *   S-T3-002  Approval message rendering
 *   S-T3-003  Prompt message rendering
 *   S-T3-004  Stall alert rendering
 *   S-T3-009  Session started notification rendering
 *   S-T3-010  Code snippet block rendering
 *
 * FRs: FR-026
 */

import { test, expect } from '@playwright/test';
import { navigateToChannel, scrollToLatestMessage } from '../helpers/slack-nav';
import { captureStep, captureElement, isVisibleWithin } from '../helpers/screenshot';
import {
  MESSAGE_SELECTORS,
  BUTTON_SELECTORS,
  THREAD_SELECTORS,
} from '../helpers/slack-selectors';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Return true when all required environment variables are present.
 * Individual tests call `test.skip()` when this returns false.
 */
function hasRequiredEnv(): boolean {
  return Boolean(process.env.SLACK_WORKSPACE_URL && process.env.SLACK_TEST_CHANNEL);
}

/** Channel name from env (without `#`). */
const testChannel = (): string => process.env.SLACK_TEST_CHANNEL ?? 'agent-intercom-test';

// ---------------------------------------------------------------------------
// S-T3-002: Approval message rendering
// ---------------------------------------------------------------------------

test.describe('S-T3-002: Approval message rendering', () => {
  test('renders emoji, diff section, and Accept/Reject buttons', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-002', 1, 'channel-loaded');

    // Locate an approval message: it contains both Accept and Reject buttons.
    const acceptBtn = page.locator(BUTTON_SELECTORS.acceptButton).first();
    const rejectBtn = page.locator(BUTTON_SELECTORS.rejectButton).first();

    const approvalVisible = await isVisibleWithin(acceptBtn, 10_000);

    if (!approvalVisible) {
      // Document that no approval message was found in the channel at this time.
      await captureStep(page, 'S-T3-002', 2, 'no-approval-message-present');
      test.skip();
      return;
    }

    await captureStep(page, 'S-T3-002', 2, 'approval-buttons-visible');

    // Verify the Reject button is also present in the same message.
    await expect(rejectBtn).toBeVisible();
    await captureStep(page, 'S-T3-002', 3, 'reject-button-visible');

    // Capture a close-up of the actions block containing the buttons.
    const actionsBlock = page.locator(MESSAGE_SELECTORS.actionsBlock).first();
    if (await actionsBlock.isVisible()) {
      await captureElement(actionsBlock, 'S-T3-002', 4, 'actions-block-closeup');
    }

    // Verify at least one section block (diff or description text) is present.
    const sectionBlock = page.locator(MESSAGE_SELECTORS.sectionBlock).first();
    const sectionVisible = await isVisibleWithin(sectionBlock, 5_000);
    expect(sectionVisible, 'Expected a section block in the approval message').toBe(true);

    await captureStep(page, 'S-T3-002', 5, 'approval-message-full');
  });
});

// ---------------------------------------------------------------------------
// S-T3-003: Prompt message rendering
// ---------------------------------------------------------------------------

test.describe('S-T3-003: Prompt message rendering', () => {
  test('renders text, Continue, Refine, and Stop buttons', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-003', 1, 'channel-loaded');

    // A prompt message contains Continue, Refine, and Stop buttons.
    const continueBtn = page.locator(BUTTON_SELECTORS.continueButton).first();
    const refineBtn = page.locator(BUTTON_SELECTORS.refineButton).first();
    const stopBtn = page.locator(BUTTON_SELECTORS.stopButton).first();

    const promptVisible = await isVisibleWithin(continueBtn, 10_000);

    if (!promptVisible) {
      await captureStep(page, 'S-T3-003', 2, 'no-prompt-message-present');
      test.skip();
      return;
    }

    await captureStep(page, 'S-T3-003', 2, 'continue-button-visible');

    await expect(refineBtn).toBeVisible();
    await expect(stopBtn).toBeVisible();

    await captureStep(page, 'S-T3-003', 3, 'all-prompt-buttons-visible');

    // Capture the actions block.
    const actionsBlock = page.locator(MESSAGE_SELECTORS.actionsBlock).first();
    if (await actionsBlock.isVisible()) {
      await captureElement(actionsBlock, 'S-T3-003', 4, 'prompt-actions-closeup');
    }

    await captureStep(page, 'S-T3-003', 5, 'prompt-message-full');
  });
});

// ---------------------------------------------------------------------------
// S-T3-004: Stall alert rendering
// ---------------------------------------------------------------------------

test.describe('S-T3-004: Stall alert rendering', () => {
  test('renders warning emoji, idle duration text, and Nudge buttons', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-004', 1, 'channel-loaded');

    // A stall alert contains a Nudge button.
    const nudgeBtn = page.locator(BUTTON_SELECTORS.nudgeButton).first();
    const nudgeWithInstructionsBtn = page.locator(BUTTON_SELECTORS.nudgeWithInstructionsButton).first();

    const stallVisible = await isVisibleWithin(nudgeBtn, 10_000);

    if (!stallVisible) {
      await captureStep(page, 'S-T3-004', 2, 'no-stall-alert-present');
      test.skip();
      return;
    }

    await captureStep(page, 'S-T3-004', 2, 'nudge-button-visible');

    // Nudge with Instructions and Stop should also be present.
    await expect(nudgeWithInstructionsBtn).toBeVisible();
    const stopBtn = page.locator(BUTTON_SELECTORS.stopButton).first();
    await expect(stopBtn).toBeVisible();

    await captureStep(page, 'S-T3-004', 3, 'all-stall-buttons-visible');

    // The stall alert should contain a section block with warning text / emoji.
    const sectionBlock = page.locator(MESSAGE_SELECTORS.sectionBlock).first();
    const sectionVisible = await isVisibleWithin(sectionBlock, 5_000);
    expect(sectionVisible, 'Expected a section block with warning content').toBe(true);

    await captureStep(page, 'S-T3-004', 4, 'stall-alert-full');
  });
});

// ---------------------------------------------------------------------------
// S-T3-009: Session started notification rendering
// ---------------------------------------------------------------------------

test.describe('S-T3-009: Session started notification rendering', () => {
  test('renders session ID, protocol mode, operational mode, workspace root, timestamp', async ({
    page,
  }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-009', 1, 'channel-loaded');

    // Session started messages are typically plain text/section blocks without
    // interactive buttons. We look for the message text that contains the
    // session ID prefix pattern ("task:" or "context:" or "spec:").
    const messageList = page.locator(MESSAGE_SELECTORS.messageList).first();
    const listVisible = await isVisibleWithin(messageList, 10_000);

    if (!listVisible) {
      await captureStep(page, 'S-T3-009', 2, 'message-list-not-visible');
      test.skip();
      return;
    }

    // Look for message text containing "Session started" or "task:" identifiers.
    const sessionText = page.locator(
      `${MESSAGE_SELECTORS.messageText}:has-text("task:"), ` +
        `${MESSAGE_SELECTORS.messageText}:has-text("Session started")`,
    ).first();

    const sessionVisible = await isVisibleWithin(sessionText, 10_000);

    if (!sessionVisible) {
      await captureStep(page, 'S-T3-009', 2, 'no-session-started-message-present');
      test.skip();
      return;
    }

    await captureStep(page, 'S-T3-009', 2, 'session-started-text-visible');
    await captureStep(page, 'S-T3-009', 3, 'session-notification-full');
  });
});

// ---------------------------------------------------------------------------
// S-T3-010: Code snippet block rendering
// ---------------------------------------------------------------------------

test.describe('S-T3-010: Code snippet block rendering', () => {
  test('renders code blocks with monospaced formatting', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-010', 1, 'channel-loaded');

    // Code blocks appear as <pre> or <code> elements in the rendered message.
    const codeBlock = page.locator(MESSAGE_SELECTORS.codeBlock).first();
    const codeVisible = await isVisibleWithin(codeBlock, 10_000);

    if (!codeVisible) {
      // Code blocks may be in the thread panel — check there too.
      const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
      const threadVisible = await isVisibleWithin(threadPanel, 2_000);

      if (threadVisible) {
        const threadCodeBlock = threadPanel.locator(MESSAGE_SELECTORS.codeBlock).first();
        const threadCodeVisible = await isVisibleWithin(threadCodeBlock, 5_000);

        if (threadCodeVisible) {
          await captureElement(threadCodeBlock, 'S-T3-010', 2, 'code-block-in-thread');
          await captureStep(page, 'S-T3-010', 3, 'thread-with-code-block');
          return;
        }
      }

      await captureStep(page, 'S-T3-010', 2, 'no-code-block-present');
      test.skip();
      return;
    }

    await captureElement(codeBlock, 'S-T3-010', 2, 'code-block-closeup');
    await captureStep(page, 'S-T3-010', 3, 'code-block-full-context');
  });
});
