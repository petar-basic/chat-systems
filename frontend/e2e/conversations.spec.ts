import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace } from './helpers';

test('a group conversation reaches all three people and shows every name', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const { ctx: charlie, userId: charlieId } = await userContext('charlie@dev.local');

  const workspace = await devWorkspace(bob);
  const stamp = Date.now();

  const created = await admin.post(`${API}/workspaces/${workspace.id}/conversations`, {
    data: { participant_ids: [bobId, charlieId] },
  });
  expect(created.status(), 'admin opens a group conversation').toBe(200);
  const conversation = await created.json();
  expect(conversation.kind).toBe('group');

  try {
    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/c/${conversation.id}`);

    await expect(page.locator('[data-qa="conversation-title"]')).toContainText('Charlie Brown');
    await expect(page.locator('[data-qa="conversation-title"]')).toContainText('Admin');

    const text = `group-hello-${stamp}`;
    const posted = await admin.post(`${API}/conversations/${conversation.id}/messages`, {
      data: { content: text },
    });
    expect(posted.status()).toBe(200);

    await expect(page.getByText(text)).toBeVisible();

    const forCharlie = await (await charlie.get(`${API}/conversations/${conversation.id}/messages`)).json();
    expect(
      forCharlie.data.some((m: { content: string }) => m.content === text),
      'the third participant sees it too',
    ).toBe(true);
  } finally {
    await admin.dispose();
    await bob.dispose();
    await charlie.dispose();
  }
});

test('a direct conversation opened twice stays one thread', async () => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(bob);

  try {
    const first = await (
      await admin.post(`${API}/workspaces/${workspace.id}/conversations`, {
        data: { participant_ids: [bobId] },
      })
    ).json();
    const second = await (
      await admin.post(`${API}/workspaces/${workspace.id}/conversations`, {
        data: { participant_ids: [bobId] },
      })
    ).json();

    expect(second.id).toBe(first.id);
    expect(first.kind).toBe('direct');
  } finally {
    await admin.dispose();
    await bob.dispose();
  }
});

test('a message scheduled from the composer waits in the pending list', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');

  const workspace = await devWorkspace(bob);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `later-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channelId}`);

    const editor = page.locator('.ProseMirror[contenteditable="true"]').last();
    await editor.click();
    await editor.pressSequentially(`scheduled-${stamp}`);
    await expect(page.locator('[data-qa="schedule-open"]')).toBeEnabled();

    await page.locator('[data-qa="schedule-open"]').click();
    await expect(page.locator('[data-qa="schedule-menu"]')).toBeVisible();
    await page.locator('[data-qa="schedule-preset"]').first().click();

    await expect(page.locator('[data-qa="schedule-menu"]')).toBeHidden();

    const pending = await (await admin.get(`${API}/workspaces/${workspace.id}/scheduled-messages`)).json();
    const mine = pending.data.filter((m: { content: string }) => m.content === `scheduled-${stamp}`);
    expect(mine.length, 'the message is queued, not sent').toBe(1);
    expect(mine[0].channel_id).toBe(channelId);
    expect(new Date(mine[0].send_at).getTime()).toBeGreaterThan(Date.now());

    const messages = await (await admin.get(`${API}/channels/${channelId}/messages`)).json();
    expect(
      messages.data.some((m: { content: string }) => m.content === `scheduled-${stamp}`),
      'nothing shows up in the channel yet',
    ).toBe(false);

    const canceled = await admin.delete(`${API}/scheduled-messages/${mine[0].id}`);
    expect(canceled.status()).toBe(200);
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('a custom date and time can be picked for a scheduled message', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');

  const workspace = await devWorkspace(bob);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `custom-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  const target = new Date(Date.now() + 3 * 24 * 60 * 60 * 1000);
  target.setSeconds(0, 0);
  const pad = (n: number) => String(n).padStart(2, '0');
  const inputValue = `${target.getFullYear()}-${pad(target.getMonth() + 1)}-${pad(target.getDate())}T${pad(
    target.getHours(),
  )}:${pad(target.getMinutes())}`;

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channelId}`);

    const editor = page.locator('.ProseMirror[contenteditable="true"]').last();
    await editor.click();
    await editor.pressSequentially(`custom-${stamp}`);

    await page.locator('[data-qa="schedule-open"]').click();
    await page.locator('[data-qa="schedule-custom-input"]').fill(inputValue);
    await page.locator('[data-qa="schedule-custom-submit"]').click();

    await expect(page.locator('[data-qa="schedule-menu"]')).toBeHidden();

    const pending = await (await admin.get(`${API}/workspaces/${workspace.id}/scheduled-messages`)).json();
    const mine = pending.data.find((m: { content: string }) => m.content === `custom-${stamp}`);
    expect(mine, 'the custom time was queued').toBeTruthy();
    expect(new Date(mine.send_at).getTime()).toBe(target.getTime());

    await admin.delete(`${API}/scheduled-messages/${mine.id}`);
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('a time in the past is refused before it reaches the server', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');

  const workspace = await devWorkspace(bob);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `past-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channelId}`);

    const editor = page.locator('.ProseMirror[contenteditable="true"]').last();
    await editor.click();
    await editor.pressSequentially(`past-${stamp}`);

    await page.locator('[data-qa="schedule-open"]').click();
    await page.locator('[data-qa="schedule-custom-input"]').fill('2020-01-01T09:00');
    await page.locator('[data-qa="schedule-custom-submit"]').click();

    await expect(page.locator('[data-qa="schedule-error"]')).toContainText('already passed');
    await expect(page.locator('[data-qa="schedule-menu"]')).toBeVisible();

    const pending = await (await admin.get(`${API}/workspaces/${workspace.id}/scheduled-messages`)).json();
    expect(pending.data.some((m: { content: string }) => m.content === `past-${stamp}`)).toBe(false);
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('the scheduled panel lists a queued message and cancels it', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');

  const workspace = await devWorkspace(bob);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `panel-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  const queued = await admin.post(`${API}/workspaces/${workspace.id}/scheduled-messages`, {
    data: {
      channel_id: channelId,
      content: `panel-queued-${stamp}`,
      send_at: new Date(Date.now() + 2 * 60 * 60 * 1000).toISOString(),
    },
  });
  const scheduledId = (await queued.json()).id as string;

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channelId}`);

    await page.getByRole('button', { name: workspace.name }).first().click();
    await page.locator('[data-qa="open-scheduled"]').click();
    await expect(page.locator('[data-qa="scheduled-panel"]')).toBeVisible();

    const row = page.locator(`[data-qa="scheduled-row"][data-scheduled-id="${scheduledId}"]`);
    await expect(row.locator('[data-qa="scheduled-content"]')).toHaveText(`panel-queued-${stamp}`);
    await expect(row.locator('[data-qa="scheduled-target"]')).toContainText(`#panel-${stamp}`);

    await row.locator('[data-qa="scheduled-cancel"]').click();
    await expect(row).toHaveCount(0);

    const pending = await (await admin.get(`${API}/workspaces/${workspace.id}/scheduled-messages`)).json();
    expect(pending.data.some((m: { id: string }) => m.id === scheduledId)).toBe(false);
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('a queued message can be moved to a new time from the panel', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');

  const workspace = await devWorkspace(bob);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `move-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  const queued = await admin.post(`${API}/workspaces/${workspace.id}/scheduled-messages`, {
    data: {
      channel_id: channelId,
      content: `move-me-${stamp}`,
      send_at: new Date(Date.now() + 60 * 60 * 1000).toISOString(),
    },
  });
  const scheduledId = (await queued.json()).id as string;

  const target = new Date(Date.now() + 5 * 24 * 60 * 60 * 1000);
  target.setSeconds(0, 0);
  const pad = (n: number) => String(n).padStart(2, '0');
  const inputValue = `${target.getFullYear()}-${pad(target.getMonth() + 1)}-${pad(target.getDate())}T${pad(
    target.getHours(),
  )}:${pad(target.getMinutes())}`;

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channelId}`);
    await page.getByRole('button', { name: workspace.name }).first().click();
    await page.locator('[data-qa="open-scheduled"]').click();

    const row = page.locator(`[data-qa="scheduled-row"][data-scheduled-id="${scheduledId}"]`);
    await row.locator('[data-qa="scheduled-reschedule"]').click();
    await row.locator('[data-qa="scheduled-reschedule-input"]').fill('2020-01-01T09:00');
    await row.locator('[data-qa="scheduled-reschedule-submit"]').click();
    await expect(row.locator('[data-qa="scheduled-reschedule-error"]')).toContainText('already passed');

    await row.locator('[data-qa="scheduled-reschedule-input"]').fill(inputValue);
    await row.locator('[data-qa="scheduled-reschedule-submit"]').click();

    await expect(row.locator('[data-qa="scheduled-reschedule-input"]')).toHaveCount(0);

    const pending = await (await admin.get(`${API}/workspaces/${workspace.id}/scheduled-messages`)).json();
    const moved = pending.data.find((m: { id: string }) => m.id === scheduledId);
    expect(new Date(moved.send_at).getTime()).toBe(target.getTime());

    await admin.delete(`${API}/scheduled-messages/${scheduledId}`);
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});
