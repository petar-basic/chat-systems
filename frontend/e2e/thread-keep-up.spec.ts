import { test, expect, type APIRequestContext } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

const REPLIES = 120;

async function post(ctx: APIRequestContext, channelId: string, data: Record<string, unknown>) {
  const res = await ctx.post(`${API}/channels/${channelId}/messages`, { data });
  expect(res.status(), `post ${String(data.content)}`).toBe(200);
  return (await res.json()) as { id: string };
}

test('a long thread reads back to its first reply', async ({ page }) => {
  test.slow();
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const authors = [admin, alice, bob];
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'long-thread');
  const stamp = Date.now();

  try {
    const root = await post(admin, channel.id, { content: `long-thread-${stamp}` });
    for (let i = 0; i < REPLIES; i++) {
      await post(authors[i % authors.length], channel.id, {
        content: `reply-${stamp} #${i}`,
        thread_parent_id: root.id,
      });
    }

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}/${root.id}?thread=1`);

    const panel = page.locator('[data-qa="thread-panel"]');
    await expect(panel.getByText(`reply-${stamp} #${REPLIES - 1}`)).toBeVisible({ timeout: 20_000 });
    await expect(panel.locator('[data-qa="thread-reply-count"]')).toHaveText(`${REPLIES} replies`);

    const earlier = panel.locator('[data-qa="thread-show-earlier"]');
    const replies = panel.locator('[data-qa="thread-reply"]');
    await expect(replies).toHaveCount(50);
    await earlier.click();
    await expect(replies).toHaveCount(100);
    await earlier.click();
    await expect(replies).toHaveCount(REPLIES);
    await expect(earlier).toHaveCount(0);

    await expect(replies.first()).toContainText(`reply-${stamp} #0`);
    await expect(replies.last()).toContainText(`reply-${stamp} #${REPLIES - 1}`);
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    for (const ctx of authors) await ctx.dispose();
  }
});

test('an open thread follows deletes and reactions as they happen', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'live-thread');
  const stamp = Date.now();

  try {
    const root = await post(alice, channel.id, { content: `live-root-${stamp}` });
    const doomed = await post(bob, channel.id, { content: `doomed-${stamp}`, thread_parent_id: root.id });
    const kept = await post(bob, channel.id, { content: `kept-${stamp}`, thread_parent_id: root.id });

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}/${root.id}?thread=1`);
    const panel = page.locator('[data-qa="thread-panel"]');
    await expect(panel.getByText(`doomed-${stamp}`)).toBeVisible({ timeout: 20_000 });
    await expect(panel.locator('[data-qa="thread-reply-count"]')).toHaveText('2 replies');

    expect((await bob.delete(`${API}/messages/${doomed.id}`)).status()).toBe(200);
    await expect(panel.getByText(`doomed-${stamp}`)).toHaveCount(0);
    await expect(panel.locator('[data-qa="thread-reply-count"]')).toHaveText('1 reply');
    const parent = page.locator('[data-qa="message-row"]', { hasText: `live-root-${stamp}` });
    await expect(parent.locator('[data-qa="message-thread-open"]')).toContainText('1 reply');

    const reacted = await bob.post(`${API}/messages/${kept.id}/reactions`, { data: { emoji: '🎉' } });
    expect(reacted.status()).toBe(200);
    await expect(
      panel.locator(`[data-message-id="${kept.id}"] [data-qa="message-reaction"]`, { hasText: '🎉' }),
    ).toBeVisible();
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
    await bob.dispose();
  }
});

test('a deleted parent keeps its thread, and a reply can go to the channel too', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'tombstone');
  const stamp = Date.now();

  try {
    const root = await post(bob, channel.id, { content: `gone-root-${stamp}` });
    await post(alice, channel.id, { content: `answer-${stamp}`, thread_parent_id: root.id });
    expect((await bob.delete(`${API}/messages/${root.id}`)).status()).toBe(200);

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    const tombstone = page.locator(`[data-qa="message-deleted"][data-message-id="${root.id}"]`);
    await expect(tombstone).toBeVisible({ timeout: 20_000 });
    await expect(page.locator('[data-qa="message-list"]').getByText(`answer-${stamp}`)).toHaveCount(0);

    await tombstone.locator('[data-qa="message-thread-open"]').click();
    const panel = page.locator('[data-qa="thread-panel"]');
    await expect(panel.locator('[data-qa="thread-parent-deleted"]')).toBeVisible();
    await expect(panel.getByText(`answer-${stamp}`)).toBeVisible();

    const loud = `loud-${stamp}`;
    await panel.locator('[data-qa="thread-also-to-channel"]').check();
    const editor = panel.locator('.ProseMirror[contenteditable="true"]');
    await editor.click();
    await editor.fill(loud);
    await editor.press('Enter');
    await expect(panel.getByText(loud)).toBeVisible({ timeout: 15_000 });

    const shown = page.locator('[data-qa="message-list"] [data-qa="message-row"]', { hasText: loud });
    await expect(shown).toBeVisible();
    await expect(shown.locator('[data-qa="message-thread-reply-of"]')).toBeVisible();

    await page.reload();
    await expect(
      page.locator('[data-qa="message-list"] [data-qa="message-row"]', { hasText: loud }),
    ).toBeVisible({ timeout: 20_000 });
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
    await bob.dispose();
  }
});
