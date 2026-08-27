import { test, expect, type Page } from '@playwright/test';
import { API, login, send, userContext, devWorkspace } from './helpers';

test.setTimeout(60_000);

async function openWorkspace(page: Page, email: string, workspaceId: string, channelId: string) {
  await login(page, email);
  await page.goto(`/app/${workspaceId}/${channelId}`);
  // An empty channel renders its empty state instead of the list, so the header
  // is what says the channel is open.
  await expect(page.locator('[data-qa="channel-header-name"]')).toBeVisible({ timeout: 20_000 });
}

test('a saved message shows up in the saved panel and can be taken out again', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const channels = (await (await admin.get(`${API}/workspaces/${workspace.id}/channels`)).json()).data;
  const channel = channels.find((c: { name: string }) => c.name === 'general');

  try {
    await openWorkspace(page, 'admin@dev.local', workspace.id, channel.id);

    const text = `save-me-${Date.now()}`;
    await send(page, text);
    const row = page.locator('[data-qa="message-row"]', { hasText: text }).last();
    await expect(row).toBeVisible();
    await row.hover();
    await row.locator('[data-qa="message-action-save"]').click();

    await page.locator('[data-qa="open-you-menu"]').click();
    await page.locator('[data-qa="open-saved"]').click();

    const panel = page.locator('[data-qa="saved-panel"]');
    await expect(panel).toBeVisible();
    const saved = panel.locator('[data-qa="saved-row"]', { hasText: text });
    await expect(saved).toBeVisible();

    await saved.locator('[data-qa="saved-remove"]').click();
    await expect(panel.locator('[data-qa="saved-row"]', { hasText: text })).toHaveCount(0);
  } finally {
    await admin.dispose();
  }
});

test('a channel bookmark is added from the bar and everybody in the channel sees it', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `bookmarks-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  try {
    await openWorkspace(page, 'admin@dev.local', workspace.id, channelId);

    await page.locator('[data-qa="channel-bookmark-add"]').click();
    await page.locator('[data-qa="channel-bookmark-label"]').fill(`Runbook ${stamp}`);
    await page.locator('[data-qa="channel-bookmark-url"]').fill('https://example.test/runbook');
    await page.locator('[data-qa="channel-bookmark-save"]').click();

    const bookmark = page.locator('[data-qa="channel-bookmark"]', { hasText: `Runbook ${stamp}` });
    await expect(bookmark).toBeVisible();

    const forBob = await (await bob.get(`${API}/channels/${channelId}/bookmarks`)).json();
    expect(
      forBob.data.some((b: { label: string }) => b.label === `Runbook ${stamp}`),
      'a bookmark belongs to the channel, not to whoever pinned it',
    ).toBe(true);

    await bookmark.hover();
    await bookmark.locator('[data-qa="channel-bookmark-remove"]').click();
    await expect(page.locator('[data-qa="channel-bookmark"]', { hasText: `Runbook ${stamp}` })).toHaveCount(
      0,
    );
  } finally {
    await admin.dispose();
    await bob.dispose();
  }
});

test('a reply inside a direct message stays in its thread', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(bob);
  const stamp = Date.now();

  const conversation = await (
    await admin.post(`${API}/workspaces/${workspace.id}/conversations`, {
      data: { participant_ids: [bobId] },
    })
  ).json();

  const parentText = `dm-thread-parent-${stamp}`;
  await admin.post(`${API}/conversations/${conversation.id}/messages`, {
    data: { content: parentText },
  });

  try {
    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/c/${conversation.id}`);

    const parent = page.locator('[data-qa="conversation-message"]', { hasText: parentText }).last();
    await expect(parent).toBeVisible({ timeout: 20_000 });
    await parent.hover();
    await parent.locator('[data-qa="dm-action-thread"]').click();

    const panel = page.locator('[data-qa="dm-thread-panel"]');
    await expect(panel).toBeVisible();

    const replyText = `dm-thread-reply-${stamp}`;
    const editor = panel.locator('.ProseMirror[contenteditable="true"]');
    await editor.click();
    await editor.fill(replyText);
    await editor.press('Enter');

    await expect(panel.locator('[data-qa="dm-thread-reply"]', { hasText: replyText })).toBeVisible();

    const feed = await (await bob.get(`${API}/conversations/${conversation.id}/messages`)).json();
    expect(
      feed.data.some((m: { content: string }) => m.content === replyText),
      'a threaded reply does not clutter the main conversation',
    ).toBe(false);
    const parentRow = feed.data.find((m: { content: string }) => m.content === parentText);
    expect(parentRow.reply_count).toBe(1);
  } finally {
    await admin.dispose();
    await bob.dispose();
  }
});

test('a message forwarded to another channel arrives as a quote', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const channels = (await (await admin.get(`${API}/workspaces/${workspace.id}/channels`)).json()).data;
  const general = channels.find((c: { name: string }) => c.name === 'general');

  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `forwarded-${stamp}`, channel_type: 'public' },
  });
  const targetId = (await created.json()).id as string;

  try {
    await openWorkspace(page, 'admin@dev.local', workspace.id, general.id);

    const text = `forward-me-${stamp}`;
    await send(page, text);
    const row = page.locator('[data-qa="message-row"]', { hasText: text }).last();
    await expect(row).toBeVisible();
    await row.hover();
    await row.locator('[data-qa="message-action-more"]').click();
    await row.locator('[data-qa="message-action-forward"]').click();

    const modal = page.locator('[data-qa="forward-modal"]');
    await expect(modal).toBeVisible();
    await modal.locator('[data-qa="forward-comment"]').fill('worth a look');
    await modal.locator('[data-qa="forward-search"]').fill(`forwarded-${stamp}`);
    await modal.locator('[data-qa="forward-target"]').first().click();
    await expect(modal).toBeHidden();

    const messages = await (await admin.get(`${API}/channels/${targetId}/messages`)).json();
    const forwarded = messages.data.find((m: { content: string }) => m.content.includes(text));
    expect(forwarded, 'the forward landed in the channel that was picked').toBeTruthy();
    expect(forwarded.content).toContain('worth a look');
    expect(forwarded.content).toContain(`> ${text}`);
  } finally {
    await admin.dispose();
  }
});

test('a status set in the profile shows next to your name', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const channels = (await (await admin.get(`${API}/workspaces/${workspace.id}/channels`)).json()).data;
  const channel = channels.find((c: { name: string }) => c.name === 'general');

  try {
    await openWorkspace(page, 'admin@dev.local', workspace.id, channel.id);

    await page.locator('[data-qa="sidebar-profile-avatar"]').click();
    const editor = page.locator('[data-qa="status-editor"]');
    await expect(editor).toBeVisible();
    await editor.locator('[data-qa="status-emoji"]').fill('🍕');
    await editor.locator('[data-qa="status-text"]').fill('at lunch');
    await editor.locator('[data-qa="status-save"]').click();

    await expect(editor.locator('[data-qa="status-clear"]')).toBeVisible();
    const me = await (await admin.get(`${API}/users/me`)).json();
    expect(me.status_text).toBe('at lunch');
    expect(me.status_emoji).toBe('🍕');

    await editor.locator('[data-qa="status-clear"]').click();
    await expect.poll(async () => (await (await admin.get(`${API}/users/me`)).json()).status_text).toBeNull();
  } finally {
    await admin.dispose();
  }
});

test('a slash reminder turns up in the reminders panel and can be cancelled', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const channels = (await (await admin.get(`${API}/workspaces/${workspace.id}/channels`)).json()).data;
  const channel = channels.find((c: { name: string }) => c.name === 'general');
  const stamp = Date.now();

  try {
    await openWorkspace(page, 'admin@dev.local', workspace.id, channel.id);

    await send(page, `/remind me in 2h to check ${stamp}`);
    await expect(page.locator('[data-qa="command-response"]')).toContainText('I will remind you');

    await page.locator('[data-qa="open-you-menu"]').click();
    await page.locator('[data-qa="open-reminders"]').click();

    const panel = page.locator('[data-qa="reminders-panel"]');
    const reminder = panel.locator('[data-qa="reminder-row"]', { hasText: `check ${stamp}` });
    await expect(reminder).toBeVisible();

    await reminder.locator('[data-qa="reminder-cancel"]').click();
    await expect(panel.locator('[data-qa="reminder-row"]', { hasText: `check ${stamp}` })).toHaveCount(0);
  } finally {
    await admin.dispose();
  }
});
