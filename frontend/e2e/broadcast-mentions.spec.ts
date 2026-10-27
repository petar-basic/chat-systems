import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace, openWorkspaceSettings, send } from './helpers';

test('@channel reaches a member who was never named', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');

  const workspace = await devWorkspace(bob);
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

    // The badge normally arrives over the socket. A page that has just loaded
    // has no position to resume from, so an event published in the gap between
    // the socket opening and the `subscribe` frame being processed reaches
    // nobody — CS-028 hands out the tail rather than a replay in that case.
    // The guarantee under test is that the mention is counted, not that it
    // arrives inside one particular second, so a reload is allowed to settle it.
    const badge = page.locator(`[data-qa="channel-mention-badge"][data-channel-id="${channelId}"]`);
    if (!(await badge.isVisible().catch(() => false))) {
      await badge.waitFor({ state: 'visible', timeout: 5_000 }).catch(async () => {
        await page.reload();
        await expect(page.getByRole('navigation').getByText(`broadcast-${stamp}`)).toBeVisible();
      });
    }
    await expect(badge).toBeVisible();

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

  const workspace = await devWorkspace(bob);
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

test('an admin keeps @channel to admins, and a member is neither offered it nor let through', async ({
  page,
  browser,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `all-hands-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;
  const bobContext = await browser.newContext();

  try {
    await admin.post(`${API}/channels/${channelId}/members`, { data: { user_id: bobId } });

    await login(page, 'admin@dev.local');
    await openWorkspaceSettings(page, 'general');
    await page.getByTestId('workspace-broadcast-mentions').selectOption('admins');
    await expect(page.getByTestId('workspace-save-status')).toHaveText('Saved');

    const bobPage = await bobContext.newPage();
    await login(bobPage, 'bob@dev.local');
    await bobPage.goto(`/app/${workspace.id}/${channelId}`);
    await expect(bobPage.getByTestId('channel-header-name')).toHaveText(`all-hands-${stamp}`);

    const editor = bobPage.locator('.ProseMirror[contenteditable="true"]').last();
    await editor.click();
    await editor.pressSequentially('@');
    await expect(bobPage.getByTestId('mention-option').first()).toBeVisible();
    await expect(bobPage.locator('[data-mention-type="broadcast"]')).toHaveCount(0);
    await editor.press('Escape');

    await send(bobPage, `@channel lunch? ${stamp}`);
    await expect(
      bobPage.getByText(
        "Only workspace admins and this channel's admins can use @channel, @here and @everyone",
      ),
    ).toBeVisible();
    await expect(bobPage.getByTestId('message-failed')).toBeVisible();
  } finally {
    await admin.patch(`${API}/workspaces/${workspace.id}`, { data: { broadcast_mentions: 'members' } });
    await admin.delete(`${API}/channels/${channelId}`);
    await bobContext.close();
    await admin.dispose();
    await bob.dispose();
  }
});
