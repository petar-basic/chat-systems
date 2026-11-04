import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test('the time zone comes from the browser, and the 12-hour clock changes message times', async ({
  page,
}) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'clock');
  const content = `clock-${Date.now()}`;
  await alice.post(`${API}/channels/${channel.id}/messages`, { data: { content } });

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    const time = page
      .locator('[data-qa="message-row"]', { hasText: content })
      .getByTestId('message-header-time');
    await expect(time).toBeVisible({ timeout: 20_000 });
    await expect(time).not.toHaveText(/am|pm/i);

    const browserZone = await page.evaluate(() => Intl.DateTimeFormat().resolvedOptions().timeZone);
    await expect
      .poll(async () => (await (await alice.get(`${API}/users/me`)).json()).timezone)
      .toBe(browserZone);

    await page.getByTestId('open-you-menu').click();
    await page.getByTestId('open-preferences').click();
    await page.getByRole('tab', { name: 'Language & region' }).click();
    await expect(page.getByTestId('timezone-picker')).toHaveValue(browserZone);
    await page.getByTestId('clock-h12').click();
    await expect(page.getByTestId('clock-h12').getByRole('radio')).toBeChecked();
    await page.keyboard.press('Escape');

    await expect(time).toHaveText(/am|pm/i);
  } finally {
    await alice.patch(`${API}/users/me/preferences`, { data: { clock: 'h24' } });
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
