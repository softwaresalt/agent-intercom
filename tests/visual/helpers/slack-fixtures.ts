/**
 * Slack Web API helpers for the self-seeding Playwright automation harness.
 *
 * These helpers create deterministic fixture messages inside the configured
 * test channel so browser automation can verify real Slack rendering without
 * depending on a prior manual HITL run.
 */
import { randomUUID } from 'crypto';

type SlackTextObject = {
  type: 'mrkdwn' | 'plain_text';
  text: string;
  emoji?: boolean;
};

type SlackButtonElement = {
  type: 'button';
  action_id: string;
  text: SlackTextObject;
  value: string;
};

type SlackSectionBlock = {
  type: 'section';
  text: SlackTextObject;
};

type SlackActionsBlock = {
  type: 'actions';
  block_id: string;
  elements: SlackButtonElement[];
};

type SlackBlock = SlackSectionBlock | SlackActionsBlock;

type SlackPostResponse = {
  ok?: boolean;
  error?: string;
  ts?: string;
};

type SlackDeleteResponse = {
  ok?: boolean;
  error?: string;
};

const SLACK_API_BASE = 'https://slack.com/api';

export type AutomatedVisualFixtures = {
  runId: string;
  approvalTs: string;
  promptTs: string;
  threadAnchorTs: string;
  threadPromptTs: string;
  fallbackTs: string;
  cleanupTs: string[];
};

/** Fixtures for the @-mention thread reply fix visual validation (Phase 11). */
export type AtMentionFixtures = {
  runId: string;
  /** Top-level anchor message that the thread is attached to. */
  anchorTs: string;
  /** Prompt-with-Refine button posted as a thread reply to the anchor. */
  promptTs: string;
  /** The @-mention fallback prompt posted as a second thread reply. */
  atMentionPromptTs: string;
  /** All timestamps to delete on cleanup. */
  cleanupTs: string[];
};

/** Fixtures for US17 text-only thread prompt validation. */
export type TextOnlyFixtures = {
  runId: string;
  /** Top-level anchor message that opens the thread. */
  anchorTs: string;
  /** Text-only continuation prompt (no blocks/buttons). */
  promptTs: string;
  /** Text-only wait status (no blocks/buttons). */
  waitTs: string;
  /** Text-only approval request (no blocks/buttons). */
  approvalTs: string;
  /** All timestamps to delete on cleanup. */
  cleanupTs: string[];
};

function getEnv(name: string): string | undefined {
  const value = process.env[name];
  return value && value.trim().length > 0 ? value.trim() : undefined;
}

function requireEnv(name: string): string {
  const value = getEnv(name);
  if (!value) {
    throw new Error(`${name} not set`);
  }

  return value;
}

/** Returns the first defined value among the given env var names. */
function requireOneOf(...names: string[]): string {
  for (const name of names) {
    const value = getEnv(name);
    if (value) {
      return value;
    }
  }

  throw new Error(`None of [${names.join(', ')}] are set`);
}

function sectionBlock(text: string): SlackSectionBlock {
  return {
    type: 'section',
    text: {
      type: 'mrkdwn',
      text,
    },
  };
}

function button(actionId: string, label: string, value: string): SlackButtonElement {
  return {
    type: 'button',
    action_id: actionId,
    text: {
      type: 'plain_text',
      text: label,
      emoji: true,
    },
    value,
  };
}

function buildApprovalBlocks(requestId: string, runId: string): SlackBlock[] {
  return [
    sectionBlock(
      `🟢 *Automated approval fixture*\n📄 \`src/lib.rs\` | Risk: *Low* | Run: \`${runId}\``,
    ),
    sectionBlock(
      '```diff\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n```',
    ),
    {
      type: 'actions',
      block_id: `approval_${requestId}`,
      elements: [
        button('approve_accept', 'Accept', requestId),
        button('approve_reject', 'Reject', requestId),
      ],
    },
  ];
}

function buildPromptBlocks(promptId: string, runId: string): SlackBlock[] {
  return [
    sectionBlock('🔄 *Automated continuation prompt*'),
    sectionBlock(
      `Please review the automated Playwright fixture set for run \`${runId}\` and continue if the Slack UI renders correctly.`,
    ),
    sectionBlock('⏱️ 12s elapsed | 📋 3 actions taken'),
    {
      type: 'actions',
      block_id: `prompt_${promptId}`,
      elements: [
        button('prompt_continue', 'Continue', promptId),
        button('prompt_refine', 'Refine', promptId),
        button('prompt_stop', 'Stop Session', promptId),
      ],
    },
  ];
}

export function hasAutomatedVisualEnv(): boolean {
  return Boolean(
    getEnv('SLACK_WORKSPACE_URL') &&
      getEnv('SLACK_EMAIL') &&
      getEnv('SLACK_PASSWORD') &&
      getEnv('SLACK_TEST_CHANNEL') &&
      (getEnv('SLACK_BOT_TOKEN') ?? getEnv('SLACK_TEST_BOT_TOKEN')) &&
      getEnv('SLACK_TEST_CHANNEL_ID'),
  );
}

/** Same check as `hasAutomatedVisualEnv` — the @-mention fixture needs identical env. */
export function hasAtMentionEnv(): boolean {
  return hasAutomatedVisualEnv();
}

/**
 * Minimal Slack Web API client used by the automated Playwright harness.
 */
export class SlackFixtureClient {
  public constructor(
    private readonly botToken: string,
    private readonly channelId: string,
  ) {}

  public static fromEnv(): SlackFixtureClient {
    return new SlackFixtureClient(
      requireOneOf('SLACK_BOT_TOKEN', 'SLACK_TEST_BOT_TOKEN'),
      requireEnv('SLACK_TEST_CHANNEL_ID'),
    );
  }

  private async postMessage(
    text: string,
    blocks?: SlackBlock[],
    threadTs?: string,
  ): Promise<string> {
    const body: Record<string, unknown> = {
      channel: this.channelId,
      text,
    };

    if (blocks) {
      body.blocks = blocks;
    }

    if (threadTs) {
      body.thread_ts = threadTs;
    }

    const response = await fetch(`${SLACK_API_BASE}/chat.postMessage`, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${this.botToken}`,
        'Content-Type': 'application/json; charset=utf-8',
      },
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      throw new Error(`chat.postMessage HTTP ${response.status}`);
    }

    const payload = (await response.json()) as SlackPostResponse;
    if (payload.ok !== true || !payload.ts) {
      throw new Error(`chat.postMessage error: ${payload.error ?? 'unknown'}`);
    }

    return payload.ts;
  }

  public async deleteMessages(timestamps: string[]): Promise<void> {
    const failures: string[] = [];

    for (const ts of timestamps) {
      const response = await fetch(`${SLACK_API_BASE}/chat.delete`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${this.botToken}`,
          'Content-Type': 'application/json; charset=utf-8',
        },
        body: JSON.stringify({
          channel: this.channelId,
          ts,
        }),
      });

      if (!response.ok) {
        failures.push(`chat.delete HTTP ${response.status} for ts=${ts}`);
        continue;
      }

      const payload = (await response.json()) as SlackDeleteResponse;
      if (payload.ok !== true) {
        failures.push(`chat.delete error for ts=${ts}: ${payload.error ?? 'unknown'}`);
      }
    }

    if (failures.length > 0) {
      throw new Error(failures.join('; '));
    }
  }

  public async seedAutomatedVisualFixtures(): Promise<AutomatedVisualFixtures> {
    const runId = randomUUID().split('-')[0];
    const approvalRequestId = `auto-approval-${runId}`;
    const promptId = `auto-prompt-${runId}`;

    const approvalTs = await this.postMessage(
      `[automated-visual] approval fixture ${runId}`,
      buildApprovalBlocks(approvalRequestId, runId),
    );

    const promptTs = await this.postMessage(
      `[automated-visual] prompt fixture ${runId}`,
      buildPromptBlocks(promptId, runId),
    );

    const threadAnchorTs = await this.postMessage(
      `[automated-visual] thread anchor ${runId}`,
    );


    // Post a prompt-with-Refine button INSIDE the thread for the in-thread modal diagnostic.
    const threadPromptId = `auto-thread-prompt-${runId}`;
    const threadPromptTs = await this.postMessage(
      `[automated-visual] thread prompt fixture ${runId}`,
      buildPromptBlocks(threadPromptId, runId),
      threadAnchorTs,
    );
    const fallbackTs = await this.postMessage(
      `Automated thread fallback for run ${runId}`,
      [
        sectionBlock(
          'Modal unavailable — please reply in this thread with your instructions.',
        ),
      ],
      threadAnchorTs,
    );

    return {
      runId,
      approvalTs,
      promptTs,
      threadAnchorTs,
      threadPromptTs,
      fallbackTs,
      cleanupTs: [fallbackTs, threadPromptTs, threadAnchorTs, promptTs, approvalTs],
    };
  }

  /**
   * Seed the @-mention thread reply fix fixture (Phase 11 — S-T3-AUTO-006/007/008).
   *
   * Posts three messages:
   *   1. A top-level anchor message to create a thread.
   *   2. A prompt-with-Refine button as a thread reply (in-thread Block Kit prompt).
   *   3. The @-mention fallback text as a second thread reply.
   *
   * The Playwright spec then opens the thread panel and verifies that the
   * @-mention prompt text is visible and contains `"@agent-intercom"`.
   */
  public async seedAtMentionThreadFixture(): Promise<AtMentionFixtures> {
    const runId = randomUUID().split('-')[0];
    const promptId = `at-mention-prompt-${runId}`;

    // 1. Top-level anchor — opens the thread.
    const anchorTs = await this.postMessage(
      `[automated-visual] @-mention thread anchor ${runId}`,
    );

    // 2. Prompt with Refine button posted inside the thread.
    const promptTs = await this.postMessage(
      `[automated-visual] in-thread prompt fixture ${runId}`,
      buildPromptBlocks(promptId, runId),
      anchorTs,
    );

    // 3. The @-mention fallback text the server posts after proactive thread detection.
    const atMentionText =
      `🤖 Please type your instructions as a reply mentioning @agent-intercom ` +
      `[run ${runId}]`;
    const atMentionPromptTs = await this.postMessage(atMentionText, undefined, anchorTs);

    return {
      runId,
      anchorTs,
      promptTs,
      atMentionPromptTs,
      cleanupTs: [atMentionPromptTs, promptTs, anchorTs],
    };
  }

  /**
   * Seed fixtures for US17 text-only thread prompt validation.
   *
   * Creates a thread with:
   * 1. A top-level anchor message
   * 2. A text-only prompt (no blocks/buttons) as a thread reply — mimics
   *    the US17 `forward_prompt` behavior when `session_thread_ts` is set
   * 3. A text-only wait message as a second thread reply
   * 4. A text-only approval message as a third thread reply
   */
  public async seedTextOnlyThreadFixtures(): Promise<TextOnlyFixtures> {
    const runId = randomUUID().split('-')[0];

    const anchorTs = await this.postMessage(
      `[automated-visual] US17 text-only thread anchor ${runId}`,
    );

    // Text-only prompt (US17) — no blocks, no buttons.
    const promptText =
      `🔄 *Continuation Prompt*\n` +
      `Agent needs guidance on next steps for run \`${runId}\`.\n` +
      `⏱️ 45s elapsed | 📋 7 actions taken\n\n` +
      `💬 Reply with \`@agent-intercom\` followed by: ` +
      `\`continue\`, \`refine <instructions>\`, or \`stop\``;
    const promptTs = await this.postMessage(promptText, undefined, anchorTs);

    // Text-only wait (US17) — no blocks, no buttons.
    const waitText =
      `⏸️ *Agent Waiting*\n` +
      `Idle and awaiting operator instructions for run \`${runId}\`.\n` +
      `⏱️ Timeout: 300s\n\n` +
      `💬 Reply with \`@agent-intercom\` followed by: ` +
      `\`resume [instructions]\` or \`stop\``;
    const waitTs = await this.postMessage(waitText, undefined, anchorTs);

    // Text-only approval (US17) — no blocks, no buttons.
    const approvalText =
      `🟢 *Approval Request* (low)\n` +
      `*Add error handler to config parser*\n` +
      `📄 \`src/config.rs\`\n` +
      `\`\`\`\n--- a/src/config.rs\n+++ b/src/config.rs\n` +
      `@@ -12 +12 @@\n-old line\n+new line\n\`\`\`\n\n` +
      `💬 Reply with \`@agent-intercom\` followed by: ` +
      `\`approve\` or \`reject <reason>\``;
    const approvalTs = await this.postMessage(approvalText, undefined, anchorTs);

    return {
      runId,
      anchorTs,
      promptTs,
      waitTs,
      approvalTs,
      cleanupTs: [approvalTs, waitTs, promptTs, anchorTs],
    };
  }
}



