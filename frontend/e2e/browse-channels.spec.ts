import { test, expect, type APIRequestContext } from '@playwright/test';
import { API, PASSWORD, login } from './helpers';

async function signIn(ctx: APIRequestContext, email: string) {
  const res = await ctx.post(`${API}/auth/login`, { data: { email, password: PASSWORD } });
  expect(res.status(), `login for ${email}`).toBe(200);
}

async function firstWorkspace(ctx: APIRequestContext) {
  return (await (await ctx.get(`${API}/workspaces`)).json()).data[0];
}

async function createChannel(ctx: APIRequestContext, workspaceId: string, name: string, type: string) {
  const res = await ctx.post(`${API}/workspaces/${workspaceId}/channels`, {
    data: { name, channel_type: type },
  });
  expect(res.status(), `create ${type} channel`).toBe(200);
  return (await res.json()).id as string;
}

test('a member can find a public channel they are not in and join it from the browser', async ({
  page,
  playwright,
}) => {
  const admin = await playwright.request.newContext();
  const alice = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  await signIn(alice, 'alice@dev.local');

  const workspace = await firstWorkspace(alice);
  const stamp = Date.now();
  const publicName = `browse-open-${stamp}`;
  const privateName = `browse-secret-${stamp}`;
  const publicId = await createChannel(admin, workspace.id, publicName, 'public');
  const privateId = await createChannel(admin, workspace.id, privateName, 'private');

  try {
    const before = await (await alice.get(`${API}/workspaces/${workspace.id}/channels`)).json();
    expect(
      before.data.some((c: { id: string }) => c.id === publicId),
      'a public channel alice has not joined stays out of her sidebar',
    ).toBe(false);

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}`);

    await page.getByTestId('browse-channels-open').click();
    await expect(page.getByTestId('browse-channels-modal')).toBeVisible();

    await page.getByTestId('browse-channels-search').fill(`browse-`);
    await expect(page.locator(`[data-channel-id="${privateId}"]`)).toHaveCount(0);

    const row = page.locator(`[data-channel-id="${publicId}"]`);
    await expect(row).toBeVisible();
    await expect(row).toContainText('1 member');
    await row.getByTestId('browse-channel-join').click();

    await expect(page.getByTestId('browse-channels-modal')).toBeHidden();
    await expect(page).toHaveURL(new RegExp(`/app/${workspace.id}/${publicId}$`));
    await expect(page.getByRole('navigation').getByText(publicName)).toBeVisible();

    const after = await (await alice.get(`${API}/workspaces/${workspace.id}/channels`)).json();
    expect(after.data.some((c: { id: string }) => c.id === publicId)).toBe(true);
  } finally {
    await admin.delete(`${API}/channels/${publicId}`);
    await admin.delete(`${API}/channels/${privateId}`);
    await admin.dispose();
    await alice.dispose();
  }
});

test('leaving from the browser drops the channel out of the sidebar', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext();
  const alice = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  await signIn(alice, 'alice@dev.local');

  const workspace = await firstWorkspace(alice);
  const name = `browse-leave-${Date.now()}`;
  const channelId = await createChannel(admin, workspace.id, name, 'public');

  try {
    const joined = await alice.post(`${API}/channels/${channelId}/join`, { data: {} });
    expect(joined.status(), 'precondition: alice joins').toBe(200);

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await expect(page.getByRole('navigation').getByText(name)).toBeVisible();

    await page.getByTestId('browse-channels-open').click();
    const row = page.locator(`[data-channel-id="${channelId}"]`);
    await row.getByTestId('browse-channel-leave').click();
    await expect(row.getByTestId('browse-channel-join')).toBeVisible();

    await page.getByRole('button', { name: 'Close' }).click();
    await expect(page.getByRole('navigation').getByText(name)).toHaveCount(0);
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await alice.dispose();
  }
});

test('the API keeps private channels out of browse and refuses joining them', async ({ playwright }) => {
  const admin = await playwright.request.newContext();
  const alice = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  await signIn(alice, 'alice@dev.local');

  const workspace = await firstWorkspace(alice);
  const privateId = await createChannel(admin, workspace.id, `browse-private-${Date.now()}`, 'private');

  try {
    const browsed = await alice.get(`${API}/workspaces/${workspace.id}/channels/browse`);
    expect(browsed.status()).toBe(200);
    const listed = (await browsed.json()).data as Array<{ id: string; channel_type: string }>;
    expect(listed.some((c) => c.id === privateId)).toBe(false);
    expect(listed.every((c) => c.channel_type === 'public')).toBe(true);

    const join = await alice.post(`${API}/channels/${privateId}/join`, { data: {} });
    expect(join.status(), 'private channels stay invite-only').toBe(403);
  } finally {
    await admin.delete(`${API}/channels/${privateId}`);
    await admin.dispose();
    await alice.dispose();
  }
});
