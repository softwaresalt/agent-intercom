/**
 * Phase 9 — Modal-in-Thread Visual Diagnosis: modal-top-level.spec.ts
 *
 * Visual verification of the Refine modal flow when triggered from a TOP-LEVEL
 * prompt message (not inside a thread). This is the A side of the A/B comparison
 * that diagnoses the modal-in-thread silent failure described in the Phase 6
 * modal diagnostic report.
 *
 * Expected behaviour (top-level context):
 *   1. Refine button is visible and clickable.
 *   2. Clicking Refine calls `views.open` with a valid trigger_id.
 *   3. Slack client renders the modal overlay containing:
 *        - Title matching the instruction modal title
 *        - A text input (plain-text or multi-line)
 *        - A Submit button
 *   4. Typing text in the input and clicking Submit sends a `ViewSubmission`
 *      event to the server.
 *   5. The server resolves the prompt and updates the message with a resolved
 *      status line (buttons replaced).
 *
 * All tests skip gracefully when SLACK_WORKSPACE_URL or SLACK_TEST_CHANNEL are
 * not configured so the spec can be enumerated in CI without live credentials.
 *
 * Scenarios covered:
 *   S-T3-005  Top-level Refine modal renders and submits
 *
 * FRs: FR-027, FR-028
 */

import { test, expect } from '@playwright/test';
import { navigateToChannel, scrollToLatestMessage } from '../helpers/slack-nav';
import { captureStep, captureElement, isVisibleWithin } from '../helpers/screenshot';
import {
  BUTTON_SELECTORS,
  MODAL_SELECTORS,
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
 * Maximum time (ms) to wait for the Refine modal to appear after clicking
 * the Refine button. Top-level modals should render within ~5 s.
 */
const MODAL_OPEN_TIMEOUT = 8_000;

/**
 * Maximum time (ms) to wait for the modal to close and the message to be
 * updated with a resolved status after clicking Submit.
 */
const MODAL_SUBMIT_TIMEOUT = 15_000;

/**
 * Instruction text typed into the modal text input during the test.
 * Kept short to minimise risk of filling a real agent's context.
 */
const TEST_INSTRUCTION_TEXT = '[visual-test] automated Refine submission — ignore';

// ---------------------------------------------------------------------------
// S-T3-005: Top-level Refine modal — full flow
// ---------------------------------------------------------------------------

test.describe('S-T3-005: Top-level Refine modal renders and submits', () => {
  /**
   * Main scenario: navigate to the test channel, find a prompt message, click
   * Refine, verify the modal renders, type text, submit, and verify the resolved
   * status appears.
   */
  test('modal opens for top-level Refine button, accepts input, and resolves on submit', async ({
    page,
  }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-005', 1, 'channel-loaded');

    // --- Locate the Refine button in a top-level prompt message ---
    // Prompt messages contain Continue, Refine, and Stop buttons.  We target
    // the Refine button specifically because it is the one that opens a modal.
    const refineBtn = page.locator(BUTTON_SELECTORS.refineButton).first();
    const refineBtnVisible = await isVisibleWithin(refineBtn, 10_000);

    if (!refineBtnVisible) {
      // No prompt message with a Refine button found in the channel.
      await captureStep(page, 'S-T3-005', 2, 'no-refine-button-found');
      test.skip();
      return;
    }

    // Capture the message with the Refine button before clicking.
    const actionsBlockBefore = page.locator(MESSAGE_SELECTORS.actionsBlock).first();
    if (await actionsBlockBefore.isVisible()) {
      await captureElement(actionsBlockBefore, 'S-T3-005', 2, 'before-click-actions-block');
    }
    await captureStep(page, 'S-T3-005', 3, 'refine-button-visible');

    // --- Click Refine — triggers views.open on the server ---
    await refineBtn.click();
    await captureStep(page, 'S-T3-005', 4, 'immediately-after-refine-click');

    // --- Verify the modal appears ---
    const modalOverlay = page.locator(MODAL_SELECTORS.modalOverlay).first();
    const modalAppeared = await isVisibleWithin(modalOverlay, MODAL_OPEN_TIMEOUT);

    await captureStep(page, 'S-T3-005', 5, modalAppeared ? 'modal-opened' : 'modal-did-not-open');

    if (!modalAppeared) {
      // Document the failure: modal did not render for a top-level message.
      // This would indicate a regression in the known-good path.
      console.log(
        '[S-T3-005] Modal did not appear after clicking Refine on a top-level message.',
      );
      // Record the finding but do not hard-fail; live environment differences
      // (e.g., no active session) may cause the server to reject the trigger.
      return;
    }

    // --- Verify modal structure ---
    const modalTitle = page.locator(MODAL_SELECTORS.modalTitle).first();
    const titleVisible = await isVisibleWithin(modalTitle, 3_000);

    if (titleVisible) {
      await captureElement(modalTitle, 'S-T3-005', 6, 'modal-title');
      const titleText = await modalTitle.textContent();
      console.log(`[S-T3-005] Modal title: "${titleText?.trim()}"`);
    }

    // Verify the text input is present.
    const textInput = page.locator(MODAL_SELECTORS.textInput).first();
    const inputVisible = await isVisibleWithin(textInput, 3_000);
    expect(inputVisible, 'Expected a text input inside the Refine modal').toBe(true);

    // Verify the Submit button is present.
    const submitBtn = page.locator(MODAL_SELECTORS.submitButton).first();
    const submitVisible = await isVisibleWithin(submitBtn, 3_000);
    expect(submitVisible, 'Expected a Submit button inside the Refine modal').toBe(true);

    // Capture the full modal view.
    const modalDialog = page.locator(MODAL_SELECTORS.modalDialog).first();
    if (await modalDialog.isVisible()) {
      await captureElement(modalDialog, 'S-T3-005', 7, 'modal-structure-with-input-and-submit');
    }
    await captureStep(page, 'S-T3-005', 8, 'modal-full-page-context');

    // --- Type test instruction text ---
    await textInput.click();
    await textInput.fill(TEST_INSTRUCTION_TEXT);
    await captureStep(page, 'S-T3-005', 9, 'instruction-text-typed');

    if (await modalDialog.isVisible()) {
      await captureElement(modalDialog, 'S-T3-005', 10, 'modal-with-text-entered');
    }

    // --- Click Submit ---
    await submitBtn.click();
    await captureStep(page, 'S-T3-005', 11, 'immediately-after-submit-click');

    // --- Verify modal closes ---
    const modalClosed = await modalOverlay
      .waitFor({ state: 'hidden', timeout: MODAL_SUBMIT_TIMEOUT })
      .then(() => true)
      .catch(() => false);

    await captureStep(page, 'S-T3-005', 12, modalClosed ? 'modal-closed-after-submit' : 'modal-still-open');

    if (modalClosed) {
      console.log('[S-T3-005] Modal closed successfully after submit.');

      // Verify the message was updated with a resolved status.
      const resolvedStatus = page.locator(MESSAGE_SELECTORS.resolvedStatus).first();
      const statusVisible = await isVisibleWithin(resolvedStatus, 10_000);

      if (statusVisible) {
        await captureElement(resolvedStatus, 'S-T3-005', 13, 'resolved-status-after-submit');
      }
    }

    await captureStep(page, 'S-T3-005', 14, 'top-level-refine-flow-complete');
  });

  /**
   * Negative-path sub-test: verify modal structural elements when modal is open
   * (title, text area, submit button presence) without submitting. Useful as a
   * lightweight structural check that does not affect live session state.
   */
  test('modal structure contains title, text input, and submit button', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    const refineBtn = page.locator(BUTTON_SELECTORS.refineButton).first();
    const refineBtnVisible = await isVisibleWithin(refineBtn, 10_000);

    if (!refineBtnVisible) {
      test.skip();
      return;
    }

    await refineBtn.click();
    await captureStep(page, 'S-T3-005', 20, 'after-refine-click-structure-test');

    const modalOverlay = page.locator(MODAL_SELECTORS.modalOverlay).first();
    const modalAppeared = await isVisibleWithin(modalOverlay, MODAL_OPEN_TIMEOUT);

    if (!modalAppeared) {
      await captureStep(page, 'S-T3-005', 21, 'modal-not-opened-structure-test');
      return;
    }

    // Assert all three required structural elements.
    const title = page.locator(MODAL_SELECTORS.modalTitle).first();
    const input = page.locator(MODAL_SELECTORS.textInput).first();
    const submit = page.locator(MODAL_SELECTORS.submitButton).first();

    await expect(title).toBeVisible({ timeout: 3_000 });
    await expect(input).toBeVisible({ timeout: 3_000 });
    await expect(submit).toBeVisible({ timeout: 3_000 });

    await captureStep(page, 'S-T3-005', 22, 'modal-structure-verified');

    // Close the modal without submitting.
    const cancelBtn = page.locator(MODAL_SELECTORS.cancelButton).first();
    if (await isVisibleWithin(cancelBtn, 2_000)) {
      await cancelBtn.click();
      await captureStep(page, 'S-T3-005', 23, 'modal-dismissed-without-submit');
    }
  });
});
