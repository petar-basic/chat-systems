import { test, expect } from '@playwright/test';
import { API, userContext } from './helpers';

test.use({
  launchOptions: { args: ['--host-resolver-rules=MAP oidc 127.0.0.1'] },
});

test('signing in with SSO from an invite link lands in the invited workspace', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const created = await admin.post(`${API}/workspaces`, { data: { name: `SSO Invite ${Date.now()}` } });
  expect(created.status()).toBe(200);
  const workspace = (await created.json()) as { id: string };

  try {
    const invited = await admin.post(`${API}/workspaces/${workspace.id}/invites`, {
      data: { email: 'sso@dev.local', role: 'member' },
    });
    expect(invited.status()).toBe(200);
    const { invite_url: link } = (await invited.json()) as { invite_url: string };

    await page.goto(new URL(link).pathname);
    await page.getByTestId('invite-sso-start').click();
    await page.waitForURL(new RegExp(`/app/${workspace.id}`), { timeout: 20_000 });

    const mine = await (await page.request.get(`${API}/workspaces`)).json();
    expect(
      mine.data.some((w: { id: string }) => w.id === workspace.id),
      'the invite was used on the way back from the provider',
    ).toBe(true);
  } finally {
    await admin.delete(`${API}/workspaces/${workspace.id}?hard=true`);
    await admin.dispose();
  }
});
