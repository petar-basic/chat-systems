import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace, createChannel } from './helpers';

test('an archived channel takes no posts until an admin unarchives it', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'archive');

  try {
    const added = await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
    expect(added.status()).toBe(200);

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.getByTestId('channel-header-name')).toHaveText(channel.name);

    await page.getByTestId('channel-actions-more').click();
    await page.getByTestId('channel-details-open').click();
    await page.getByTestId('channel-about-archive').click();
    await page.getByTestId('channel-about-archive-dialog-confirm').click();

    const bar = page.getByTestId('channel-archived');
    await expect(bar).toContainText(`#${channel.name} is archived`);
    await expect(page.locator('[data-message-pane] .ProseMirror[contenteditable="true"]')).toHaveCount(0);
    await expect(page.getByText('archived this channel')).toBeVisible();

    const refused = await bob.post(`${API}/channels/${channel.id}/messages`, {
      data: { content: 'still here?' },
    });
    expect(refused.status(), 'a member cannot post to an archived channel').toBe(403);
    const notHis = await bob.post(`${API}/channels/${channel.id}/unarchive`);
    expect(notHis.status(), 'and cannot unarchive it either').toBe(403);

    await page.getByTestId('channel-unarchive').click();
    await expect(bar).toBeHidden();
    await expect(page.locator('[data-message-pane] .ProseMirror[contenteditable="true"]')).toBeVisible();

    const back = await bob.post(`${API}/channels/${channel.id}/messages`, {
      data: { content: 'back again' },
    });
    expect(back.status(), 'posting works once it is unarchived').toBe(200);
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('a member finds an archived private channel in Browse and unarchives it', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `hr-${Date.now()}`, channel_type: 'private' },
  });
  expect(created.status()).toBe(200);
  const channel = (await created.json()) as { id: string; name: string };

  try {
    expect((await admin.delete(`${API}/channels/${channel.id}`)).status()).toBe(200);

    const outsider = await bob.get(`${API}/workspaces/${workspace.id}/channels/browse`);
    const listed = ((await outsider.json()) as { data: { id: string }[] }).data.map((c) => c.id);
    expect(listed, 'an outsider never sees it').not.toContain(channel.id);

    await login(page, 'admin@dev.local');
    await page.getByTestId('browse-channels-open').click();
    await page.getByTestId('browse-channels-search').fill(channel.name);
    const row = page.locator(`[data-qa="browse-channel-row"][data-channel-id="${channel.id}"]`);
    await expect(row).toContainText('Archived private channel');
    await row.getByTestId('browse-channel-open').click();

    const bar = page.getByTestId('channel-archived');
    await expect(bar).toContainText(`#${channel.name} is archived`);
    await page.getByTestId('channel-unarchive').click();
    await expect(bar).toBeHidden();
    await expect(page.locator('[data-message-pane] .ProseMirror[contenteditable="true"]')).toBeVisible();
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
  }
});
