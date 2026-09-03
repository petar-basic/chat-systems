import { test, expect, type Page } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

const stamp = () => `${Date.now()}`;

async function replyInPanel(panel: ReturnType<Page['locator']>, text: string) {
  const editor = panel.locator('.ProseMirror[contenteditable="true"]');
  await editor.click();
  await editor.fill(text);
  await editor.press('Enter');
  await expect(panel.getByText(text)).toBeVisible({ timeout: 15_000 });
}

// The count the sender sees is the one that used to read 2 after a single
// reply: the send and the websocket echo of it each added one.
test('the sender sees one reply on their own channel message', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'thread-count');
  const parentText = `thread-parent-${stamp()}`;
  const posted = await alice.post(`${API}/channels/${channel.id}/messages`, {
    data: { content: parentText },
  });
  expect(posted.status(), 'seed the parent message').toBe(200);

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);

    const parent = page.locator('[data-qa="message-row"]', { hasText: parentText });
    await expect(parent).toBeVisible({ timeout: 20_000 });
    await parent.hover();
    await parent.locator('[data-qa="message-action-thread"]').click();

    const panel = page.locator('[data-qa="thread-panel"]');
    await expect(panel).toBeVisible();
    await replyInPanel(panel, `thread-reply-${stamp()}`);

    const count = parent.locator('[data-qa="message-thread-open"]');
    await expect(count).toContainText('1 reply');
    // The echo of the reply arrives after the send resolves, and it is that
    // second arrival that used to bump the count again.
    await page.waitForTimeout(1500);
    await expect(count).toContainText('1 reply');
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});

test('the sender sees one reply on their own direct message', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(alice);
  const conversation = await (
    await alice.post(`${API}/workspaces/${workspace.id}/conversations`, {
      data: { participant_ids: [bobId] },
    })
  ).json();
  const parentText = `dm-count-parent-${stamp()}`;
  const posted = await alice.post(`${API}/channels/${conversation.id}/messages`, {
    data: { content: parentText },
  });
  expect(posted.status(), 'seed the parent message').toBe(200);

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/c/${conversation.id}`);

    const parent = page.locator('[data-qa="message-row"]', { hasText: parentText });
    await expect(parent).toBeVisible({ timeout: 20_000 });
    await parent.hover();
    await parent.locator('[data-qa="message-action-thread"]').click();

    const panel = page.locator('[data-qa="thread-panel"]');
    await expect(panel).toBeVisible();
    await replyInPanel(panel, `dm-thread-reply-${stamp()}`);

    const count = parent.locator('[data-qa="message-thread-open"]');
    await expect(count).toContainText('1 reply');
    await page.waitForTimeout(1500);
    await expect(count).toContainText('1 reply');
  } finally {
    await alice.dispose();
    await bob.dispose();
  }
});

// The thread panel is the narrowest composer in the app, and the formatting
// buttons used to push the send button straight out of it.
test('the thread composer keeps its controls inside the panel', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'thread-layout');
  const parentText = `thread-layout-${stamp()}`;
  await alice.post(`${API}/channels/${channel.id}/messages`, { data: { content: parentText } });

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);

    const parent = page.locator('[data-qa="message-row"]', { hasText: parentText });
    await expect(parent).toBeVisible({ timeout: 20_000 });
    await parent.hover();
    await parent.locator('[data-qa="message-action-thread"]').click();

    const panel = page.locator('[data-qa="thread-panel"]');
    await expect(panel).toBeVisible();

    const send = panel.locator('button[title="Send (Enter)"]');
    await expect(send).toBeVisible();

    const panelBox = await panel.boundingBox();
    const sendBox = await send.boundingBox();
    expect(panelBox && sendBox, 'both boxes measurable').toBeTruthy();
    expect(
      sendBox!.x + sendBox!.width,
      'the send button must end inside the panel it lives in',
    ).toBeLessThanOrEqual(panelBox!.x + panelBox!.width);
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
