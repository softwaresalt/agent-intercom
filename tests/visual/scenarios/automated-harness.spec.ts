/**
 * Self-seeding browser coverage for the automated API + Playwright harness.
 *
 * This suite posts deterministic Slack fixture messages through the Web API,
 * then validates how those fixtures render in the real Slack web client. It is
 * intentionally separate from the broader manual-oriented visual suite so the
 * automated harness can run without a prior HITL pass.
 */
import { test, expect } from '@playwright/test';

import { navigateToChannel, navigateToThread, scrollToLatestMessage } from '../helpers/slack-nav';
import { captureElement, captureStep, isVisibleWithin } from '../helpers/screenshot';
import {
  BUTTON_SELECTORS,
  MESSAGE_SELECTORS,
  THREAD_SELECTORS,
  byTimestamp,
} from '../helpers/slack-selectors';
import {
  SlackFixtureClient,
  hasAutomatedVisualEnv,
  type AutomatedVisualFixtures,
} from '../helpers/slack-fixtures';

const APPROVAL_SCENARIO = 'S-T3-AUTO-001';
const PROMPT_SCENARIO = 'S-T3-AUTO-002';
const THREAD_SCENARIO = 'S-T3-AUTO-003';

const testChannel = (): string => process.env.SLACK_TEST_CHANNEL ?? 'agent-intercom-test';

let fixtureClient: SlackFixtureClient | null = null;
let fixtures: AutomatedVisualFixtures | null = null;

function automatedEnvReady(): boolean {
  return hasAutomatedVisualEnv();
}

function requireFixtures(): AutomatedVisualFixtures {
  if (!fixtures) {
    throw new Error('automated visual fixtures were not seeded');
  }

  return fixtures;
}

test.describe('Automated visual harness fixtures', () => {
  test.beforeAll(async () => {
    if (!automatedEnvReady()) {
      return;
    }

    fixtureClient = SlackFixtureClient.fromEnv();
    fixtures = await fixtureClient.seedAutomatedVisualFixtures();
  });

  test.afterAll(async () => {
    if (fixtureClient && fixtures) {
      await fixtureClient.deleteMessages(fixtures.cleanupTs);
    }
  });

  test('renders a seeded approval fixture with buttons and diff block', async ({ page }) => {
    if (!automatedEnvReady()) {
      test.skip();
      return;
    }

    const currentFixtures = requireFixtures();

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);
    await captureStep(page, APPROVAL_SCENARIO, 1, 'channel-loaded');

    const approvalRow = page.locator(byTimestamp(currentFixtures.approvalTs)).first();
    const approvalVisible = await isVisibleWithin(approvalRow, 15_000);
    expect(approvalVisible, 'Expected the seeded approval fixture to be visible').toBe(true);

    await captureElement(approvalRow, APPROVAL_SCENARIO, 2, 'approval-fixture-row');

    const acceptButton = approvalRow.locator(BUTTON_SELECTORS.acceptButton).first();
    const rejectButton = approvalRow.locator(BUTTON_SELECTORS.rejectButton).first();
    const diffBlock = approvalRow.locator(MESSAGE_SELECTORS.codeBlock).first();

    await expect(acceptButton).toBeVisible();
    await expect(rejectButton).toBeVisible();
    await expect(diffBlock).toBeVisible();

    await captureStep(page, APPROVAL_SCENARIO, 3, 'approval-fixture-validated');
  });

  test('renders a seeded continuation prompt with all action buttons', async ({ page }) => {
    if (!automatedEnvReady()) {
      test.skip();
      return;
    }

    const currentFixtures = requireFixtures();

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);
    await captureStep(page, PROMPT_SCENARIO, 1, 'channel-loaded');

    const promptRow = page.locator(byTimestamp(currentFixtures.promptTs)).first();
    const promptVisible = await isVisibleWithin(promptRow, 15_000);
    expect(promptVisible, 'Expected the seeded prompt fixture to be visible').toBe(true);

    await captureElement(promptRow, PROMPT_SCENARIO, 2, 'prompt-fixture-row');

    const continueButton = promptRow.locator(BUTTON_SELECTORS.continueButton).first();
    const refineButton = promptRow.locator(BUTTON_SELECTORS.refineButton).first();
    const stopButton = promptRow.locator(BUTTON_SELECTORS.stopButton).first();

    await expect(continueButton).toBeVisible();
    await expect(refineButton).toBeVisible();
    await expect(stopButton).toBeVisible();

    await captureStep(page, PROMPT_SCENARIO, 3, 'prompt-fixture-validated');
  });

  test('renders the seeded thread fallback inside the thread pane', async ({ page }) => {
    if (!automatedEnvReady()) {
      test.skip();
      return;
    }

    const currentFixtures = requireFixtures();

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);
    await captureStep(page, THREAD_SCENARIO, 1, 'channel-loaded');

    await navigateToThread(page, currentFixtures.threadAnchorTs);
    await captureStep(page, THREAD_SCENARIO, 2, 'thread-opened');

    const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
    const panelVisible = await isVisibleWithin(threadPanel, 10_000);
    expect(panelVisible, 'Expected the thread panel to open for the seeded anchor').toBe(true);

    const fallbackMessage = threadPanel
      .locator(`${MESSAGE_SELECTORS.messageText}:has-text("reply in this thread")`)
      .first();
    const composer = threadPanel.locator(THREAD_SELECTORS.threadComposer).first();

    await expect(fallbackMessage).toBeVisible();
    await expect(composer).toBeVisible();

    await captureElement(fallbackMessage, THREAD_SCENARIO, 3, 'fallback-message-row');
    await captureStep(page, THREAD_SCENARIO, 4, 'thread-fallback-validated');
  });
});
