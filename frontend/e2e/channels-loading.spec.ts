import { test, expect } from '@playwright/test';
import { login, userContext, devWorkspace } from './helpers';

test('a workspace still loading its channels does not say it has none', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);

  try {
    await login(page, 'alice@dev.local');

    let release = () => {};
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      (url) => url.pathname.endsWith(`/workspaces/${workspace.id}/channels`),
      async (route) => {
        await held;
        await route.continue();
      },
    );

    await page.goto(`/app/${workspace.id}`);
    await expect(page.locator('[data-qa="workspace-tile"]').first()).toBeVisible();
    await page.waitForTimeout(500);
    await expect(page.getByText('No channels yet')).toHaveCount(0);

    release();
    await expect(page.locator('[data-qa="message-list"]')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByText('No channels yet')).toHaveCount(0);
  } finally {
    await alice.dispose();
  }
});
