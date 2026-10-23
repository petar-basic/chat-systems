import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test('a reply to my thread shows in Threads, and I answer it from there', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'threads-view');
  const stamp = Date.now();
  const rootText = `threads-root-${stamp}`;
  const bobText = `threads-bob-${stamp}`;
  const aliceText = `threads-alice-${stamp}`;

  try {
    const root = await (
      await alice.post(`${API}/channels/${channel.id}/messages`, { data: { content: rootText } })
    ).json();
    expect((await bob.post(`${API}/channels/${channel.id}/join`)).status(), 'bob joins').toBe(200);
    const reply = await bob.post(`${API}/channels/${channel.id}/messages`, {
      data: { content: bobText, thread_parent_id: root.id },
    });
    expect(reply.status(), 'bob replies in the thread').toBe(200);

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="threads-count"]')).toBeVisible({ timeout: 20_000 });

    await page.locator('[data-qa="threads-open"]').click();
    await expect(page).toHaveURL(new RegExp(`/app/${workspace.id}/threads$`));

    const card = page.locator('[data-qa="thread-card"]', { hasText: rootText });
    await expect(card).toBeVisible({ timeout: 20_000 });
    await expect(card.locator('[data-qa="thread-new"]')).toHaveText('1 new reply');
    await expect(card.locator('[data-qa="thread-reply-new"]', { hasText: bobText })).toBeVisible();

    const editor = card.locator('.ProseMirror[contenteditable="true"]');
    await editor.click();
    await editor.fill(aliceText);
    await editor.press('Enter');
    await expect(card.getByText(aliceText)).toBeVisible({ timeout: 15_000 });

    const state = await (await alice.get(`${API}/workspaces/${workspace.id}/threads`)).json();
    const followed = state.data.find((t: { root: { id: string } }) => t.root.id === root.id);
    expect(followed?.unread_count, 'reading and replying cleared the thread').toBe(0);
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
    await bob.dispose();
  }
});
