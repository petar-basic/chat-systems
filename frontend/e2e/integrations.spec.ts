import { test, expect, type APIRequestContext, type Page } from '@playwright/test';
import { API, PASSWORD, login } from './helpers';

async function signIn(ctx: APIRequestContext, email: string) {
  const res = await ctx.post(`${API}/auth/login`, { data: { email, password: PASSWORD } });
  expect(res.status(), `login for ${email}`).toBe(200);
}

async function sharedWorkspace(ctx: APIRequestContext) {
  return (await (await ctx.get(`${API}/workspaces`)).json()).data[0];
}

async function openIntegrations(page: Page, workspaceName: string) {
  await page.getByRole('button', { name: workspaceName }).first().click();
  await page.locator('[data-qa="open-integrations"]').click();
  await expect(page.locator('[data-qa="integrations-panel"]')).toBeVisible();
}

test('an admin creates an incoming webhook and a post through it lands in the channel', async ({
  page,
  playwright,
}) => {
  const admin = await playwright.request.newContext();
  const bob = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  await signIn(bob, 'bob@dev.local');

  const workspace = await sharedWorkspace(bob);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `hooks-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channelId}`);
    await openIntegrations(page, workspace.name);

    await page.locator('[data-qa="incoming-hook-name"]').fill(`CI ${stamp}`);
    await page.locator('[data-qa="incoming-hook-channel"]').selectOption(channelId);
    await page.locator('[data-qa="incoming-hook-create"]').click();

    const row = page.locator('[data-qa="hook-row"]').filter({ hasText: `CI ${stamp}` });
    await expect(row.locator('[data-qa="hook-target"]')).toHaveText(`#hooks-${stamp}`);

    const url = await row.locator('[data-qa="hook-secret-value"]').first().innerText();
    expect(url, 'the freshly minted URL is shown once created').toContain('/api/hooks/incoming/');

    const text = `posted-by-webhook-${stamp}`;
    const posted = await admin.post(url, { data: { text } });
    expect(posted.status(), 'the webhook URL accepts a Slack-shaped payload').toBe(200);

    await expect(page.locator('[data-qa="message-list"]').getByText(text)).toBeVisible();
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('rotating a webhook retires the old URL', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext();
  const bob = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  await signIn(bob, 'bob@dev.local');

  const workspace = await sharedWorkspace(bob);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `rotate-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  const hook = await admin.post(`${API}/workspaces/${workspace.id}/hooks`, {
    data: { hook_type: 'incoming_webhook', name: `Rotate ${stamp}`, config: { channel_id: channelId } },
  });
  const hookId = (await hook.json()).id as string;

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channelId}`);
    await openIntegrations(page, workspace.name);

    const row = page.locator(`[data-qa="hook-row"][data-hook-id="${hookId}"]`);
    await row.locator('[data-qa="hook-reveal"]').click();
    const before = await row.locator('[data-qa="hook-secret-value"]').first().innerText();

    await row.locator('[data-qa="hook-rotate"]').click();
    await expect(row.locator('[data-qa="hook-secret-value"]').first()).not.toHaveText(before);
    const after = await row.locator('[data-qa="hook-secret-value"]').first().innerText();

    const retired = await admin.post(before, { data: { text: 'should not arrive' } });
    expect(retired.status(), 'the rotated-away URL stops working').toBe(401);

    const accepted = await admin.post(after, { data: { text: `rotated-${stamp}` } });
    expect(accepted.status()).toBe(200);
  } finally {
    await admin.delete(`${API}/hooks/${hookId}`);
    await admin.delete(`${API}/channels/${channelId}`);
    await admin.dispose();
    await bob.dispose();
  }
});

test('the Integrations menu and the hook API stay closed to non-admins', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext();
  const bob = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  await signIn(bob, 'bob@dev.local');

  const workspace = await sharedWorkspace(bob);

  try {
    const listed = await bob.get(`${API}/workspaces/${workspace.id}/hooks`);
    expect(listed.status(), 'a plain member cannot list integrations').toBe(403);

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await page.getByRole('button', { name: workspace.name }).first().click();
    await expect(page.locator('[data-qa="open-integrations"]')).toHaveCount(0);
  } finally {
    await admin.dispose();
    await bob.dispose();
  }
});
