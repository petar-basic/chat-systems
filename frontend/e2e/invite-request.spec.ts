import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace, openWorkspaceMenu, inviteEmailBody } from './helpers';

test('a member asks for an invite, an admin approves it, and the invite names the member', async ({
  page,
  browser,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const stamp = Date.now();
  const first = `req-a-${stamp}@example.com`;
  const second = `req-b-${stamp}@example.com`;
  const adminContext = await browser.newContext();
  const adminPage = await adminContext.newPage();

  try {
    await admin.patch(`${API}/workspaces/${workspace.id}`, { data: { inviters: 'admins' } });
    const members = await (await admin.get(`${API}/workspaces/${workspace.id}/members`)).json();
    const seat = members.data.find((m: { email: string }) => m.email === 'bob@dev.local');
    expect(seat?.role, 'bob asks rather than invites').toBe('member');
    const bobName: string = seat.display_name;

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await openWorkspaceMenu(page);
    await page.getByTestId('open-invite').click();
    await page.getByTestId('invite-request-emails').fill(`${first}, ${second}`);
    await page.getByTestId('invite-request-reason').fill('New hires on the support team');
    await page.getByTestId('invite-request-submit').click();
    await expect(page.getByTestId('invite-request-sent')).toBeVisible();
    await expect(page.getByTestId('my-invite-request-row')).toContainText(first);

    await login(adminPage, 'admin@dev.local');
    await adminPage.goto(`/app/${workspace.id}`);
    await adminPage.getByTestId('open-notifications').click();
    await expect(
      adminPage
        .getByTestId('notification-title')
        .filter({ hasText: `${bobName} asked to invite people` })
        .first(),
    ).toBeVisible({ timeout: 10_000 });
    await adminPage.goto(`/app/${workspace.id}?settings=members`);
    const row = adminPage.getByTestId('invite-request-row').filter({ hasText: first });
    await expect(row).toContainText('New hires on the support team');
    await row.getByTestId('invite-request-approve').click();
    await expect(row).toHaveCount(0);

    const body = await inviteEmailBody(admin, first);
    expect(body, 'the invite says who asked for it').toContain(bobName);
    await inviteEmailBody(admin, second);

    await expect
      .poll(async () => {
        const list = await (await bob.get(`${API}/workspaces/${workspace.id}/notifications`)).json();
        return list.data.some(
          (n: { data: { notice?: string; invite_emails?: string[] } }) =>
            n.data.notice === 'invite_request_approved' && n.data.invite_emails?.includes(first),
        );
      })
      .toBe(true);
    await page.reload();
    await openWorkspaceMenu(page);
    await page.getByTestId('open-invite').click();
    await expect(page.getByTestId('my-invite-requests')).toHaveCount(0);
  } finally {
    const invites = await (await admin.get(`${API}/workspaces/${workspace.id}/invites`)).json();
    for (const invite of invites.data ?? []) {
      if (invite.email === first || invite.email === second) {
        await admin.delete(`${API}/workspaces/${workspace.id}/invites/${invite.id}`);
      }
    }
    await adminContext.close();
    await bob.dispose();
    await admin.dispose();
  }
});
