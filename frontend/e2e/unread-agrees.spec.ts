import { test, expect, type APIRequestContext, type Locator, type Page } from '@playwright/test';
import { API, login, userContext, devWorkspace, createChannel } from './helpers';

interface Activity {
  is_read: boolean;
  data: { channel_id?: string; thread_parent_id?: string | null };
}

async function unreadActivity(
  ctx: APIRequestContext,
  workspaceId: string,
  about: (n: Activity) => boolean,
): Promise<number> {
  const listed = await (await ctx.get(`${API}/workspaces/${workspaceId}/notifications`)).json();
  return (listed.data as Activity[]).filter((n) => !n.is_read && about(n)).length;
}

function liveIn(page: Page, workspaceId: string, channelId: string): Promise<void> {
  return new Promise((resolve) => {
    let joined = false;
    let synced = false;
    page.on('websocket', (socket) => {
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

async function post(ctx: APIRequestContext, channelId: string, data: Record<string, unknown>) {
  const res = await ctx.post(`${API}/channels/${channelId}/messages`, { data });
  expect(res.status(), `post ${String(data.content)}`).toBe(200);
  return (await res.json()) as { id: string };
}

test('a mention in the channel on screen leaves no badge and no Activity row', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(bob);
  const channel = await createChannel(admin, workspace.id, 'on-screen');

  try {
    await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
    await login(page, 'bob@dev.local');
    const live = liveIn(page, workspace.id, channel.id);
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(channel.name);
    await live;

    const mention = await post(admin, channel.id, { content: `@[Bob Smith](${bobId}) look here` });
    await expect(page.locator(`[data-qa="message-list"] [data-message-id="${mention.id}"]`)).toBeVisible({
      timeout: 15_000,
    });

    await expect
      .poll(() => unreadActivity(bob, workspace.id, (n) => n.data.channel_id === channel.id), {
        timeout: 15_000,
      })
      .toBe(0);
    const row = page.locator('[data-qa="sidebar-channel-row"]', { hasText: channel.name });
    await expect(row.locator('[data-qa="channel-mention-badge"]')).toHaveCount(0);
    await expect(row.locator('[data-qa="channel-unread-badge"]')).toHaveCount(0);

    await page
      .getByRole('button', { name: /^general$/ })
      .first()
      .click();
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('general');
    await expect(
      row.locator('[data-qa="channel-mention-badge"]'),
      'leaving does not bring it back',
    ).toHaveCount(0);
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('a reply clears with its thread, not with its channel', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(bob);
  const channel = await createChannel(admin, workspace.id, 'thread-rule');

  try {
    await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
    const root = await post(bob, channel.id, { content: 'what do we ship?' });
    await post(admin, channel.id, {
      content: `@[Bob Smith](${bobId}) the thread answer`,
      thread_parent_id: root.id,
    });
    const inThread = (n: Activity) => n.data.thread_parent_id === root.id;
    await expect.poll(() => unreadActivity(bob, workspace.id, inThread), { timeout: 15_000 }).toBe(1);

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(channel.name);
    const row = page.locator('[data-qa="sidebar-channel-row"]', { hasText: channel.name });
    await expect(
      row.locator('[data-qa="channel-mention-badge"]'),
      'a thread mention is not a channel one',
    ).toHaveCount(0);
    expect(await unreadActivity(bob, workspace.id, inThread), 'reading the channel leaves the thread').toBe(
      1,
    );

    await page.goto(`/app/${workspace.id}/${channel.id}/${root.id}?thread=1`);
    await expect(page.locator('[data-qa="thread-panel"]')).toBeVisible({ timeout: 20_000 });
    await expect.poll(() => unreadActivity(bob, workspace.id, inThread), { timeout: 15_000 }).toBe(0);
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('a channel with unread messages opens on the first of them', async ({ page }) => {
  test.slow();
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(bob);
  const channel = await createChannel(admin, workspace.id, 'first-unread');
  const UNREAD = 40;

  try {
    await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
    await post(admin, channel.id, { content: 'already read' });
    await bob.get(`${API}/channels/${channel.id}/messages?mark_read=true`);
    const unread: string[] = [];
    for (let i = 0; i < UNREAD; i++) {
      const author = i % 2 === 0 ? admin : alice;
      unread.push((await post(author, channel.id, { content: `unread #${i}` })).id);
    }

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    const list = page.locator('[data-qa="message-list"]');
    const divider = list.locator('[data-qa="unread-divider"]');
    await expect(divider, 'a cold load draws the line').toBeInViewport({ timeout: 15_000 });
    await expect(list.locator(`[data-message-id="${unread[0]}"]`)).toBeInViewport();
    await expect(list.locator(`[data-message-id="${unread[UNREAD - 1]}"]`)).not.toBeInViewport();

    await list.evaluate((el) => {
      el.scrollTop += el.clientHeight;
    });
    await expect(divider).not.toBeInViewport();
    const jump = page.locator('[data-qa="jump-to-first-unread"]');
    await expect(jump).toBeVisible();
    await jump.click();
    await expect(divider).toBeInViewport();
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await alice.dispose();
    await bob.dispose();
  }
});

test('after Mark all read the channel opens on one page, with the line above what came since', async ({
  page,
}) => {
  test.slow();
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice, userId: aliceId } = await userContext('alice@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(bob);
  const channel = await createChannel(admin, workspace.id, 'read-all');

  try {
    for (const userId of [aliceId, bobId]) {
      await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: userId } });
    }
    let last = { id: '' };
    for (let i = 0; i < 60; i++) {
      last = await post(i % 2 ? alice : admin, channel.id, { content: `read-all #${i}` });
    }
    await post(admin, channel.id, { content: 'only in the thread', thread_parent_id: last.id });

    const read = await bob.post(`${API}/workspaces/${workspace.id}/read`);
    expect(read.status()).toBe(200);
    await post(alice, channel.id, { content: 'since the read' });

    const older: string[] = [];
    page.on('request', (request) => {
      const url = request.url();
      if (url.includes(`/channels/${channel.id}/messages`) && url.includes('cursor=')) older.push(url);
    });
    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);

    const list = page.locator('[data-qa="message-list"]');
    await expect(list.getByText('since the read')).toBeVisible();
    const divider = list.locator('[data-qa="unread-divider"]');
    await expect(divider).toHaveCount(1);
    const top = async (locator: Locator) => (await locator.boundingBox())?.y ?? NaN;
    const line = await top(divider);
    expect(line).toBeGreaterThan(await top(list.getByText('read-all #59')));
    expect(line).toBeLessThan(await top(list.getByText('since the read')));
    await page.waitForTimeout(1_000);
    expect(older, 'the boundary is on the first page, so nothing older loads').toEqual([]);
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    await Promise.all([admin, alice, bob].map((ctx) => ctx.dispose()));
  }
});
