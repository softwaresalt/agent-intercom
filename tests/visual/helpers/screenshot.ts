/**
 * Screenshot capture utilities for agent-intercom Playwright visual tests.
 *
 * All screenshot file names follow the convention:
 *   `{scenarioId}_{stepNumber}_{description}_{timestamp}.png`
 *
 * Example: `S-T3-005_01_modal-opened_1700000000000.png`
 *
 * Screenshots are written to the `screenshots/` directory relative to this
 * project root (i.e., `tests/visual/screenshots/`).
 */
import { type Page, type Locator } from '@playwright/test';
import * as path from 'path';
import * as fs from 'fs';

const SCREENSHOTS_DIR = path.resolve(__dirname, '..', 'screenshots');

/** Ensure the screenshots directory exists on first use. */
function ensureScreenshotsDir(): void {
  if (!fs.existsSync(SCREENSHOTS_DIR)) {
    fs.mkdirSync(SCREENSHOTS_DIR, { recursive: true });
  }
}

/**
 * Sanitize a string for use as a file name component.
 * Replaces spaces and non-alphanumeric characters (except hyphens) with hyphens.
 */
function sanitizeForFilename(value: string): string {
  return value
    .toLowerCase()
    .replace(/\s+/g, '-')
    .replace(/[^a-z0-9\-_]/g, '')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '');
}

/**
 * Build a screenshot file path following the naming convention:
 *   `{scenarioId}_{stepNumber:02d}_{description}_{timestamp}.png`
 *
 * @param scenarioId  - Scenario identifier, e.g. `S-T3-005`
 * @param stepNumber  - Step number within the scenario (used for ordering)
 * @param description - Human-readable step description
 * @returns Absolute file path for the screenshot
 */
export function buildScreenshotPath(
  scenarioId: string,
  stepNumber: number,
  description: string,
): string {
  ensureScreenshotsDir();
  const ts = Date.now();
  const step = String(stepNumber).padStart(2, '0');
  const desc = sanitizeForFilename(description);
  const fileName = `${sanitizeForFilename(scenarioId)}_${step}_${desc}_${ts}.png`;
  return path.join(SCREENSHOTS_DIR, fileName);
}

/**
 * Capture a full-page screenshot at a named step within a scenario.
 *
 * The resulting file is saved to `screenshots/` using the naming convention:
 *   `{scenarioId}_{stepNumber:02d}_{description}_{timestamp}.png`
 *
 * @param page        - Playwright page instance
 * @param scenarioId  - Scenario identifier, e.g. `S-T3-005`
 * @param stepNumber  - Step number within the scenario (determines file order)
 * @param description - Short human-readable description of this step
 * @returns Absolute path of the saved screenshot file
 *
 * @example
 * const filePath = await captureStep(page, 'S-T3-005', 1, 'modal-opened');
 * // → screenshots/s-t3-005_01_modal-opened_1700000000000.png
 */
export async function captureStep(
  page: Page,
  scenarioId: string,
  stepNumber: number,
  description: string,
): Promise<string> {
  const filePath = buildScreenshotPath(scenarioId, stepNumber, description);
  await page.screenshot({ path: filePath, fullPage: false });
  console.log(`[screenshot] ${path.basename(filePath)}`);
  return filePath;
}

/**
 * Capture a screenshot of a specific element rather than the full page.
 *
 * Useful for isolating a single message, button, or modal from the rest of
 * the channel view.
 *
 * @param locator     - Playwright locator targeting the element to capture
 * @param scenarioId  - Scenario identifier
 * @param stepNumber  - Step number within the scenario
 * @param description - Short human-readable description
 * @returns Absolute path of the saved screenshot file
 */
export async function captureElement(
  locator: Locator,
  scenarioId: string,
  stepNumber: number,
  description: string,
): Promise<string> {
  const filePath = buildScreenshotPath(scenarioId, stepNumber, description);
  await locator.screenshot({ path: filePath });
  console.log(`[screenshot] ${path.basename(filePath)}`);
  return filePath;
}

/**
 * Check whether a Playwright locator matches a visible element within the
 * given timeout.
 *
 * Returns `true` if the element is visible, `false` if the timeout elapses
 * without the element becoming visible. This is a non-throwing alternative to
 * `locator.waitFor({ state: 'visible' })`.
 *
 * @param locator  - Playwright locator to check
 * @param timeout  - Maximum time to wait in milliseconds (default: 5 000 ms)
 * @returns `true` if the element became visible within the timeout
 *
 * @example
 * const modalVisible = await isVisibleWithin(page.locator('[data-qa="modal"]'), 5_000);
 * if (!modalVisible) {
 *   await captureStep(page, scenarioId, step, 'no-modal-rendered');
 * }
 */
export async function isVisibleWithin(locator: Locator, timeout = 5_000): Promise<boolean> {
  try {
    await locator.waitFor({ state: 'visible', timeout });
    return true;
  } catch {
    return false;
  }
}

/**
 * List all screenshots captured for a given scenario, sorted chronologically.
 *
 * @param scenarioId - Scenario identifier prefix to filter by
 * @returns Sorted array of absolute file paths
 */
export function listScenarioScreenshots(scenarioId: string): string[] {
  if (!fs.existsSync(SCREENSHOTS_DIR)) {
    return [];
  }
  const prefix = sanitizeForFilename(scenarioId);
  return fs
    .readdirSync(SCREENSHOTS_DIR)
    .filter((f) => f.startsWith(prefix) && f.endsWith('.png'))
    .sort()
    .map((f) => path.join(SCREENSHOTS_DIR, f));
}
