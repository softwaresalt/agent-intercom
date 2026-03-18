/**
 * Slack DOM selector strategies for Playwright visual tests.
 *
 * Selectors are ordered by stability:
 *   1. `data-qa` attributes  — most stable, Slack uses them for internal QA
 *   2. `aria-label` / role   — accessibility attributes, moderately stable
 *   3. class-based patterns  — least stable, may break on Slack client updates
 *
 * Each exported object documents the selectors it contains and notes which
 * may be affected by Slack client version changes.
 *
 * IMPORTANT: Slack's web client is a heavily modified Electron/SPA application.
 * These selectors were validated against Slack web client circa 2024–2026. If
 * a selector stops matching, first check the `data-qa` attribute using the
 * browser's DevTools element inspector — Slack frequently adds/changes these
 * for their own test suite.
 */

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

/**
 * Selectors for interactive Block Kit action buttons rendered in messages.
 *
 * Button selectors rely on `data-qa` attributes on the button container and
 * text matching on the button label. The `data-block-id` pattern is available
 * on the outer block element but is not stable across different message types.
 */
export const BUTTON_SELECTORS = {
  /** Any interactive button inside a Slack message. */
  anyMessageButton: '[data-qa="message-actions-button"], .c-button-unstyled[data-block-id]',

  /** Accept / Approve button in an approval request message. */
  acceptButton: 'button:has-text("✅ Accept"), button:has-text("Accept")',

  /** Reject button in an approval request message. */
  rejectButton: 'button:has-text("❌ Reject"), button:has-text("Reject")',

  /** Continue button in a prompt message. */
  continueButton: 'button:has-text("▶️ Continue"), button:has-text("Continue")',

  /** Refine button in a prompt message — opens a modal for instruction input. */
  refineButton: 'button:has-text("✏️ Refine"), button:has-text("Refine")',

  /** Stop Session button (appears in prompts and stall alerts). */
  stopButton: 'button:has-text("🛑 Stop Session"), button:has-text("Stop")',

  /** Nudge button in a stall alert. */
  nudgeButton: 'button:has-text("👋 Nudge"), button:has-text("Nudge")',

  /** Nudge with Instructions button in a stall alert. */
  nudgeWithInstructionsButton: 'button:has-text("💬 Nudge with Instructions"), button:has-text("Nudge with Instructions")',

  /** Resume button in a wait-for-instruction message. */
  resumeButton: 'button:has-text("▶️ Resume"), button:has-text("Resume")',

  /** Resume with Instructions button — opens a modal for instruction input. */
  resumeWithInstructionsButton: 'button:has-text("💬 Resume with Instructions"), button:has-text("Resume with Instructions")',
} as const;

// ---------------------------------------------------------------------------
// Modals
// ---------------------------------------------------------------------------

/**
 * Selectors for Slack modal dialogs (views.open).
 *
 * Modal dialogs in Slack are rendered in a separate overlay layer. The
 * `data-qa="modal"` attribute is the most reliable selector.
 *
 * NOTE: Modals triggered from within threads may silently fail to render —
 * this is the known issue being diagnosed in Phase 9. When a modal is expected
 * but absent, `modalOverlay` will not be present in the DOM.
 */
export const MODAL_SELECTORS = {
  /** The outer modal overlay container. */
  modalOverlay: '[data-qa="modal"], .c-sk-modal_portal, .p-modal_overlay',

  /** The modal dialog itself (inner frame). */
  modalDialog: '[data-qa="modal_dialog"], [role="dialog"]',

  /** Modal title text. */
  modalTitle: '[data-qa="modal_title"], .c-modal__title, h1[class*="modal"]',

  /** Text input field inside the modal. */
  textInput: '[data-qa="modal_input_text_area"], textarea[data-qa*="input"], textarea',

  /** Single-line text input inside the modal. */
  singleLineInput: '[data-qa="modal_input_field"], input[data-qa*="input"]',

  /** Submit button inside the modal. */
  submitButton: '[data-qa="modal_submit_button"], button[data-qa="submit_button"], button:has-text("Submit")',

  /** Cancel/close button inside the modal. */
  cancelButton: '[data-qa="modal_cancel_button"], button:has-text("Cancel"), button[aria-label="Close"]',
} as const;

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/**
 * Selectors for Slack message elements in the channel view.
 *
 * Message DOM structure uses virtual rendering — only messages within the
 * viewport are actually in the DOM at any given time. Scroll operations may
 * be needed before a specific message can be targeted.
 */
export const MESSAGE_SELECTORS = {
  /** Container wrapping all visible messages in the channel. */
  messageList: '[data-qa="message_list"], .c-message_list',

  /** A single message row (use `.locator(byTimestamp(ts))` to target a specific message). */
  messageRow: '[data-item-key]',

  /** The main text content area within a message. */
  messageText: '[data-qa="message-text"], .p-rich_text_section',

  /** A Block Kit section block. */
  sectionBlock: '[data-block-id], .p-block_kit_renderer__block',

  /** A Block Kit action block containing buttons. */
  actionsBlock: '[data-qa="block_actions_block"], .c-block_kit_action_block',

  /** Static text replacing buttons after an action is taken (e.g. "✅ Accepted by @user"). */
  resolvedStatus: '.p-block_kit_renderer__element--plain_text, [data-qa="block_text_element"]',

  /** Code block rendered from a Block Kit `code` element or markdown backtick fence. */
  codeBlock: 'pre.c-mrkdwn__pre, .p-mrkdwn__code_block, code',
} as const;

// ---------------------------------------------------------------------------
// Threads
// ---------------------------------------------------------------------------

/**
 * Selectors for the thread panel (flexpane) that appears on the right side
 * of the channel when a thread is opened.
 *
 * NOTE: Class names such as `p-flexpane__container` may change on Slack
 * client updates. The `data-qa` attributes are more reliable.
 */
export const THREAD_SELECTORS = {
  /** The thread panel container. */
  threadPanel: '[data-qa="threads_flexpane"], .p-flexpane__container',

  /** A single reply message row within the thread panel. */
  threadMessage: '[data-qa="threads_message"], .c-virtual_list__item',

  /** The thread composer text input area. */
  threadComposer: '[data-qa="threads_flexpane_input"], [data-qa="message_input"]',

  /** Send button within the thread composer. */
  threadSendButton: '[data-qa="threads_flexpane_submit"], button[data-qa="message_submit_button"]',

  /** Close button for the thread panel. */
  closeButton: '[data-qa="close_flexpane"], button[aria-label="Close"]',
} as const;

// ---------------------------------------------------------------------------
// Text Inputs (channel composer)
// ---------------------------------------------------------------------------

/**
 * Selectors for the main channel message composer.
 */
export const COMPOSER_SELECTORS = {
  /** The rich text editor in the main channel composer. */
  composerInput: '[data-qa="message_input"], .ql-editor[contenteditable="true"]',

  /** The send button in the main channel composer. */
  sendButton: '[data-qa="message_submit_button"], button[aria-label="Send message"]',
} as const;

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

/**
 * Selectors for workspace navigation elements.
 */
export const NAV_SELECTORS = {
  /** The quick channel/DM switcher input (opened with Ctrl+K). */
  quickSwitcher: '[data-qa="channel_search_input"], [placeholder="Jump to..."]',

  /** A result item in the quick switcher. */
  quickSwitcherResult: '[data-qa="channel_search_result_item"], .c-search_autocomplete__item',

  /** The channel sidebar (visible when signed in). */
  channelSidebar: '[data-qa="channel_sidebar"], .p-channel_sidebar',

  /** A channel row within the sidebar. */
  sidebarChannel: '[data-qa="channel_sidebar_name"], .p-channel_sidebar__name',
} as const;

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/**
 * Build a CSS attribute selector targeting a specific Slack message by its
 * `data-item-key` (which matches the `ts` from `chat.postMessage`).
 *
 * @param ts - Slack message timestamp (e.g. "1700000000.123456")
 * @returns CSS selector string
 */
export function byTimestamp(ts: string): string {
  return `[data-item-key="${ts}"]`;
}

/**
 * Build a selector for a button with a specific text label, searching within
 * a message identified by its timestamp.
 *
 * @param ts    - Slack message timestamp
 * @param label - Button text (exact or partial match via `:has-text`)
 * @returns Playwright-compatible locator string
 */
export function buttonInMessage(ts: string, label: string): string {
  return `${byTimestamp(ts)} button:has-text("${label}")`;
}
