/**
 * Phase 9 — Modal-in-Thread Visual Diagnosis: modal-wait-instruct-thread.spec.ts
 *
 * Visual A/B test for the "Resume with Instructions" modal — the same diagnosis
 * pattern as modal-in-thread.spec.ts (Refine) but applied to the
 * `wait_resume_instruct` modal path.
 *
 * The "Resume with Instructions" button appears in wait-for-instruction messages.
 * Clicking it calls `views.open` with the `wait_instruct:{session_id}` callback_id.
 * In a top-level message this works correctly; in a thread the Slack client
 * silently suppresses modal rendering — consistent with Phase 6 API evidence
 * (S-T2-011) and the Refine modal finding (S-T2-006).
 *
 * A-side (top-level):
 *   - Navigate to channel
 *   - Find a wait-for-instruction message with "Resume with Instructions" button
 *   - Click it
 *   - Verify modal appears, captures screenshots
 *   - Dismiss without submitting
 *
 * B-side (in-thread):
 *   - Navigate to a thread containing a wait-for-instruction message
 *   - Click "Resume with Instructions"
 *   - Wait MODAL_WAIT_TIMEOUT
 *   - Document whether modal appears or is silently suppressed
 *   - Capture the key diagnostic screenshot
 *
 * All tests skip gracefully when required environment variables are absent.
 *
 * Scenarios covered:
 *   S-T3-011  Wait-resume-instruct modal A/B: top-level vs in-thread
 *
 * FRs: FR-022, FR-028
 */

import { test } from '@playwright/test';
import {
  navigateToChannel,
  scrollToLatestMessage,
  navigateToThread,
  closeThreadPanel,
} from '../helpers/slack-nav';
import { captureStep, captureElement, isVisibleWithin } from '../helpers/screenshot';
import {
  BUTTON_SELECTORS,
  MODAL_SELECTORS,
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

/** Optional: timestamp of a thread anchor message for the B-side test. */
const threadAnchorTs = (): string | undefined => process.env.SLACK_THREAD_TS;

/**
 * Time (ms) to wait for the modal overlay after clicking "Resume with Instructions"
 * on a top-level message (A-side — should succeed).
 */
const MODAL_OPEN_TIMEOUT_TOPLEVEL = 8_000;

/**
 * Time (ms) to wait before concluding the modal was suppressed after clicking
 * "Resume with Instructions" inside a thread (B-side — expected to be suppressed).
 */
const MODAL_WAIT_TIMEOUT_THREAD = 5_000;

/**
 * Log a structured A/B comparison row for the S-T3-011 diagnostic report.
 */
function logAbComparisonRow(
  context: 'top-level' | 'in-thread',
  modalOpened: boolean,
  screenshotPath: string,
): void {
  const status = modalOpened ? '✅ modal appeared' : '❌ modal suppressed (silent failure)';
  console.log(
    `[S-T3-011 A/B] context=${context} | modal=${status} | screenshot=${screenshotPath}`,
  );
}

// ---------------------------------------------------------------------------
// S-T3-011 A-side: top-level wait-resume-instruct modal
// ---------------------------------------------------------------------------

test.describe('S-T3-011 A-side: Resume with Instructions modal in top-level message', () => {
  test('clicking Resume with Instructions on top-level message opens modal', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-011', 1, 'channel-loaded');

    // Locate a wait-for-instruction message (contains "Resume with Instructions" button).
    const resumeWithInstructionsBtn = page
      .locator(BUTTON_SELECTORS.resumeWithInstructionsButton)
      .first();
    const btnVisible = await isVisibleWithin(resumeWithInstructionsBtn, 10_000);

    if (!btnVisible) {
      await captureStep(page, 'S-T3-011', 2, 'no-resume-with-instructions-button-found');
      console.log(
        '[S-T3-011 A] No "Resume with Instructions" button found in the channel. ' +
          'Ensure a wait-for-instruction message exists in the test channel.',
      );
      test.skip();
      return;
    }

    // Capture actions block before clicking.
    const actionsBlock = page.locator(MESSAGE_SELECTORS.actionsBlock).first();
    if (await actionsBlock.isVisible()) {
      await captureElement(actionsBlock, 'S-T3-011', 2, 'a-side-before-click-actions-block');
    }
    await captureStep(page, 'S-T3-011', 3, 'a-side-resume-with-instructions-button-visible');

    // Click the button.
    await resumeWithInstructionsBtn.click();
    await captureStep(page, 'S-T3-011', 4, 'a-side-immediately-after-click');

    // Wait for the modal overlay.
    const modalOverlay = page.locator(MODAL_SELECTORS.modalOverlay).first();
    const modalAppeared = await isVisibleWithin(modalOverlay, MODAL_OPEN_TIMEOUT_TOPLEVEL);

    const keyScreenshot = await captureStep(
      page,
      'S-T3-011',
      5,
      modalAppeared ? 'a-side-modal-opened' : 'a-side-modal-not-opened',
    );

    logAbComparisonRow('top-level', modalAppeared, keyScreenshot);

    if (modalAppeared) {
      console.log('[S-T3-011 A] Modal appeared for top-level Resume with Instructions button ✅');

      // Verify modal structure.
      const modalDialog = page.locator(MODAL_SELECTORS.modalDialog).first();
      if (await modalDialog.isVisible()) {
        await captureElement(modalDialog, 'S-T3-011', 6, 'a-side-modal-structure');
      }

      const titleText = await page
        .locator(MODAL_SELECTORS.modalTitle)
        .first()
        .textContent()
        .catch(() => null);
      console.log(`[S-T3-011 A] Modal title: "${titleText?.trim()}"`);

      const textInput = page.locator(MODAL_SELECTORS.textInput).first();
      const inputVisible = await isVisibleWithin(textInput, 3_000);
      console.log(`[S-T3-011 A] Modal text input visible: ${inputVisible}`);

      await captureStep(page, 'S-T3-011', 7, 'a-side-modal-full-page-context');

      // Dismiss without submitting.
      const cancelBtn = page.locator(MODAL_SELECTORS.cancelButton).first();
      if (await isVisibleWithin(cancelBtn, 2_000)) {
        await cancelBtn.click();
        await captureStep(page, 'S-T3-011', 8, 'a-side-modal-dismissed');
      }
    } else {
      console.log(
        '[S-T3-011 A] Modal did not appear for top-level Resume with Instructions. ' +
          'This may indicate no active session or server not running.',
      );
    }

    await captureStep(page, 'S-T3-011', 9, 'a-side-flow-complete');
  });
});

// ---------------------------------------------------------------------------
// S-T3-011 B-side: in-thread wait-resume-instruct modal (expected: suppressed)
// ---------------------------------------------------------------------------

test.describe('S-T3-011 B-side: Resume with Instructions modal in threaded message', () => {
  test('clicking Resume with Instructions inside thread documents modal outcome', async ({
    page,
  }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-011', 10, 'channel-loaded');

    // --- Open the target thread ---
    const anchorTs = threadAnchorTs();

    if (anchorTs) {
      await navigateToThread(page, anchorTs);
      await captureStep(page, 'S-T3-011', 11, 'thread-opened-via-env-ts');
    } else {
      const replyBadge = page.locator(
        '[data-qa="threads-reply-count"], .c-threads-beta, [data-qa="thread-reply-count"]',
      ).first();
      const badgeVisible = await isVisibleWithin(replyBadge, 8_000);

      if (!badgeVisible) {
        await captureStep(page, 'S-T3-011', 11, 'no-thread-found-in-channel');
        test.skip();
        return;
      }

      await replyBadge.click();
      await captureStep(page, 'S-T3-011', 11, 'thread-opened-via-scan');
    }

    // Wait for thread panel.
    const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
    const threadPanelVisible = await isVisibleWithin(threadPanel, 10_000);

    if (!threadPanelVisible) {
      await captureStep(page, 'S-T3-011', 12, 'thread-panel-did-not-open');
      test.skip();
      return;
    }

    await captureStep(page, 'S-T3-011', 12, 'thread-panel-open');

    // Locate "Resume with Instructions" inside the thread.
    const resumeWithInstructionsBtnInThread = threadPanel
      .locator(BUTTON_SELECTORS.resumeWithInstructionsButton)
      .first();
    const btnInThreadVisible = await isVisibleWithin(resumeWithInstructionsBtnInThread, 8_000);

    if (!btnInThreadVisible) {
      await captureStep(page, 'S-T3-011', 13, 'no-resume-with-instructions-button-in-thread');
      console.log(
        '[S-T3-011 B] "Resume with Instructions" button not found inside the thread. ' +
          'Ensure SLACK_THREAD_TS points to a thread with a wait-for-instruction message.',
      );
      await closeThreadPanel(page);
      test.skip();
      return;
    }

    // Capture the button inside the thread.
    await captureElement(
      resumeWithInstructionsBtnInThread,
      'S-T3-011',
      13,
      'b-side-resume-with-instructions-button-in-thread',
    );
    await captureStep(page, 'S-T3-011', 14, 'b-side-thread-with-button-visible');

    // Click the button inside the thread.
    await resumeWithInstructionsBtnInThread.click();
    await captureStep(page, 'S-T3-011', 15, 'b-side-immediately-after-click-in-thread');

    // Wait for (or confirm absence of) modal.
    const modalOverlay = page.locator(MODAL_SELECTORS.modalOverlay).first();
    const modalAppeared = await isVisibleWithin(modalOverlay, MODAL_WAIT_TIMEOUT_THREAD);

    const keyScreenshot = await captureStep(
      page,
      'S-T3-011',
      16,
      modalAppeared
        ? 'b-side-modal-appeared-in-thread'
        : 'b-side-modal-suppressed-silent-failure-documented',
    );

    logAbComparisonRow('in-thread', modalAppeared, keyScreenshot);

    if (modalAppeared) {
      console.log(
        '[S-T3-011 B] UNEXPECTED: Modal appeared when Resume with Instructions clicked inside thread. ' +
          'This may indicate a Slack client fix. Capturing modal details.',
      );

      const modalDialog = page.locator(MODAL_SELECTORS.modalDialog).first();
      if (await modalDialog.isVisible()) {
        await captureElement(modalDialog, 'S-T3-011', 17, 'b-side-modal-structure-in-thread');
      }

      // Dismiss without submitting.
      const cancelBtn = page.locator(MODAL_SELECTORS.cancelButton).first();
      if (await isVisibleWithin(cancelBtn, 2_000)) {
        await cancelBtn.click();
        await captureStep(page, 'S-T3-011', 18, 'b-side-modal-dismissed');
      }
    } else {
      console.log(
        '[S-T3-011 B] Confirmed: modal silently suppressed for Resume with Instructions in thread. ' +
          'Consistent with S-T3-006 (Refine) and Phase 6 API evidence (S-T2-011).',
      );

      // Capture unchanged thread view as evidence.
      const actionsBlock = threadPanel.locator(MESSAGE_SELECTORS.actionsBlock).first();
      if (await isVisibleWithin(actionsBlock, 2_000)) {
        await captureElement(actionsBlock, 'S-T3-011', 17, 'b-side-thread-actions-block-unchanged');
      }
      await captureStep(page, 'S-T3-011', 18, 'b-side-thread-view-unchanged-after-suppression');
    }

    await closeThreadPanel(page);
    await captureStep(page, 'S-T3-011', 19, 'b-side-thread-closed-diagnosis-complete');
  });
});

// ---------------------------------------------------------------------------
// S-T3-011: Combined A/B summary
// ---------------------------------------------------------------------------

test.describe('S-T3-011 A/B summary: wait-resume-instruct modal-in-thread diagnostic', () => {
  test('logs structured A/B comparison for diagnostic report', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-011', 30, 'channel-loaded-for-ab-summary');

    // A-side summary: check if a top-level "Resume with Instructions" is visible.
    const topLevelBtn = page.locator(BUTTON_SELECTORS.resumeWithInstructionsButton).first();
    const topLevelBtnVisible = await isVisibleWithin(topLevelBtn, 8_000);

    if (!topLevelBtnVisible) {
      console.log('[S-T3-011 A/B summary] No top-level Resume with Instructions button visible.');
      test.skip();
      return;
    }

    await topLevelBtn.click();
    const modalOverlay = page.locator(MODAL_SELECTORS.modalOverlay).first();
    const topLevelModalAppeared = await isVisibleWithin(modalOverlay, MODAL_OPEN_TIMEOUT_TOPLEVEL);
    const aScreenshot = await captureStep(
      page,
      'S-T3-011',
      31,
      topLevelModalAppeared
        ? 'ab-summary-a-side-modal-opened'
        : 'ab-summary-a-side-modal-not-opened',
    );

    logAbComparisonRow('top-level', topLevelModalAppeared, aScreenshot);

    // Dismiss the modal before the B-side.
    if (topLevelModalAppeared) {
      const cancelBtn = page.locator(MODAL_SELECTORS.cancelButton).first();
      if (await isVisibleWithin(cancelBtn, 2_000)) {
        await cancelBtn.click();
      }
    }

    // B-side summary: try to find and click the button inside a thread.
    const replyBadge = page.locator(
      '[data-qa="threads-reply-count"], .c-threads-beta, [data-qa="thread-reply-count"]',
    ).first();
    const badgeVisible = await isVisibleWithin(replyBadge, 8_000);

    if (!badgeVisible) {
      console.log('[S-T3-011 A/B summary] No thread found for B-side test.');
      await captureStep(page, 'S-T3-011', 32, 'ab-summary-no-thread-for-b-side');
      return;
    }

    await replyBadge.click();
    const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
    await isVisibleWithin(threadPanel, 10_000);

    const threadBtn = threadPanel
      .locator(BUTTON_SELECTORS.resumeWithInstructionsButton)
      .first();
    const threadBtnVisible = await isVisibleWithin(threadBtn, 5_000);

    if (!threadBtnVisible) {
      console.log('[S-T3-011 A/B summary] No Resume with Instructions button found in thread.');
      await closeThreadPanel(page);
      return;
    }

    await threadBtn.click();
    const threadModalOverlay = page.locator(MODAL_SELECTORS.modalOverlay).first();
    const threadModalAppeared = await isVisibleWithin(
      threadModalOverlay,
      MODAL_WAIT_TIMEOUT_THREAD,
    );
    const bScreenshot = await captureStep(
      page,
      'S-T3-011',
      32,
      threadModalAppeared ? 'ab-summary-b-side-modal-appeared' : 'ab-summary-b-side-modal-suppressed',
    );

    logAbComparisonRow('in-thread', threadModalAppeared, bScreenshot);

    // Dismiss if unexpectedly opened.
    if (threadModalAppeared) {
      const cancelBtn = page.locator(MODAL_SELECTORS.cancelButton).first();
      if (await isVisibleWithin(cancelBtn, 2_000)) {
        await cancelBtn.click();
      }
    }

    await closeThreadPanel(page);
    await captureStep(page, 'S-T3-011', 33, 'ab-summary-complete');

    console.log('[S-T3-011 A/B summary] Comparison complete. See screenshots for visual evidence.');
  });
});
