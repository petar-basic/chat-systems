import { test, expect, type APIRequestContext, type Page } from '@playwright/test';
import { API, login, userContext, devWorkspace } from './helpers';

const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
  'base64',
);

async function generalChannel(ctx: APIRequestContext) {
  const workspace = await devWorkspace(ctx);
  const channels = (await (await ctx.get(`${API}/workspaces/${workspace.id}/channels`)).json()).data;
  return { workspace, general: channels.find((c: { name: string }) => c.name === 'general') };
}

async function openChannel(page: Page, workspaceId: string, channelId: string) {
  await page.goto(`/app/${workspaceId}/${channelId}`);
  await expect(page.locator('[data-qa="message-list"]')).toBeVisible();
}

test('an uploaded avatar replaces the initials fallback across the app', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: admin } = await userContext('admin@dev.local');

  const { workspace, general } = await generalChannel(alice);

  const upload = await alice.post(`${API}/files/upload/${workspace.id}`, {
    multipart: { file: { name: 'avatar.png', mimeType: 'image/png', buffer: PNG } },
  });
  expect(upload.status(), 'avatar upload').toBe(200);
  const avatarUrl = (await upload.json())[0].url as string;

  const saved = await alice.patch(`${API}/users/me`, { data: { avatar_url: avatarUrl } });
  expect(saved.status(), 'saving the avatar on the profile').toBe(200);

  try {
    const separator = await admin.post(`${API}/channels/${general.id}/messages`, {
      data: { content: `avatar-e2e-separator-${Date.now()}` },
    });
    expect(separator.status(), 'admin posts ahead of alice so her message is not grouped').toBe(200);

    const text = `avatar-e2e-${Date.now()}`;
    const posted = await alice.post(`${API}/channels/${general.id}/messages`, {
      data: { content: text },
    });
    expect(posted.status(), 'alice posts a message').toBe(200);

    await login(page, 'admin@dev.local');
    await openChannel(page, workspace.id, general.id);

    const row = page.locator('[data-qa="message-row"]', { hasText: text }).first();
    const senderAvatar = row.locator('[data-qa="avatar-image"]');
    await expect(senderAvatar).toBeVisible();
    expect(await senderAvatar.evaluate((img: HTMLImageElement) => img.naturalWidth)).toBeGreaterThan(0);

    const aliceInSidebar = page.getByRole('button', { name: 'Alice Johnson' }).first();
    await expect(aliceInSidebar.locator('[data-qa="avatar-image"]')).toBeVisible();

    await expect(
      page.locator('[data-qa="sidebar-profile-avatar"] [data-qa="avatar-initials"]'),
    ).toBeVisible();
  } finally {
    await alice.patch(`${API}/users/me`, { data: { avatar_url: '' } });
    await alice.dispose();
    await admin.dispose();
  }
});

test('a broken avatar url degrades to initials instead of a broken image', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { workspace, general } = await generalChannel(alice);

  const saved = await alice.patch(`${API}/users/me`, {
    data: { avatar_url: '/api/files/download/does/not/exist.png' },
  });
  expect(saved.status()).toBe(200);

  try {
    await login(page, 'admin@dev.local');
    await openChannel(page, workspace.id, general.id);
    const aliceInSidebar = page.getByRole('button', { name: 'Alice Johnson' }).first();
    await expect(aliceInSidebar.locator('[data-qa="avatar-initials"]')).toBeVisible();
  } finally {
    await alice.patch(`${API}/users/me`, { data: { avatar_url: '' } });
    await alice.dispose();
  }
});

test('the API rejects an avatar url with a hostile scheme', async () => {
  const { ctx: alice } = await userContext('alice@dev.local');

  const rejected = await alice.patch(`${API}/users/me`, {
    data: { avatar_url: 'javascript:alert(1)' },
  });
  expect(rejected.status()).toBe(422);

  await alice.dispose();
});
