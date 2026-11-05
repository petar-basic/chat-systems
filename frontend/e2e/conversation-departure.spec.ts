import { test, expect, type APIRequestContext } from '@playwright/test';
import { API, PASSWORD, login, userContext, devWorkspace } from './helpers';

async function inviteLink(admin: APIRequestContext, workspaceId: string, email: string) {
  const res = await admin.post(`${API}/workspaces/${workspaceId}/invites`, { data: { email } });
  expect(res.status(), `invite ${email}`).toBe(200);
  const invite = await res.json();
  const linked = await admin.get(`${API}/workspaces/${workspaceId}/invites/${invite.id}/link`);
  return ((await linked.json()).url ?? invite.invite_url) as string;
}

test('a DM outlives the other side leaving and carries on when they come back', async ({ page, browser }) => {
  const { ctx: admin, userId: adminId } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const stamp = Date.now();
  const email = `departing-${stamp}@example.com`;
  let leaverId: string | null = null;

  try {
    const guest = await browser.newContext();
    const newcomer = await guest.newPage();
    await newcomer.goto(new URL(await inviteLink(admin, workspace.id, email)).pathname);
    await newcomer.getByLabel('Display name').fill('Departing Dana');
    await newcomer.getByLabel('Password', { exact: true }).fill(PASSWORD);
    await newcomer.getByLabel('Confirm password').fill(PASSWORD);
    await newcomer.getByRole('button', { name: 'Create account' }).click();
    await newcomer.waitForURL(new RegExp(`/app/${workspace.id}`), { timeout: 20_000 });
    await guest.close();

    const { ctx: leaver, userId } = await userContext(email);
    leaverId = userId;
    const direct = await (
      await leaver.post(`${API}/workspaces/${workspace.id}/conversations`, {
        data: { participant_ids: [adminId] },
      })
    ).json();
    const said = `before-I-go-${stamp}`;
    expect(
      (await leaver.post(`${API}/channels/${direct.id}/messages`, { data: { content: said } })).status(),
    ).toBe(200);
    expect((await leaver.delete(`${API}/workspaces/${workspace.id}/members/${userId}`)).status()).toBe(200);

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/c/${direct.id}`);
    await expect(page.getByTestId('conversation-title')).toHaveText('Departing Dana');
    await expect(page.getByText(said)).toBeVisible();
    await expect(page.getByTestId('conversation-departed')).toHaveText(
      'Departing Dana is no longer in this workspace',
    );
    await expect(page.locator('.ProseMirror[contenteditable="true"]')).toHaveCount(0);
    expect(
      (await admin.post(`${API}/channels/${direct.id}/messages`, { data: { content: 'anyone?' } })).status(),
      'nobody would read it',
    ).toBe(403);

    const again = new URL(await inviteLink(admin, workspace.id, email)).pathname.split('/').pop();
    expect((await leaver.post(`${API}/auth/invites/${again}/accept`)).status(), 'they come back').toBe(200);

    await expect(page.getByTestId('conversation-departed')).toBeHidden();
    await expect(page.locator('.ProseMirror[contenteditable="true"]').last()).toBeVisible();
    const reopened = await (
      await admin.post(`${API}/workspaces/${workspace.id}/conversations`, {
        data: { participant_ids: [userId] },
      })
    ).json();
    expect(reopened.id, 'the same DM, not a new one').toBe(direct.id);
    const history = await (await leaver.get(`${API}/channels/${direct.id}/messages`)).json();
    expect(history.data.some((m: { content: string }) => m.content === said)).toBe(true);
    await leaver.dispose();
  } finally {
    if (leaverId) await admin.delete(`${API}/workspaces/${workspace.id}/members/${leaverId}`);
    await admin.dispose();
  }
});
