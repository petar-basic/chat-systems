import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test('the compact layout picked in preferences is still there after a reload', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'compact');
  const content = `compact-layout-${Date.now()}`;
  await alice.post(`${API}/channels/${channel.id}/messages`, { data: { content } });

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    const row = page.locator('[data-qa="message-row"]', { hasText: content });
    await expect(row).toBeVisible({ timeout: 20_000 });
    await expect(row.getByTestId('message-compact-sender')).toHaveCount(0);

    await page.getByTestId('open-you-menu').click();
    await page.getByTestId('open-preferences').click();
    await page.getByRole('tab', { name: 'Messages & media' }).click();
    await page.getByTestId('density-compact').click();
    await expect(page.getByTestId('density-compact').getByRole('radio')).toBeChecked();
    await page.keyboard.press('Escape');

    await expect(row.getByTestId('message-compact-sender')).toBeVisible();
    await expect(row.getByTestId('message-time')).toBeVisible();

    await page.reload();
    await expect(row).toBeVisible({ timeout: 20_000 });
    await expect(row.getByTestId('message-compact-sender')).toBeVisible();
  } finally {
    await alice.patch(`${API}/users/me/preferences`, { data: { density: 'clean' } });
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
