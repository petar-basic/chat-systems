import { test, expect, type APIRequestContext, type Page } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test.setTimeout(60_000);

const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
  'base64',
);

async function addEmoji(ctx: APIRequestContext, workspaceId: string, name: string) {
  const res = await ctx.post(`${API}/workspaces/${workspaceId}/emojis`, {
    multipart: { name, image: { name: `${name}.png`, mimeType: 'image/png', buffer: PNG } },
  });
  expect(res.status(), `upload :${name}:`).toBe(200);
  return (await res.json()) as { id: string; name: string; url: string };
}

function liveIn(page: Page, workspaceId: string, channelId: string): Promise<void> {
  return new Promise((resolve) => {
    page.on('websocket', (socket) => {
      let joined = false;
      let synced = false;
      socket.on('framesent', ({ payload }) => {
        if (
          typeof payload === 'string' &&
          payload.includes('"channel.join"') &&
          payload.includes(channelId)
        ) {
          joined = true;
        }
        if (joined && synced) resolve();
      });
      socket.on('framereceived', ({ payload }) => {
        if (
          typeof payload === 'string' &&
          payload.includes('"sync.complete"') &&
          payload.includes(workspaceId)
        ) {
          synced = true;
        }
        if (joined && synced) resolve();
      });
    });
  });
}

async function searchPicker(page: Page, query: string) {
  await page.getByRole('button', { name: 'Insert emoji' }).click();
  await page.locator('em-emoji-picker input[type="search"]').fill(query);
}

test('an emoji added by someone else renders without a reload', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'emoji-live');
  const name = `live${Date.now()}`;
  let emojiId: string | undefined;

  try {
    await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
    await login(page, 'bob@dev.local');
    const live = liveIn(page, workspace.id, channel.id);
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(channel.name);
    await live;

    emojiId = (await addEmoji(admin, workspace.id, name)).id;
    const res = await admin.post(`${API}/channels/${channel.id}/messages`, {
      data: { content: `ship it :${name}:` },
    });
    expect(res.status()).toBe(200);

    const row = page.locator('[data-qa="message-row"]', { hasText: 'ship it' });
    await expect(row.locator(`[data-qa="custom-emoji"][alt=":${name}:"]`)).toBeVisible({ timeout: 15_000 });
  } finally {
    if (emojiId) await admin.delete(`${API}/workspaces/${workspace.id}/emojis/${emojiId}`);
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test("the picker offers only the open workspace's emoji", async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const channels = (await (await admin.get(`${API}/workspaces/${workspace.id}/channels`)).json()).data;
  const general = channels.find((c: { name: string }) => c.name === 'general');
  const other = await (
    await admin.post(`${API}/workspaces`, { data: { name: `Emojiless ${Date.now()}` } })
  ).json();
  const emoji = await addEmoji(admin, workspace.id, `scoped${Date.now()}`);

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${general.id}`);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('general');

    await searchPicker(page, emoji.name);
    await expect(page.locator(`em-emoji-picker img[src="${emoji.url}"]`).first()).toBeVisible();

    await page.getByRole('button', { name: other.name, exact: true }).click();
    await expect(page.locator('em-emoji-picker')).toHaveCount(0);
    await expect(page).toHaveURL(new RegExp(`/app/${other.id}`));
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('general');

    await searchPicker(page, emoji.name);
    await expect(page.locator('em-emoji-picker').getByText(/couldn.t be found/)).toBeVisible();
    await expect(page.locator(`em-emoji-picker img[src="${emoji.url}"]`)).toHaveCount(0);
  } finally {
    await admin.delete(`${API}/workspaces/${workspace.id}/emojis/${emoji.id}`);
    await admin.delete(`${API}/workspaces/${other.id}`);
    await admin.dispose();
  }
});
