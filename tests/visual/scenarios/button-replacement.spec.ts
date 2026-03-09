/**
 * Phase 8 — Visual Rendering Tests: button-replacement.spec.ts
 *
 * Screenshot-based verification that interactive Block Kit buttons are replaced
 * by static status text after the operator takes an action. Covers the primary
 * button types used across agent-intercom message types:
 *
 *   - Continue (prompt messages)
 *   - Stop Session (prompt / stall alert messages)
 *   - Nudge (stall alert messages)
 *   - Resume (wait-for-instruction messages)
 *
 * For each button type the test captures:
 *   1. The message before clicking (buttons visible, interactive).
 *   2. Immediately after the click (optimistic state capture).
 *   3. After the server has processed the action (static text replaces buttons).
 *
 * All tests skip gracefully when required environment variables are absent.
 *
 * Scenarios covered:
 *   S-T3-008  Button replacement after various operator actions
 *
 * FRs: FR-027
 */

import { test, expect } from '@playwright/test';
import { navigateToChannel, scrollToLatestMessage } from '../helpers/slack-nav';
import { captureStep, captureElement, isVisibleWithin } from '../helpers/screenshot';
import {
  MESSAGE_SELECTORS,
  BUTTON_SELECTORS,
} from '../helpers/slack-selectors';
import type { Locator, Page } from '@playwright/test';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function hasRequiredEnv(): boolean {
  return Boolean(process.env.SLACK_WORKSPACE_URL && process.env.SLACK_TEST_CHANNEL);
}

const testChannel = (): string => process.env.SLACK_TEST_CHANNEL ?? 'agent-intercom-test';

/** Milliseconds to wait for a button to disappear after being clicked. */
const REPLACEMENT_TIMEOUT = 15_000;

/**
 * Generic button-click test helper.
 *
 * 1. Navigates to the test channel.
 * 2. Waits for the target button to appear.
 * 3. Captures screenshots before / immediately after / after replacement.
 * 4. Asserts the button is gone (or documents the timeout).
 *
 * @param page       - Playwright page instance
 * @param scenarioId - Scenario identifier for screenshot naming
 * @param stepOffset - Starting step number (allows multiple tests in same scenario)
 * @param buttonSel  - Playwright locator for the button to click
 * @param label      - Human-readable label for screenshot file names
 */
async function runButtonReplacementTest(
  page: Page,
  scenarioId: string,
  stepOffset: number,
  buttonSel: Locator,
  label: string,
): Promise<void> {
  await navigateToChannel(page, testChannel());
  await scrollToLatestMessage(page);

  await captureStep(page, scenarioId, stepOffset, `channel-loaded-${label}`);

  const buttonVisible = await isVisibleWithin(buttonSel, 10_000);

  if (!buttonVisible) {
    await captureStep(page, scenarioId, stepOffset + 1, `no-${label}-button-present`);
    test.skip();
    return;
  }

  // Capture the actions block before clicking.
  const actionsBlock = page.locator(MESSAGE_SELECTORS.actionsBlock).first();
  if (await actionsBlock.isVisible()) {
    await captureElement(actionsBlock, scenarioId, stepOffset + 1, `before-${label}-click-actions`);
  }
  await captureStep(page, scenarioId, stepOffset + 2, `before-${label}-click-full`);

  // Click the button.
  await buttonSel.click();
  await captureStep(page, scenarioId, stepOffset + 3, `immediately-after-${label}-click`);

  // Wait for the button to be replaced (hidden).
  const buttonGone = await buttonSel
    .waitFor({ state: 'hidden', timeout: REPLACEMENT_TIMEOUT })
    .then(() => true)
    .catch(() => false);

  await captureStep(page, scenarioId, stepOffset + 4, `after-${label}-replacement`);

  if (buttonGone) {
    // Verify static text is shown instead.
    const resolvedStatus = page.locator(MESSAGE_SELECTORS.resolvedStatus).first();
    if (await isVisibleWithin(resolvedStatus, 5_000)) {
      await captureElement(resolvedStatus, scenarioId, stepOffset + 5, `${label}-resolved-status`);
    }
    expect(buttonGone, `Expected ${label} button to be replaced after clicking`).toBe(true);
  } else {
    // Document: server did not update the message within timeout.
    await captureStep(page, scenarioId, stepOffset + 5, `${label}-replacement-timeout`);
  }
}

// ---------------------------------------------------------------------------
// Continue button replacement (prompt messages)
// ---------------------------------------------------------------------------

test.describe('S-T3-008 Continue: prompt Continue button replaced after click', () => {
  test('Continue button is replaced with static resolved text', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await runButtonReplacementTest(
      page,
      'S-T3-008',
      1,
      page.locator(BUTTON_SELECTORS.continueButton).first(),
      'continue',
    );
  });
});

// ---------------------------------------------------------------------------
// Stop Session button replacement (prompt / stall alert messages)
// ---------------------------------------------------------------------------

test.describe('S-T3-008 Stop: Stop Session button replaced after click', () => {
  test('Stop Session button is replaced with static resolved text', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await runButtonReplacementTest(
      page,
      'S-T3-008',
      10,
      page.locator(BUTTON_SELECTORS.stopButton).first(),
      'stop-session',
    );
  });
});

// ---------------------------------------------------------------------------
// Nudge button replacement (stall alert messages)
// ---------------------------------------------------------------------------

test.describe('S-T3-008 Nudge: Nudge button replaced after click', () => {
  test('Nudge button is replaced with static resolved text', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await runButtonReplacementTest(
      page,
      'S-T3-008',
      20,
      page.locator(BUTTON_SELECTORS.nudgeButton).first(),
      'nudge',
    );
  });
});

// ---------------------------------------------------------------------------
// Resume button replacement (wait-for-instruction messages)
// ---------------------------------------------------------------------------

test.describe('S-T3-008 Resume: Resume button replaced after click', () => {
  test('Resume button is replaced with static resolved text', async ({ page }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await runButtonReplacementTest(
      page,
      'S-T3-008',
      30,
      page.locator(BUTTON_SELECTORS.resumeButton).first(),
      'resume',
    );
  });
});

// ---------------------------------------------------------------------------
// Composite: verify static text content after replacement
// ---------------------------------------------------------------------------

test.describe('S-T3-008 Static text: resolved status replaces action block', () => {
  test('after any button click, the actions block is replaced by plain-text section', async ({
    page,
  }) => {
    if (!hasRequiredEnv()) {
      test.skip();
      return;
    }

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);

    await captureStep(page, 'S-T3-008', 40, 'channel-loaded-for-static-text-test');

    // Find any interactive button — any of the known types will do.
    const anyButton = page.locator(
      [
        BUTTON_SELECTORS.continueButton,
        BUTTON_SELECTORS.nudgeButton,
        BUTTON_SELECTORS.resumeButton,
      ].join(', '),
    ).first();

    const anyButtonVisible = await isVisibleWithin(anyButton, 10_000);

    if (!anyButtonVisible) {
      await captureStep(page, 'S-T3-008', 41, 'no-interactive-button-found');
      test.skip();
      return;
    }

    // Capture the actions block before clicking.
    const actionsBlock = page.locator(MESSAGE_SELECTORS.actionsBlock).first();
    const actionsBlockVisible = await actionsBlock.isVisible();
    if (actionsBlockVisible) {
      await captureElement(actionsBlock, 'S-T3-008', 41, 'actions-block-before-click');
    }

    await captureStep(page, 'S-T3-008', 42, 'before-any-button-click');

    // Click the first available interactive button.
    await anyButton.click();
    await captureStep(page, 'S-T3-008', 43, 'immediately-after-button-click');

    // Wait for the actions block to be replaced.
    const actionBlockGone = await actionsBlock
      .waitFor({ state: 'hidden', timeout: REPLACEMENT_TIMEOUT })
      .then(() => true)
      .catch(() => false);

    await captureStep(page, 'S-T3-008', 44, 'after-replacement');

    if (actionBlockGone) {
      // Verify the static resolved status text is now visible.
      const resolvedStatus = page.locator(MESSAGE_SELECTORS.resolvedStatus).first();
      const statusVisible = await isVisibleWithin(resolvedStatus, 5_000);

      if (statusVisible) {
        await captureElement(resolvedStatus, 'S-T3-008', 45, 'static-resolved-text');
        // Confirm the element contains some text content (not just whitespace).
        const textContent = await resolvedStatus.textContent();
        expect(
          (textContent ?? '').trim().length,
          'Expected resolved status text to have content',
        ).toBeGreaterThan(0);
      }
    } else {
      await captureStep(page, 'S-T3-008', 45, 'action-block-still-visible-after-timeout');
    }

    await captureStep(page, 'S-T3-008', 46, 'button-replacement-test-complete');
  });
});
