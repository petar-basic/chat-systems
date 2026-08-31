import { test, expect } from '@playwright/test';
import { devWorkspace, login, userContext } from './helpers';

const PASSWORD = process.env.E2E_PASSWORD;

test.skip(!PASSWORD, 'Set E2E_PASSWORD (the seeded admin password) and run the stack to enable E2E.');

const ONE_PIXEL_PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
  'base64',
);

test('an uploaded image opens a preview instead of downloading', async ({ page }) => {
  let downloadStarted = false;
  page.on('download', () => {
    downloadStarted = true;
  });

  await login(page, 'admin@dev.local');
  const { ctx } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(ctx);
  await ctx.dispose();
  await page.goto(`/app/${workspace.id}`);
  await page
    .getByRole('button', { name: /^general$/ })
    .first()
    .click();
  await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('general');

  const chooser = page.waitForEvent('filechooser');
  await page.getByLabel('Upload file').click();
  await (
    await chooser
  ).setFiles({
    name: `preview-${Date.now()}.png`,
    mimeType: 'image/png',
    buffer: ONE_PIXEL_PNG,
  });

  const thumbnail = page.locator('[data-qa="attachment-image"]').last();
  await expect(thumbnail).toBeVisible({ timeout: 20_000 });

  const before = page.url();
  await thumbnail.click();

  const lightbox = page.locator('[data-qa="image-lightbox"]');
  await expect(lightbox).toBeVisible();
  await expect(lightbox.locator('img')).toBeVisible();
  expect(page.url(), 'the preview must not navigate away').toBe(before);
  expect(downloadStarted, 'clicking the image must not start a download').toBe(false);

  await page.keyboard.press('Escape');
  await expect(lightbox).toHaveCount(0);
});
