/**
 * US17 — Text-Only Thread Prompt Validation
 *
 * Verifies that US17 text-only thread messages render correctly in
 * the Slack web client:
 *
 * S-T3-AUTO-009: Text-only continuation prompt renders in thread
 *   without any block-kit action buttons.
 * S-T3-AUTO-010: Text-only wait status renders in thread without
 *   any block-kit action buttons.
 * S-T3-AUTO-011: Text-only approval request renders in thread with
 *   diff block but no action buttons.
 * S-T3-AUTO-012: All text-only messages include @-mention reply
 *   instruction text.
 */
import { test, expect } from '@playwright/test';

import { navigateToChannel, navigateToThread } from '../helpers/slack-nav';
import { captureStep, isVisibleWithin } from '../helpers/screenshot';
import { MESSAGE_SELECTORS, THREAD_SELECTORS } from '../helpers/slack-selectors';
import {
  SlackFixtureClient,
  hasAutomatedVisualEnv,
  type TextOnlyFixtures,
} from '../helpers/slack-fixtures';

const PROMPT_SCENARIO = 'S-T3-AUTO-009';
const WAIT_SCENARIO = 'S-T3-AUTO-010';
const APPROVAL_SCENARIO = 'S-T3-AUTO-011';
const INSTRUCTION_SCENARIO = 'S-T3-AUTO-012';

const testChannel = (): string =>
  process.env.SLACK_TEST_CHANNEL ?? 'agent-intercom-test';

let fixtureClient: SlackFixtureClient | null = null;
let fixtures: TextOnlyFixtures | null = null;

function envReady(): boolean {
  return hasAutomatedVisualEnv();
}

function requireFixtures(): TextOnlyFixtures {
  if (!fixtures) {
    throw new Error('text-only thread fixtures were not seeded');
  }

  return fixtures;
}

test.describe('US17 text-only thread prompt validation', () => {
  test.beforeAll(async () => {
    if (!envReady()) {
      return;
    }

    fixtureClient = SlackFixtureClient.fromEnv();
    fixtures = await fixtureClient.seedTextOnlyThreadFixtures();
    console.log(
      `[US17] Seeded text-only fixtures: run=${fixtures.runId}, ` +
        `anchorTs=${fixtures.anchorTs}, promptTs=${fixtures.promptTs}, ` +
        `waitTs=${fixtures.waitTs}, approvalTs=${fixtures.approvalTs}`,
    );
  });

  test.afterAll(async () => {
    if (fixtureClient && fixtures) {
      await fixtureClient.deleteMessages(fixtures.cleanupTs);
    }
  });

  test(
    'S-T3-AUTO-009: text-only prompt renders in thread without buttons',
    async ({ page }) => {
      if (!envReady()) {
        test.skip();
        return;
      }

      const f = requireFixtures();

      await navigateToChannel(page, testChannel());
      await navigateToThread(page, f.anchorTs);
      await captureStep(page, PROMPT_SCENARIO, 1, 'thread-opened');

      const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
      const panelVisible = await isVisibleWithin(threadPanel, 10_000);
      expect(panelVisible, 'Thread panel must open').toBe(true);

      // Find the text-only prompt by its timestamp.
      const promptRow = threadPanel
        .locator(`[data-item-key="${f.promptTs}"]`)
        .first();
      const promptVisible = await isVisibleWithin(promptRow, 15_000);
      expect(promptVisible, 'Text-only prompt must be visible in thread').toBe(true);

      await captureStep(page, PROMPT_SCENARIO, 2, 'prompt-found');

      // Verify NO action buttons exist inside this message.
      const buttons = promptRow.locator('button[data-qa="actions_block_action"]');
      const buttonCount = await buttons.count();
      expect(
        buttonCount,
        'Text-only prompt must have zero action buttons',
      ).toBe(0);

      // Verify the prompt text is present.
      const text = await promptRow
        .locator(MESSAGE_SELECTORS.messageText)
        .first()
        .textContent();
      expect(text?.toLowerCase()).toContain('continuation prompt');

      await captureStep(page, PROMPT_SCENARIO, 3, 'no-buttons-confirmed');
      console.log(`[${PROMPT_SCENARIO}] Text-only prompt confirmed: no buttons, text visible.`);
    },
  );

  test(
    'S-T3-AUTO-010: text-only wait renders in thread without buttons',
    async ({ page }) => {
      if (!envReady()) {
        test.skip();
        return;
      }

      const f = requireFixtures();

      await navigateToChannel(page, testChannel());
      await navigateToThread(page, f.anchorTs);
      await captureStep(page, WAIT_SCENARIO, 1, 'thread-opened');

      const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
      await isVisibleWithin(threadPanel, 10_000);

      const waitRow = threadPanel
        .locator(`[data-item-key="${f.waitTs}"]`)
        .first();
      const waitVisible = await isVisibleWithin(waitRow, 15_000);
      expect(waitVisible, 'Text-only wait must be visible in thread').toBe(true);

      await captureStep(page, WAIT_SCENARIO, 2, 'wait-found');

      const buttons = waitRow.locator('button[data-qa="actions_block_action"]');
      const buttonCount = await buttons.count();
      expect(buttonCount, 'Text-only wait must have zero action buttons').toBe(0);

      const text = await waitRow
        .locator(MESSAGE_SELECTORS.messageText)
        .first()
        .textContent();
      expect(text?.toLowerCase()).toContain('agent waiting');

      await captureStep(page, WAIT_SCENARIO, 3, 'no-buttons-confirmed');
      console.log(`[${WAIT_SCENARIO}] Text-only wait confirmed: no buttons, text visible.`);
    },
  );

  test(
    'S-T3-AUTO-011: text-only approval renders in thread without buttons',
    async ({ page }) => {
      if (!envReady()) {
        test.skip();
        return;
      }

      const f = requireFixtures();

      await navigateToChannel(page, testChannel());
      await navigateToThread(page, f.anchorTs);
      await captureStep(page, APPROVAL_SCENARIO, 1, 'thread-opened');

      const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
      await isVisibleWithin(threadPanel, 10_000);

      const approvalRow = threadPanel
        .locator(`[data-item-key="${f.approvalTs}"]`)
        .first();
      const approvalVisible = await isVisibleWithin(approvalRow, 15_000);
      expect(approvalVisible, 'Text-only approval must be visible in thread').toBe(true);

      await captureStep(page, APPROVAL_SCENARIO, 2, 'approval-found');

      const buttons = approvalRow.locator('button[data-qa="actions_block_action"]');
      const buttonCount = await buttons.count();
      expect(buttonCount, 'Text-only approval must have zero action buttons').toBe(0);

      const text = await approvalRow
        .locator(MESSAGE_SELECTORS.messageText)
        .first()
        .textContent();
      expect(text?.toLowerCase()).toContain('approval request');

      await captureStep(page, APPROVAL_SCENARIO, 3, 'no-buttons-confirmed');
      console.log(
        `[${APPROVAL_SCENARIO}] Text-only approval confirmed: no buttons, text visible.`,
      );
    },
  );

  test(
    'S-T3-AUTO-012: all text-only messages include @-mention reply instructions',
    async ({ page }) => {
      if (!envReady()) {
        test.skip();
        return;
      }

      const f = requireFixtures();

      await navigateToChannel(page, testChannel());
      await navigateToThread(page, f.anchorTs);
      await captureStep(page, INSTRUCTION_SCENARIO, 1, 'thread-opened');

      const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
      await isVisibleWithin(threadPanel, 10_000);

      // Check all three messages for @-mention instruction text.
      for (const { ts, label } of [
        { ts: f.promptTs, label: 'prompt' },
        { ts: f.waitTs, label: 'wait' },
        { ts: f.approvalTs, label: 'approval' },
      ]) {
        const row = threadPanel.locator(`[data-item-key="${ts}"]`).first();
        const visible = await isVisibleWithin(row, 10_000);
        expect(visible, `${label} message must be visible`).toBe(true);

        const text = await row
          .locator(MESSAGE_SELECTORS.messageText)
          .first()
          .textContent();
        expect(
          text?.toLowerCase(),
          `${label} must include @agent-intercom instruction`,
        ).toContain('@agent-intercom');
      }

      await captureStep(page, INSTRUCTION_SCENARIO, 2, 'instructions-validated');
      console.log(
        `[${INSTRUCTION_SCENARIO}] All 3 messages contain @agent-intercom reply instructions.`,
      );
    },
  );
});
