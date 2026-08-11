import { test, expect, type APIRequestContext } from '@playwright/test';
import { API, PASSWORD, login } from './helpers';

async function signIn(ctx: APIRequestContext, email: string) {
  const res = await ctx.post(`${API}/auth/login`, { data: { email, password: PASSWORD } });
  expect(res.status(), `login for ${email}`).toBe(200);
}

async function sharedWorkspace(ctx: APIRequestContext) {
  return (await (await ctx.get(`${API}/workspaces`)).json()).data[0];
}

test('a destructive action shows up in the workspace audit log', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext();
  const bob = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  await signIn(bob, 'bob@dev.local');

  const workspace = await sharedWorkspace(bob);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `audited-${stamp}`, channel_type: 'public' },
  });
  const channelId = (await created.json()).id as string;

  try {
    const archived = await admin.delete(`${API}/channels/${channelId}`);
    expect(archived.status()).toBe(200);

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await page.getByRole('button', { name: workspace.name }).first().click();
    await page.locator('[data-qa="open-audit-log"]').click();

    const panel = page.locator('[data-qa="audit-log-panel"]');
    await expect(panel).toBeVisible();
    await expect(panel.locator('[data-qa="audit-log-action"]').first()).toHaveText('Channel archived');
  } finally {
    await admin.dispose();
    await bob.dispose();
  }
});

test('the audit log stays closed to plain members', async ({ page, playwright }) => {
  const bob = await playwright.request.newContext();
  await signIn(bob, 'bob@dev.local');
  const workspace = await sharedWorkspace(bob);

  try {
    const denied = await bob.get(`${API}/workspaces/${workspace.id}/audit-log`);
    expect(denied.status(), 'a plain member cannot read the trail').toBe(403);

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await page.getByRole('button', { name: workspace.name }).first().click();
    await expect(page.locator('[data-qa="open-audit-log"]')).toHaveCount(0);
  } finally {
    await bob.dispose();
  }
});
