/**
 * Phase 9 — Modal-in-Thread Visual Diagnosis: modal-in-thread.spec.ts
 *
 * The critical B side of the A/B comparison. Navigates into a Slack thread,
 * finds a Refine button on a threaded prompt message, clicks it, and
 * documents whether the modal overlay appears.
 *
 * Based on Phase 6 API-level evidence (modal-diagnostic-report.md), the
 * expected finding is:
 *   - The server calls `views.open` and receives `{"ok": true}`.
 *   - The Slack client silently suppresses modal rendering.
 *   - The operator sees no dialog — only the thread view remains unchanged.
 *
 * This test captures visual evidence (screenshots) of that silent failure.
 * It does NOT assert that the modal MUST appear; instead it documents the
 * actual client behaviour and captures a screenshot for the diagnostic report.
 *
 * When the modal does appear (future Slack client update or platform fix),
 * the test documents that too — the screenshot set will reflect the change.
 *
 * All tests skip gracefully when required environment variables are absent.
 *
 * Scenarios covered:
 *   S-T3-006  Modal-in-thread: click Refine inside thread, document outcome
 *
 * FRs: FR-022, FR-027, FR-028, FR-030
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

/**
 * Timestamp of a specific message that was posted inside a thread, used as the
 * anchor for `navigateToThread`. Configurable via env so each test run can
 * target a fresh thread without hardcoding a timestamp.
 *
 * If not set, the test uses the most recent threaded message it can locate.
 */
const threadAnchorTs = (): string | undefined => process.env.SLACK_THREAD_TS;

/**
 * Time (ms) to wait before concluding that the modal has NOT rendered after
 * clicking the Refine button inside a thread. Slack's client-side suppression
 * is immediate; a 5-second window is sufficient for diagnosis while keeping
 * the test fast.
 */
const MODAL_WAIT_TIMEOUT = 5_000;

/**
 * Log a structured A/B comparison row for the diagnostic report.
 *
 * @param context     - "top-level" or "in-thread"
 * @param modalOpened - true if the modal overlay appeared
 * @param screenshotPath - path of the key screenshot for this observation
 */
function logAbComparisonRow(
  context: 'top-level' | 'in-thread',
  modalOpened: boolean,
  screenshotPath: string,
): void {
  const status = modalOpened ? '✅ modal appeared' : '❌ modal suppressed (silent failure)';
  console.log(`[S-T3-006 A/B] context=${context} | modal=${status} | screenshot=${screenshotPath}`);
}

// ---------------------------------------------------------------------------
// S-T3-006: Modal-in-thread diagnosis
// ---------------------------------------------------------------------------

test.describe('S-T3-006: Modal-in-thread — click Refine inside a thread, document outcome', () => {
  /**
   * Primary diagnostic test: open a thread, find a Refine button on a threaded
   * prompt message, click it, and capture what the Slack client renders (or
   * does not render) within the MODAL_WAIT_TIMEOUT window.
   */
  test('clicking Refine inside a thread documents whether modal appears or is silently suppressed', async ({
    page,
  }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-006', 1, 'channel-loaded');

    // --- Locate a threaded message with a Refine button ---
    // Strategy A: SLACK_THREAD_TS env var points to a specific message.
    // Strategy B: scan visible messages for a reply-count badge (thread exists),
    //             open the first thread found, and look for a Refine button there.

    const anchorTs = threadAnchorTs();

    if (anchorTs) {
      // Navigate directly to the specified thread.
      await navigateToThread(page, anchorTs);
      await captureStep(page, 'S-T3-006', 2, 'thread-opened-via-env-ts');
    } else {
      // Scan for the first visible threaded message (has reply count badge).
      const replyBadge = page.locator(
        '[data-qa="threads-reply-count"], .c-threads-beta, [data-qa="thread-reply-count"]',
      ).first();
      const badgeVisible = await isVisibleWithin(replyBadge, 8_000);

      if (!badgeVisible) {
        await captureStep(page, 'S-T3-006', 2, 'no-thread-found-in-channel');
        test.skip();
        return;
      }

      // Hover the parent message to reveal the thread icon, then click it.
      const parentMessage = replyBadge.locator('..').locator('..').first();
      await parentMessage.hover();
      await replyBadge.click();
      await captureStep(page, 'S-T3-006', 2, 'thread-opened-via-scan');
    }

    // Wait for the thread panel to be visible.
    const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
    const threadPanelVisible = await isVisibleWithin(threadPanel, 10_000);

    if (!threadPanelVisible) {
      await captureStep(page, 'S-T3-006', 3, 'thread-panel-did-not-open');
      test.skip();
      return;
    }

    await captureStep(page, 'S-T3-006', 3, 'thread-panel-open');

    // --- Locate a Refine button inside the thread panel ---
    const refineBtn = threadPanel.locator(BUTTON_SELECTORS.refineButton).first();
    const refineBtnVisible = await isVisibleWithin(refineBtn, 8_000);

    if (!refineBtnVisible) {
      await captureStep(page, 'S-T3-006', 4, 'no-refine-button-in-thread');
      console.log('[S-T3-006] No Refine button found inside the thread panel.');
      await closeThreadPanel(page);
      test.skip();
      return;
    }

    // Capture the Refine button before clicking.
    if (await refineBtn.isVisible()) {
      await captureElement(refineBtn, 'S-T3-006', 4, 'refine-button-inside-thread');
    }
    await captureStep(page, 'S-T3-006', 5, 'thread-with-refine-button');

    // --- Click Refine inside the thread ---
    await refineBtn.click();
    await captureStep(page, 'S-T3-006', 6, 'immediately-after-refine-click-in-thread');

    // --- Wait and observe: does the modal appear? ---
    const modalOverlay = page.locator(MODAL_SELECTORS.modalOverlay).first();
    const modalAppeared = await isVisibleWithin(modalOverlay, MODAL_WAIT_TIMEOUT);

    // Capture the key diagnostic screenshot — this is the primary evidence.
    const keyScreenshotPath = await captureStep(
      page,
      'S-T3-006',
      7,
      modalAppeared
        ? 'modal-appeared-in-thread-context'
        : 'no-modal-rendered-silent-failure-documented',
    );

    // Log the A/B comparison row.
    logAbComparisonRow('in-thread', modalAppeared, keyScreenshotPath);

    if (modalAppeared) {
      console.log(
        '[S-T3-006] UNEXPECTED: Modal appeared when Refine clicked inside a thread. ' +
          'This may indicate a Slack client update has resolved the silent-failure issue.',
      );

      // Capture modal details.
      const modalDialog = page.locator(MODAL_SELECTORS.modalDialog).first();
      if (await modalDialog.isVisible()) {
        await captureElement(modalDialog, 'S-T3-006', 8, 'modal-structure-in-thread-context');
      }

      const titleText = await page
        .locator(MODAL_SELECTORS.modalTitle)
        .first()
        .textContent()
        .catch(() => null);
      console.log(`[S-T3-006] Modal title (thread context): "${titleText?.trim()}"`);

      // Dismiss without submitting to avoid side effects.
      const cancelBtn = page.locator(MODAL_SELECTORS.cancelButton).first();
      if (await isVisibleWithin(cancelBtn, 2_000)) {
        await cancelBtn.click();
        await captureStep(page, 'S-T3-006', 9, 'modal-dismissed-in-thread');
      }
    } else {
      console.log(
        '[S-T3-006] Confirmed: modal silently suppressed when Refine clicked inside thread. ' +
          'This is consistent with Phase 6 API evidence — client-side rendering suppression.',
      );

      // Capture the unchanged thread view (visual evidence of the suppression).
      const actionsBlock = threadPanel.locator(MESSAGE_SELECTORS.actionsBlock).first();
      if (await isVisibleWithin(actionsBlock, 2_000)) {
        await captureElement(actionsBlock, 'S-T3-006', 8, 'thread-actions-block-unchanged');
      }
      await captureStep(page, 'S-T3-006', 9, 'thread-view-unchanged-after-suppressed-modal');
    }

    await closeThreadPanel(page);
    await captureStep(page, 'S-T3-006', 10, 'thread-closed-diagnosis-complete');
  });

  /**
   * A/B summary sub-test: runs the B-side (in-thread) observation and logs a
   * structured comparison row alongside the A-side (top-level) expectation.
   * Does not assert — documents.
   */
  test('A/B summary: thread-context modal outcome documented for diagnostic report', async ({
    page,
  }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    // A-side expectation: top-level modal is known to work.
    console.log('[S-T3-006 A/B] A-side (top-level): modal renders ✅ (confirmed by S-T3-005)');

    // B-side: open any thread and check for Refine button.
    const replyBadge = page.locator(
      '[data-qa="threads-reply-count"], .c-threads-beta, [data-qa="thread-reply-count"]',
    ).first();
    const badgeVisible = await isVisibleWithin(replyBadge, 8_000);

    if (!badgeVisible) {
      console.log('[S-T3-006 A/B] B-side: no thread found — skipping B-side documentation.');
      test.skip();
      return;
    }

    await replyBadge.click();

    const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
    await isVisibleWithin(threadPanel, 10_000);

    const refineBtnInThread = threadPanel.locator(BUTTON_SELECTORS.refineButton).first();
    const hasRefineInThread = await isVisibleWithin(refineBtnInThread, 5_000);

    if (!hasRefineInThread) {
      console.log('[S-T3-006 A/B] B-side: Refine button not found in thread — skipping.');
      await closeThreadPanel(page);
      test.skip();
      return;
    }

    await refineBtnInThread.click();
    await captureStep(page, 'S-T3-006', 30, 'ab-summary-after-threaded-refine-click');

    const modalOverlay = page.locator(MODAL_SELECTORS.modalOverlay).first();
    const modalAppeared = await isVisibleWithin(modalOverlay, MODAL_WAIT_TIMEOUT);

    const screenshotPath = await captureStep(
      page,
      'S-T3-006',
      31,
      modalAppeared ? 'ab-summary-modal-appeared' : 'ab-summary-modal-suppressed',
    );

    logAbComparisonRow('in-thread', modalAppeared, screenshotPath);

    // Dismiss any open modal.
    if (modalAppeared) {
      const cancelBtn = page.locator(MODAL_SELECTORS.cancelButton).first();
      if (await isVisibleWithin(cancelBtn, 2_000)) {
        await cancelBtn.click();
      }
    }

    await closeThreadPanel(page);
    await captureStep(page, 'S-T3-006', 32, 'ab-summary-complete');
  });
});
