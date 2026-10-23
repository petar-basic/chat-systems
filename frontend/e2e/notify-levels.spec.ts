import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace, createChannel } from './helpers';

interface NotificationFrame {
  type: string;
  channel_id: string | null;
  message_id: string | null;
  kind: string | null;
}

test('a channel set to Nothing keeps its badge but sends no notification', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(bob);
  const channel = await createChannel(admin, workspace.id, 'quiet');

  const frames: NotificationFrame[] = [];
  page.on('websocket', (socket) => {
    socket.on('framereceived', ({ payload }) => {
      if (typeof payload !== 'string' || !payload.includes('"notification"')) return;
      const frame = JSON.parse(payload) as NotificationFrame;
      if (frame.type === 'notification') frames.push(frame);
    });
  });

  try {
    await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(channel.name);

    await page.locator('[data-qa="channel-header"] [data-qa="notification-menu-open"]').click();
    await expect(page.locator('[data-qa="notification-menu"]')).toBeVisible();
    await expect(page.locator('[data-qa="notify-level-default"]')).toHaveAttribute('aria-checked', 'true');
    await page.locator('[data-qa="notify-level-nothing"]').click();
    await expect(page.locator('[data-qa="notification-menu"]')).toHaveCount(0);

    const row = page.locator('[data-qa="sidebar-channel-row"]', { hasText: channel.name });
    await expect(row.locator('[data-qa="sidebar-channel-quiet"]')).toBeVisible();

    await page
      .getByRole('button', { name: /^general$/ })
      .first()
      .click();
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('general');

    const quiet = await admin.post(`${API}/channels/${channel.id}/messages`, {
      data: { content: `@[Bob Smith](${bobId}) quiet probe` },
    });
    expect(quiet.status(), 'admin mentions bob in the quiet channel').toBe(200);
    const quietId = (await quiet.json()).id as string;

    const badge = page.locator(`[data-qa="channel-mention-badge"][data-channel-id="${channel.id}"]`);
    await badge.waitFor({ state: 'visible', timeout: 5_000 }).catch(async () => {
      await page.reload();
      await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('general');
    });
    await expect(badge).toBeVisible();

    await row.hover();
    await row.locator('[data-qa="notification-menu-open"]').click();
    await expect(page.locator('[data-qa="notify-level-nothing"]')).toHaveAttribute('aria-checked', 'true');
    await page.locator('[data-qa="notify-level-default"]').click();
    await expect(row.locator('[data-qa="sidebar-channel-quiet"]')).toHaveCount(0);

    const loud = await admin.post(`${API}/channels/${channel.id}/messages`, {
      data: { content: `@[Bob Smith](${bobId}) loud probe` },
    });
    expect(loud.status(), 'admin mentions bob again on the default level').toBe(200);
    const loudId = (await loud.json()).id as string;

    await expect.poll(() => frames.some((f) => f.message_id === loudId), { timeout: 15_000 }).toBe(true);
    expect(frames.filter((f) => f.message_id === quietId)).toEqual([]);

    const activity = await (await bob.get(`${API}/workspaces/${workspace.id}/notifications`)).json();
    const forChannel = activity.data.filter(
      (n: { data: { channel_id?: string } }) => n.data?.channel_id === channel.id,
    );
    expect(forChannel.length, 'only the mention on the default level reached Activity').toBe(1);
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
  }
});
