import { test, expect } from '@playwright/test';

import { continueInBrowserIfPrompted, waitForSlackWorkspace } from '../helpers/slack-auth';

test.use({ storageState: { cookies: [], origins: [] } });

test.describe('Slack auth browser handoff', () => {
  test('continueInBrowserIfPrompted clicks the browser handoff link', async ({ page }) => {
    await page.setContent(`
      <html>
        <body>
          <a id="browser-link" href="#browser">Use Slack in your browser</a>
        </body>
      </html>
    `);

    const clicked = await continueInBrowserIfPrompted(page);

    expect(clicked).toBe(true);
    await expect(page).toHaveURL(/#browser$/);
  });

  test('waitForSlackWorkspace clicks browser handoff and accepts alternate Slack app selectors', async ({
    page,
  }) => {
    await page.setContent(`
      <html>
        <body>
          <a id="browser-link" href="javascript:void(0)">Continue in browser</a>
          <script>
            document.getElementById('browser-link').addEventListener('click', () => {
              setTimeout(() => {
                const appShell = document.createElement('div');
                appShell.setAttribute('data-qa', 'message_list');
                appShell.textContent = 'workspace loaded';
                document.body.appendChild(appShell);
              }, 50);
            });
          </script>
        </body>
      </html>
    `);

    await waitForSlackWorkspace(page, 2000);

    await expect(page.locator('[data-qa="message_list"]')).toBeVisible();
  });

  test('waitForSlackWorkspace dismisses app redirect prompt before continuing in browser', async ({
    page,
  }) => {
    await page.setContent(`
      <html>
        <body>
          <div id="redirect-modal" role="dialog">
            <button id="cancel-open">Cancel</button>
          </div>
          <a id="browser-link" href="javascript:void(0)" hidden>Use Slack in your browser</a>
          <script>
            document.getElementById('cancel-open').addEventListener('click', () => {
              document.getElementById('redirect-modal').remove();
              document.getElementById('browser-link').hidden = false;
            });
            document.getElementById('browser-link').addEventListener('click', () => {
              setTimeout(() => {
                const appShell = document.createElement('div');
                appShell.setAttribute('data-qa', 'channel_search_input');
                appShell.textContent = 'workspace loaded';
                document.body.appendChild(appShell);
              }, 50);
            });
          </script>
        </body>
      </html>
    `);

    await waitForSlackWorkspace(page, 3000);

    await expect(page.locator('[data-qa="channel_search_input"]')).toBeVisible();
  });
});
