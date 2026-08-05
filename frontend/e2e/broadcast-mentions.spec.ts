import { test, expect } from '@playwright/test';
import { API, login, userContext } from './helpers';

test('@channel reaches a member who was never named', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');

  const workspace = (await (await bob.get(`${API}/workspaces`)).json()).data[0];
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `broadcast-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  try {
    await admin.post(`${API}/channels/${channelId}/members`, { data: { user_id: bobId } });

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await expect(page.getByRole('navigation').getByText(`broadcast-${stamp}`)).toBeVisible();
    await expect(
      page.locator(`[data-qa="channel-mention-badge"][data-channel-id="${channelId}"]`),
    ).toHaveCount(0);

    const posted = await admin.post(`${API}/channels/${channelId}/messages`, {
      data: { content: `@[channel](channel) standup in five — ${stamp}` },
    });
    expect(posted.status(), 'admin broadcasts to the channel').toBe(200);

    await expect(
      page.locator(`[data-qa="channel-mention-badge"][data-channel-id="${channelId}"]`),
    ).toBeVisible();

    const notifications = await (await bob.get(`${API}/workspaces/${workspace.id}/notifications`)).json();
    const forThisChannel = notifications.data.filter(
      (n: { data: { channel_id?: string } }) => n.data?.channel_id === channelId,
    );
    expect(forThisChannel.length, 'the broadcast left bob a notification he can open').toBe(1);
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('a plain message leaves uninvolved members alone', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');

  const workspace = (await (await bob.get(`${API}/workspaces`)).json()).data[0];
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `quiet-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  try {
    await admin.post(`${API}/channels/${channelId}/members`, { data: { user_id: bobId } });

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await expect(page.getByRole('navigation').getByText(`quiet-${stamp}`)).toBeVisible();

    await admin.post(`${API}/channels/${channelId}/messages`, {
      data: { content: `just talking to myself — ${stamp}` },
    });

    await expect(page.getByRole('navigation').getByText(`quiet-${stamp}`)).toBeVisible();
    await expect(
      page.locator(`[data-qa="channel-mention-badge"][data-channel-id="${channelId}"]`),
    ).toHaveCount(0);
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});
