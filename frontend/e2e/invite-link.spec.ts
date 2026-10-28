import { test, expect } from '@playwright/test';
import { API, PASSWORD, login, userContext, devWorkspace, openWorkspaceSettings } from './helpers';

test('an admin invites by a copied link and revokes a pending invite from the list', async ({
  page,
  browser,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const stamp = Date.now();
  const joining = `link-${stamp}@example.com`;
  const dropped = `dropped-${stamp}@example.com`;
  let joinedId: string | null = null;

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await openWorkspaceSettings(page, 'members');

    await page.getByRole('button', { name: 'Invite people' }).click();
    await page.getByLabel('Email address to invite').fill(joining);
    await page.getByRole('button', { name: 'Invite', exact: true }).click();
    const link = (await page.getByTestId('invite-link-value').textContent())!;
    expect(link).toContain('/invite/');

    const guest = await browser.newContext();
    const newcomer = await guest.newPage();
    await newcomer.goto(new URL(link).pathname);
    await newcomer.getByLabel('Display name').fill('Link Newcomer');
    await newcomer.getByLabel('Password', { exact: true }).fill(PASSWORD);
    await newcomer.getByLabel('Confirm password').fill(PASSWORD);
    await newcomer.getByRole('button', { name: 'Create account' }).click();
    await newcomer.waitForURL(new RegExp(`/app/${workspace.id}`), { timeout: 20_000 });
    await guest.close();

    const members = await (await admin.get(`${API}/workspaces/${workspace.id}/members`)).json();
    joinedId = members.data.find((m: { email: string; user_id: string }) => m.email === joining)?.user_id;
    expect(joinedId, 'the copied link got them in').toBeTruthy();

    await page.getByLabel('Email address to invite').fill(dropped);
    await page.getByRole('button', { name: 'Invite', exact: true }).click();
    const droppedLink = (await page.getByTestId('invite-link-value').textContent())!;
    const row = page.getByTestId('pending-invite-row').filter({ hasText: dropped });
    await expect(row).toBeVisible();
    await row.getByTestId('pending-invite-revoke').click();
    await page.getByTestId('pending-invite-revoke-dialog-confirm').click();
    await expect(row).toBeHidden();

    const token = new URL(droppedLink).pathname.split('/').pop();
    const verify = await admin.get(`${API}/auth/invites/${token}/verify`);
    expect(verify.status(), 'a revoked invite takes its link with it').not.toBe(200);
  } finally {
    if (joinedId) await admin.delete(`${API}/workspaces/${workspace.id}/members/${joinedId}`);
    await admin.dispose();
  }
});
