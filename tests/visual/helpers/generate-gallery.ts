/**
 * Screenshot gallery generator for agent-intercom Tier 3 visual tests.
 *
 * Runs as a Playwright global teardown after the visual test suite completes,
 * and is also importable as a library for manual invocation.
 *
 * Reads all PNG files from `screenshots/`, groups them by scenario ID, and
 * generates a self-contained HTML report at `reports/gallery.html` with:
 *   - A summary table of all scenarios and screenshot counts (S-T3-012).
 *   - Scenario sections ordered alphabetically by scenario ID.
 *   - Each step's screenshot inlined as a base64 data URI.
 *   - Step labels derived from the screenshot filename's description segment.
 *
 * Screenshot naming convention (set by `helpers/screenshot.ts`):
 *   `{scenarioId}_{step:02d}_{description}_{timestamp}.png`
 *   Example: `s-t3-005_01_modal-opened_1700000000000.png`
 *
 * FRs: FR-029
 * Scenarios: S-T3-012
 */
import * as fs from 'fs';
import * as path from 'path';

const SCREENSHOTS_DIR = path.resolve(__dirname, '..', 'screenshots');
const REPORTS_DIR = path.resolve(__dirname, '..', 'reports');
const GALLERY_OUTPUT = path.join(REPORTS_DIR, 'gallery.html');

/** A parsed screenshot filename broken into its component parts. */
interface ScreenshotEntry {
  filePath: string;
  fileName: string;
  scenarioId: string;
  stepNumber: number;
  description: string;
  timestamp: number;
}

/**
 * Parse a screenshot filename into its components.
 *
 * The expected format is:
 *   `{rawScenarioId}_{step:02d}_{description}_{timestamp13}.png`
 *
 * Returns `null` if the filename does not match the expected pattern (e.g.
 * files not produced by `captureStep`).
 */
function parseScreenshotFilename(fileName: string): ScreenshotEntry | null {
  const base = fileName.replace(/\.png$/, '');
  // The timestamp is always a 13-digit Unix millisecond value at the end.
  const match = base.match(/^(.+?)_(\d{2})_(.+?)_(\d{13})$/);
  if (!match) {
    return null;
  }
  const [, rawScenarioId, rawStep, rawDescription, rawTimestamp] = match;
  return {
    filePath: path.join(SCREENSHOTS_DIR, fileName),
    fileName,
    scenarioId: rawScenarioId.toUpperCase().replace(/[_]+/g, '-'),
    stepNumber: parseInt(rawStep, 10),
    description: rawDescription.replace(/-/g, ' '),
    timestamp: parseInt(rawTimestamp, 10),
  };
}

/**
 * Read a PNG file and return its base64-encoded content.
 * Returns an empty string when the file cannot be read.
 */
function encodeScreenshot(filePath: string): string {
  try {
    return fs.readFileSync(filePath).toString('base64');
  } catch {
    return '';
  }
}

/**
 * Group screenshot entries by scenario ID.
 *
 * Within each group, entries are sorted first by step number, then by
 * timestamp to break ties (multiple screenshots at the same step number).
 */
function groupByScenario(
  entries: ScreenshotEntry[],
): Map<string, ScreenshotEntry[]> {
  const groups = new Map<string, ScreenshotEntry[]>();
  for (const entry of entries) {
    const existing = groups.get(entry.scenarioId) ?? [];
    existing.push(entry);
    groups.set(entry.scenarioId, existing);
  }
  for (const [, group] of groups) {
    group.sort((a, b) =>
      a.stepNumber !== b.stepNumber
        ? a.stepNumber - b.stepNumber
        : a.timestamp - b.timestamp,
    );
  }
  return groups;
}

/**
 * Render the HTML summary table row for a single scenario.
 */
function renderSummaryRow(scenarioId: string, count: number): string {
  return `      <tr>
        <td><a href="#${scenarioId.toLowerCase()}">${scenarioId}</a></td>
        <td>${count} screenshot${count !== 1 ? 's' : ''}</td>
      </tr>`;
}

/**
 * Render a single screenshot step block within a scenario section.
 */
function renderStep(entry: ScreenshotEntry): string {
  const encoded = encodeScreenshot(entry.filePath);
  const imgHtml = encoded
    ? `<img src="data:image/png;base64,${encoded}" alt="${entry.description}" class="screenshot" loading="lazy">`
    : `<p class="missing">Screenshot not found: ${entry.fileName}</p>`;
  return `        <div class="step">
          <p class="step-label">Step ${entry.stepNumber} — ${entry.description}</p>
          ${imgHtml}
        </div>`;
}

/**
 * Render a full scenario section (heading + step screenshots).
 */
function renderScenario(scenarioId: string, entries: ScreenshotEntry[]): string {
  const steps = entries.map(renderStep).join('\n');
  return `    <section class="scenario" id="${scenarioId.toLowerCase()}">
      <h2>${scenarioId}</h2>
      <p class="step-count">${entries.length} screenshot${entries.length !== 1 ? 's' : ''}</p>
      <div class="steps">
${steps}
      </div>
    </section>`;
}

/**
 * Build the complete HTML gallery document from the grouped screenshot entries.
 */
function buildGalleryHtml(groups: Map<string, ScreenshotEntry[]>): string {
  const scenarioIds = [...groups.keys()].sort();
  const totalScreenshots = scenarioIds.reduce(
    (sum, id) => sum + (groups.get(id)?.length ?? 0),
    0,
  );

  const summaryRows = scenarioIds
    .map((id) => renderSummaryRow(id, groups.get(id)?.length ?? 0))
    .join('\n');

  const navLinks = scenarioIds
    .map((id) => `<a href="#${id.toLowerCase()}">${id}</a>`)
    .join('\n    ');

  const scenarioSections = scenarioIds
    .map((id) => renderScenario(id, groups.get(id) ?? []))
    .join('\n\n');

  const generatedAt = new Date().toISOString();

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>agent-intercom — Tier 3 Screenshot Gallery</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: system-ui, -apple-system, sans-serif;
      max-width: 1400px;
      margin: 0 auto;
      padding: 24px;
      background: #f8f9fa;
      color: #212529;
      line-height: 1.5;
    }
    h1 { font-size: 1.75rem; border-bottom: 2px solid #dee2e6; padding-bottom: 12px; margin-bottom: 8px; }
    h2 { font-size: 1.2rem; color: #495057; margin-bottom: 4px; }
    .meta { color: #6c757d; margin-bottom: 24px; font-size: 0.875rem; }
    .summary-heading { font-size: 1.1rem; font-weight: 600; margin: 24px 0 8px; }
    table { border-collapse: collapse; width: 100%; margin-bottom: 24px; }
    th, td { border: 1px solid #dee2e6; padding: 8px 12px; text-align: left; }
    th { background: #e9ecef; font-weight: 600; }
    nav { margin-bottom: 32px; display: flex; flex-wrap: wrap; gap: 8px; }
    nav a { color: #0d6efd; text-decoration: none; font-size: 0.875rem;
            background: #e7f1ff; padding: 4px 10px; border-radius: 4px; }
    nav a:hover { text-decoration: underline; }
    .scenario {
      background: white;
      border: 1px solid #dee2e6;
      border-radius: 8px;
      padding: 20px;
      margin-bottom: 24px;
    }
    .step-count { color: #6c757d; font-size: 0.8rem; margin-bottom: 16px; }
    .steps { display: flex; flex-direction: column; gap: 20px; }
    .step { border-left: 3px solid #0d6efd; padding-left: 16px; }
    .step-label { font-size: 0.875rem; font-weight: 600; color: #495057; margin-bottom: 8px; }
    .screenshot {
      max-width: 100%;
      border: 1px solid #dee2e6;
      border-radius: 4px;
      display: block;
      box-shadow: 0 1px 3px rgba(0,0,0,0.08);
    }
    .missing { color: #dc3545; font-style: italic; font-size: 0.85rem; }
    .empty { color: #6c757d; font-style: italic; margin: 32px 0; text-align: center; }
  </style>
</head>
<body>
  <h1>agent-intercom — Tier 3 Screenshot Gallery</h1>
  <p class="meta">
    Generated: ${generatedAt} &nbsp;·&nbsp;
    ${scenarioIds.length} scenario${scenarioIds.length !== 1 ? 's' : ''} &nbsp;·&nbsp;
    ${totalScreenshots} total screenshot${totalScreenshots !== 1 ? 's' : ''}
  </p>

  <p class="summary-heading">Scenario Summary</p>
  <table>
    <thead>
      <tr><th>Scenario</th><th>Screenshots</th></tr>
    </thead>
    <tbody>
${summaryRows}
    </tbody>
  </table>

  <nav>
    ${navLinks}
  </nav>

${scenarioSections}

${totalScreenshots === 0 ? '  <p class="empty">No screenshots found in the screenshots/ directory. Run the visual test suite first.</p>' : ''}
</body>
</html>`;
}

/**
 * Generate the HTML screenshot gallery from all PNG files in `screenshots/`.
 *
 * Writes `reports/gallery.html`. Safe to call when no screenshots exist —
 * produces a valid but empty report with an explanatory message.
 *
 * @returns Number of screenshots included in the gallery.
 */
export async function generateGallery(): Promise<number> {
  fs.mkdirSync(REPORTS_DIR, { recursive: true });

  const screenshots: ScreenshotEntry[] = [];
  if (fs.existsSync(SCREENSHOTS_DIR)) {
    for (const file of fs.readdirSync(SCREENSHOTS_DIR)) {
      if (!file.endsWith('.png')) {
        continue;
      }
      const entry = parseScreenshotFilename(file);
      if (entry) {
        screenshots.push(entry);
      }
    }
  }

  const groups = groupByScenario(screenshots);
  const html = buildGalleryHtml(groups);
  fs.writeFileSync(GALLERY_OUTPUT, html, 'utf8');

  console.log(
    `[gallery] ${GALLERY_OUTPUT}: ${screenshots.length} screenshots across ${groups.size} scenarios.`,
  );
  return screenshots.length;
}

/**
 * Playwright global teardown entry point.
 *
 * Automatically invoked after all Playwright tests finish when registered as
 * `globalTeardown` in `playwright.config.ts`.
 */
export default async function teardown(): Promise<void> {
  await generateGallery();
}
