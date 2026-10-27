import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test('Activity opens a direct message on the message', async ({ page }) => {
  const { ctx: bob } = await userContext('bob@dev.local');
  const { ctx: alice, userId: aliceId } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const text = `dm-landing-${Date.now()}`;

  try {
    const opened = await bob.post(`${API}/workspaces/${workspace.id}/conversations`, {
      data: { participant_ids: [aliceId] },
    });
    expect(opened.status()).toBe(200);
    const conversation = await opened.json();
    const posted = await bob.post(`${API}/channels/${conversation.id}/messages`, { data: { content: text } });
    expect(posted.status()).toBe(200);
    const message = await posted.json();

    await expect
      .poll(
        async () => {
          const listed = await (await alice.get(`${API}/workspaces/${workspace.id}/notifications`)).json();
          return listed.data.some((n: { data: { message_id?: string } }) => n.data.message_id === message.id);
        },
        { timeout: 15_000 },
      )
      .toBe(true);

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await page.getByTitle('Notifications').click();
    await page.locator('[data-qa="notification-row"]').filter({ hasText: text }).click();

    await expect(page).toHaveURL(new RegExp(`/c/${conversation.id}/${message.id}$`));
    await expect(page.locator('[data-qa="conversation-title"]')).toBeVisible();
    await expect(page.locator(`[data-message-id="${message.id}"]`)).toBeVisible();
  } finally {
    await bob.dispose();
    await alice.dispose();
  }
});

test('the push route opens a thread reply in its thread', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'reply-landing');
  const text = `reply-landing-${Date.now()}`;

  try {
    const root = await (
      await alice.post(`${API}/channels/${channel.id}/messages`, { data: { content: 'the plan' } })
    ).json();
    const reply = await (
      await alice.post(`${API}/channels/${channel.id}/messages`, {
        data: { content: text, thread_parent_id: root.id },
      })
    ).json();

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}/${reply.id}`);

    const thread = page.locator('[data-qa="thread-panel"]');
    await expect(thread).toBeVisible();
    await expect(thread.locator(`[data-message-id="${reply.id}"]`)).toContainText(text);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(channel.name);
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});

test('search opens a channel not joined as a preview to join', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(admin, workspace.id, 'preview-landing');
  const text = `preview-landing-${Date.now()}`;

  try {
    const posted = await admin.post(`${API}/channels/${channel.id}/messages`, { data: { content: text } });
    expect(posted.status()).toBe(200);
    const message = await posted.json();

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await page.locator('[data-qa="channel-header-search"]').click();
    await page.getByPlaceholder('Search messages...').fill(text);
    await page.locator('[data-qa="search-result"]').filter({ hasText: text }).click();

    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(channel.name);
    await expect(page.locator(`[data-message-id="${message.id}"]`)).toBeVisible();
    await expect(page.locator('[data-qa="channel-preview"]')).toBeVisible();

    await page.locator('[data-qa="channel-preview-join"]').click();
    await expect(page.locator('[data-qa="channel-preview"]')).toHaveCount(0);
    await expect(page.getByRole('navigation').getByText(channel.name)).toBeVisible();
    await expect(page.locator('.ProseMirror[contenteditable="true"]').last()).toBeVisible();
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await alice.dispose();
  }
});
