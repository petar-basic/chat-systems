import { test, expect, type APIRequestContext, type Page } from '@playwright/test';
import { API, PASSWORD, login } from './helpers';

async function openChannel(page: Page, workspaceId: string, channelId: string, name: string) {
  await page.goto(`/app/${workspaceId}/${channelId}`);
  await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(name);
  await expect(page).toHaveURL(new RegExp(`/app/${workspaceId}/${channelId}$`));
}

async function signIn(ctx: APIRequestContext, email: string) {
  const res = await ctx.post(`${API}/auth/login`, { data: { email, password: PASSWORD } });
  expect(res.status(), `login for ${email}`).toBe(200);
  return (await res.json()).user.id as string;
}

async function sharedWorkspace(ctx: APIRequestContext) {
  return (await (await ctx.get(`${API}/workspaces`)).json()).data[0];
}

async function createChannel(ctx: APIRequestContext, workspaceId: string, name: string, type = 'public') {
  const res = await ctx.post(`${API}/workspaces/${workspaceId}/channels`, {
    data: { name, channel_type: type },
  });
  expect(res.status(), `create ${type} channel`).toBe(200);
  return (await res.json()).id as string;
}

test('a channel admin can rename their channel while a plain member cannot', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext();
  const bob = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  const bobId = await signIn(bob, 'bob@dev.local');

  const workspace = await sharedWorkspace(bob);
  const stamp = Date.now();
  const channelName = `perm-${stamp}`;
  const channelId = await createChannel(admin, workspace.id, channelName);

  try {
    const added = await admin.post(`${API}/channels/${channelId}/members`, {
      data: { user_id: bobId },
    });
    expect(added.status(), 'admin adds bob to the channel').toBe(200);

    await login(page, 'bob@dev.local');
    await openChannel(page, workspace.id, channelId, channelName);
    await expect(page.locator('[data-qa="channel-settings-open"]')).toHaveCount(0);

    const denied = await bob.patch(`${API}/channels/${channelId}`, { data: { name: `nope-${stamp}` } });
    expect(denied.status(), 'a plain channel member cannot rename').toBe(403);

    const promoted = await admin.patch(`${API}/channels/${channelId}/members/${bobId}/role`, {
      data: { role: 'admin' },
    });
    expect(promoted.status(), 'admin promotes bob to channel admin').toBe(200);

    await page.reload();
    await expect(page.locator('[data-qa="channel-settings-open"]')).toBeVisible();

    await page.locator('[data-qa="channel-settings-open"]').click();
    await page.locator('[data-qa="channel-settings-name"]').fill(`perm-${stamp}-renamed`);
    await page.locator('[data-qa="channel-settings-topic"]').fill('owned by bob');
    await page.locator('[data-qa="channel-settings-save"]').click();

    await expect(page.locator('[data-qa="channel-settings-modal"]')).toBeHidden();
    await expect(page.getByRole('navigation').getByText(`perm-${stamp}-renamed`)).toBeVisible();

    const fetched = await (await admin.get(`${API}/channels/${channelId}`)).json();
    expect(fetched.name).toBe(`perm-${stamp}-renamed`);
    expect(fetched.topic).toBe('owned by bob');
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('channel roles can be handed out and taken back from the members panel', async ({
  page,
  playwright,
}) => {
  const admin = await playwright.request.newContext();
  const bob = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  const bobId = await signIn(bob, 'bob@dev.local');

  const workspace = await sharedWorkspace(bob);
  const channelName = `roles-${Date.now()}`;
  const channelId = await createChannel(admin, workspace.id, channelName);

  try {
    await admin.post(`${API}/channels/${channelId}/members`, { data: { user_id: bobId } });

    await login(page, 'admin@dev.local');
    await openChannel(page, workspace.id, channelId, channelName);
    await page.getByRole('button', { name: 'Channel members' }).click();
    await expect(page.locator('[data-qa="channel-members-panel"]')).toBeVisible();

    const bobRow = page.locator(`[data-qa="channel-member-row"][data-user-id="${bobId}"]`);
    await expect(bobRow.locator('[data-qa="channel-member-role"]')).toHaveText('Member');

    await bobRow.locator('[data-qa="channel-member-promote"]').click();
    await expect(bobRow.locator('[data-qa="channel-member-role"]')).toHaveText('Channel admin');

    const afterPromote = await (await admin.get(`${API}/channels/${channelId}/members`)).json();
    expect(afterPromote.data.find((m: { user_id: string }) => m.user_id === bobId).role).toBe('admin');

    await bobRow.locator('[data-qa="channel-member-demote"]').click();
    await expect(bobRow.locator('[data-qa="channel-member-role"]')).toHaveText('Member');
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('a plain member can add people to a public channel but not to a private one', async ({ playwright }) => {
  const admin = await playwright.request.newContext();
  const bob = await playwright.request.newContext();
  const charlie = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  await signIn(bob, 'bob@dev.local');
  const charlieId = await signIn(charlie, 'charlie@dev.local');

  const workspace = await sharedWorkspace(bob);
  const stamp = Date.now();
  const publicId = await createChannel(admin, workspace.id, `open-${stamp}`);
  const privateId = await createChannel(admin, workspace.id, `closed-${stamp}`, 'private');

  try {
    const addedToPublic = await bob.post(`${API}/channels/${publicId}/members`, {
      data: { user_id: charlieId },
    });
    expect(addedToPublic.status(), 'plain member adds to a public channel').toBe(200);

    const addedToPrivate = await bob.post(`${API}/channels/${privateId}/members`, {
      data: { user_id: charlieId },
    });
    expect(addedToPrivate.status(), 'someone outside a private channel cannot add to it').toBe(403);
  } finally {
    await admin.delete(`${API}/channels/${publicId}`);
    await admin.delete(`${API}/channels/${privateId}`);
    await admin.dispose();
    await bob.dispose();
    await charlie.dispose();
  }
});
