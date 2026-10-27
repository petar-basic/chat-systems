import { test, expect } from '@playwright/test';
import { login } from './helpers';

test('a deploy while the tab is open offers a reload, and the reload takes it', async ({ page }) => {
  await login(page, 'alice@dev.local');

  const served = await (await page.request.get('/version.json')).json();
  expect(served.version, 'the build publishes its version').toEqual(expect.any(String));
  const index = await page.request.get('/');
  expect(index.headers()['cache-control'], 'the page itself is never cached').toBe('no-cache');

  await page.route('**/version.json', (route) =>
    route.fulfill({
      contentType: 'application/json',
      body: JSON.stringify({ version: `${served.version}-next` }),
    }),
  );
  await expect(page.locator('[data-qa="new-version-banner"]')).toHaveCount(0);
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));

  const banner = page.locator('[data-qa="new-version-banner"]');
  await expect(banner).toBeVisible();
  await expect(banner).toContainText('A new version is available');

  await banner.locator('[data-qa="new-version-reload"]').click();
  await expect(page.locator('.ProseMirror[contenteditable="true"]').last()).toBeVisible({ timeout: 20_000 });
  await expect(banner).toHaveCount(0);
});
