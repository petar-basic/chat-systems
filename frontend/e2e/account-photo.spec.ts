import { test, expect } from '@playwright/test';
import { API, devWorkspace, login, userContext } from './helpers';

const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
  'base64',
);

test('an account photo is stored on the account and shows to people in other workspaces', async ({
  page,
}) => {
  const { ctx: charlie } = await userContext('charlie@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(charlie);

  try {
    await login(page, 'charlie@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await page.getByTestId('open-you-menu').click();
    await page.getByTestId('open-preferences').click();
    await page.getByTestId('preferences-tab-account').click();

    const uploaded = page.waitForResponse(
      (res) => res.url().endsWith('/users/me/avatar') && res.request().method() === 'POST',
    );
    await page.getByLabel('Profile photo file').setInputFiles({
      name: 'me.png',
      mimeType: 'image/png',
      buffer: PNG,
    });
    const res = await uploaded;
    expect(res.status()).toBe(200);
    const me = await res.json();
    expect(me.avatar_url).toContain(`/api/files/download/avatars/${me.id}/`);
    await expect(page.getByText('Photo updated.')).toBeVisible();

    const path = me.avatar_url.slice(me.avatar_url.indexOf('/files/download/'));
    const seen = await bob.get(`${API}${path}`);
    expect(seen.status(), 'a coworker sees the photo').toBe(200);
    expect(seen.headers()['content-type']).toBe('image/png');
  } finally {
    await charlie.patch(`${API}/users/me`, { data: { avatar_url: '' } });
    await charlie.dispose();
    await bob.dispose();
  }
});
