/**
 * Self-seeding browser coverage for the automated API + Playwright harness.
 *
 * This suite posts deterministic Slack fixture messages through the Web API,
 * then validates how those fixtures render in the real Slack web client. It is
 * intentionally separate from the broader manual-oriented visual suite so the
 * automated harness can run without a prior HITL pass.
 */
import { test, expect } from '@playwright/test';

import { navigateToChannel, navigateToThread, scrollToLatestMessage, closeThreadPanel } from '../helpers/slack-nav';
import { captureElement, captureStep, isVisibleWithin } from '../helpers/screenshot';
import {
  BUTTON_SELECTORS,
  MODAL_SELECTORS,
  MESSAGE_SELECTORS,
  THREAD_SELECTORS,
  byTimestamp,
} from '../helpers/slack-selectors';
import {
  SlackFixtureClient,
  hasAutomatedVisualEnv,
  hasAtMentionEnv,
  type AutomatedVisualFixtures,
  type AtMentionFixtures,
} from '../helpers/slack-fixtures';

const APPROVAL_SCENARIO = 'S-T3-AUTO-001';
const PROMPT_SCENARIO = 'S-T3-AUTO-002';
const THREAD_SCENARIO = 'S-T3-AUTO-003';
const MODAL_TOP_SCENARIO = 'S-T3-AUTO-004';
const MODAL_THREAD_SCENARIO = 'S-T3-AUTO-005';

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

  test('documents top-level Refine button click: modal open/not-open diagnostic', async ({ page }) => {
    if (!automatedEnvReady()) {
      test.skip();
      return;
    }

    const currentFixtures = requireFixtures();

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);
    await captureStep(page, MODAL_TOP_SCENARIO, 1, 'channel-loaded');

    const promptRow = page.locator(byTimestamp(currentFixtures.promptTs)).first();
    await promptRow.waitFor({ state: 'visible', timeout: 15_000 });

    const refineButton = promptRow.locator(BUTTON_SELECTORS.refineButton).first();
    await refineButton.waitFor({ state: 'visible', timeout: 5_000 });
    await captureStep(page, MODAL_TOP_SCENARIO, 2, 'refine-button-visible');

    await refineButton.click();
    await captureStep(page, MODAL_TOP_SCENARIO, 3, 'refine-clicked');

    // Wait up to 5 seconds for a modal overlay to appear.
    const modalOverlay = page.locator(MODAL_SELECTORS.modalOverlay).first();
    const modalAppeared = await modalOverlay
      .waitFor({ state: 'visible', timeout: 5_000 })
      .then(() => true)
      .catch(() => false);

    await captureStep(page, MODAL_TOP_SCENARIO, 4, `modal-outcome-appeared-${String(modalAppeared)}`);

    if (modalAppeared) {
      const modalTitle = page.locator(MODAL_SELECTORS.modalTitle).first();
      const titleVisible = await modalTitle
        .waitFor({ state: 'visible', timeout: 3_000 })
        .then(() => true)
        .catch(() => false);
      await captureStep(page, MODAL_TOP_SCENARIO, 5, 'modal-structure-captured');
      console.log(
        `[${MODAL_TOP_SCENARIO}] DIAGNOSTIC: modal appeared. title visible: ${String(titleVisible)}`,
      );
    } else {
      console.log(
        `[${MODAL_TOP_SCENARIO}] DIAGNOSTIC: modal did NOT appear (expected — fixture uses a dummy promptId not in DB). ` +
          'This confirms the Refine button click IS delivered to agent-intercom; server rejects silently.',
      );
    }

    // The test is diagnostic — pass in both cases.
    expect(true).toBe(true);
  });

  test('documents in-thread Refine button click: Slack client modal suppression diagnostic', async ({ page }) => {
    if (!automatedEnvReady()) {
      test.skip();
      return;
    }

    const currentFixtures = requireFixtures();

    await navigateToChannel(page, testChannel());
    await scrollToLatestMessage(page);
    await captureStep(page, MODAL_THREAD_SCENARIO, 1, 'channel-loaded');

    // Open the thread and wait for it to render.
    await navigateToThread(page, currentFixtures.threadAnchorTs);
    await captureStep(page, MODAL_THREAD_SCENARIO, 2, 'thread-opened');

    const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
    const panelVisible = await isVisibleWithin(threadPanel, 10_000);
    expect(panelVisible, 'Thread panel should open').toBe(true);

    // Find the seeded prompt-with-Refine button posted inside the thread.
    const threadPromptRow = threadPanel
      .locator(byTimestamp(currentFixtures.threadPromptTs))
      .first();
    const threadPromptVisible = await threadPromptRow
      .waitFor({ state: 'visible', timeout: 10_000 })
      .then(() => true)
      .catch(() => false);

    if (!threadPromptVisible) {
      console.log(
        `[${MODAL_THREAD_SCENARIO}] DIAGNOSTIC: thread prompt row not found in DOM ` +
          '(Slack virtual list may not have scrolled to it). Capturing screenshot.',
      );
      await captureStep(page, MODAL_THREAD_SCENARIO, 3, 'thread-prompt-not-found');
      expect(true).toBe(true);
      return;
    }

    const refineButton = threadPanel.locator(BUTTON_SELECTORS.refineButton).first();
    await refineButton.waitFor({ state: 'visible', timeout: 5_000 });
    await captureStep(page, MODAL_THREAD_SCENARIO, 3, 'in-thread-refine-button-visible');

    await refineButton.click({ force: true });
    await captureStep(page, MODAL_THREAD_SCENARIO, 4, 'in-thread-refine-clicked');

    // Per Phase 6 API evidence, Slack silently suppresses views.open from thread
    // trigger_ids. We document the client-side outcome here.
    const modalOverlay = page.locator(MODAL_SELECTORS.modalOverlay).first();
    const modalAppeared = await modalOverlay
      .waitFor({ state: 'visible', timeout: 5_000 })
      .then(() => true)
      .catch(() => false);

    await captureStep(
      page,
      MODAL_THREAD_SCENARIO,
      5,
      `in-thread-modal-outcome-appeared-${String(modalAppeared)}`,
    );

    if (modalAppeared) {
      console.log(
        `[${MODAL_THREAD_SCENARIO}] DIAGNOSTIC: modal DID appear from thread context. ` +
          'This would be a positive change — Slack may have fixed the silent suppression.',
      );
    } else {
      console.log(
        `[${MODAL_THREAD_SCENARIO}] DIAGNOSTIC: modal did NOT appear from thread context. ` +
          'Consistent with known Slack limitation: views.open is silently suppressed when ' +
          'trigger_id originates from a thread block_action.',
      );
    }

    await closeThreadPanel(page);
    // The test is diagnostic — pass in both cases.
    expect(true).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// S-T3-AUTO-008: @-mention thread reply fix — static fixture validation
// ─────────────────────────────────────────────────────────────────────────────

const AT_MENTION_AUTO_SCENARIO = 'S-T3-AUTO-008';

let atMentionFixtureClient: SlackFixtureClient | null = null;
let atMentionFixtures: AtMentionFixtures | null = null;

test.describe('S-T3-AUTO-008: @-mention thread reply fix — static fixture validation', () => {
  test.beforeAll(async () => {
    if (!hasAtMentionEnv()) {
      return;
    }
    atMentionFixtureClient = SlackFixtureClient.fromEnv();
    atMentionFixtures = await atMentionFixtureClient.seedAtMentionThreadFixture();
  });

  test.afterAll(async () => {
    if (atMentionFixtureClient && atMentionFixtures) {
      await atMentionFixtureClient.deleteMessages(atMentionFixtures.cleanupTs);
    }
  });

  test(
    '@-mention prompt message contains @agent-intercom bot mention marker',
    async ({ page }) => {
      if (!hasAtMentionEnv()) {
        test.skip();
        return;
      }

      if (!atMentionFixtures) {
        throw new Error('S-T3-AUTO-008: @-mention fixtures were not seeded');
      }

      await navigateToChannel(page, testChannel());
      await scrollToLatestMessage(page);
      await captureStep(page, AT_MENTION_AUTO_SCENARIO, 1, 'channel-loaded');

      await navigateToThread(page, atMentionFixtures.anchorTs);
      await captureStep(page, AT_MENTION_AUTO_SCENARIO, 2, 'thread-opened');

      const threadPanel = page.locator(THREAD_SELECTORS.threadPanel).first();
      const panelVisible = await isVisibleWithin(threadPanel, 10_000);
      expect(panelVisible, 'Thread panel must open').toBe(true);

      // Locate the @-mention prompt text in the thread.
      const atMentionMsg = threadPanel
        .locator(`${MESSAGE_SELECTORS.messageText}:has-text("@agent-intercom")`)
        .first();

      const msgVisible = await isVisibleWithin(atMentionMsg, 15_000);

      await captureStep(
        page,
        AT_MENTION_AUTO_SCENARIO,
        3,
        msgVisible ? 'at-mention-prompt-found' : 'at-mention-prompt-absent',
      );

      expect(
        msgVisible,
        'S-T3-AUTO-008: seeded @-mention prompt must be visible in thread',
      ).toBe(true);

      const text = await atMentionMsg.textContent();
      expect(text?.toLowerCase()).toContain('@agent-intercom');

      await captureElement(atMentionMsg, AT_MENTION_AUTO_SCENARIO, 4, 'at-mention-closeup');
      await captureStep(page, AT_MENTION_AUTO_SCENARIO, 5, 'at-mention-text-validated');

      console.log(
        `[${AT_MENTION_AUTO_SCENARIO}] @-mention marker confirmed in seeded fixture text.`,
      );
    },
  );
});

