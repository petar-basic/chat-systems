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

test('a workspace photo is stored for that workspace and shows to its members only', async ({ page }) => {
  const { ctx: charlie } = await userContext('charlie@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(charlie);
  const channels = (await (await charlie.get(`${API}/workspaces/${workspace.id}/channels`)).json()).data;
  const general = channels.find((c: { name: string }) => c.name === 'general');

  try {
    await login(page, 'charlie@dev.local');
    await page.goto(`/app/${workspace.id}/${general.id}`);
    await page.locator('[data-qa="sidebar-profile-avatar"]').click();

    const uploaded = page.waitForResponse(
      (res) =>
        res.url().endsWith(`/workspaces/${workspace.id}/profile/avatar`) && res.request().method() === 'POST',
    );
    await page.getByTestId('profile-modal').getByLabel('Profile photo file').setInputFiles({
      name: 'here.png',
      mimeType: 'image/png',
      buffer: PNG,
    });
    const res = await uploaded;
    expect(res.status()).toBe(200);
    const profile = await res.json();
    expect(profile.avatar_url).toContain(`/avatars/`);
    expect(profile.avatar_url).toContain(`/${workspace.id}/`);
    await page.getByTestId('profile-save').click();
    await expect(page.getByTestId('profile-modal')).toBeHidden();

    const path = profile.avatar_url.slice(profile.avatar_url.indexOf('/files/download/'));
    expect((await bob.get(`${API}${path}`)).status(), 'a member sees it').toBe(200);
  } finally {
    await charlie.patch(`${API}/workspaces/${workspace.id}/profile`, { data: { avatar_url: '' } });
    await charlie.dispose();
    await bob.dispose();
  }
});
