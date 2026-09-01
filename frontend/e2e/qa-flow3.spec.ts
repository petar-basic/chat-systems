import { test, expect, type Page } from '@playwright/test';
import { authHeaders, devWorkspace, login, SHOTS } from './helpers';

/// Opens the seeded workspace by id first. Landing on `/app` picks whichever
/// workspace the instance happens to sort first, which is not this suite's to
/// assume — an import, or anybody creating one, changes it.
async function openMembersPanel(page: Page, workspaceId: string) {
  await page.goto(`/app/${workspaceId}`);
  await page
    .getByRole('button', { name: /Dev Team/ })
    .first()
    .click();
  await page.getByRole('button', { name: 'Members', exact: true }).click();
  await expect(page.getByText(/\d+ members?/)).toBeVisible({ timeout: 10_000 });
}

test('G. instance admin page renders for the instance admin', async ({ page }) => {
  await login(page, 'admin@dev.local');
  await page.goto('/app/admin');
  await expect(page.locator('[data-qa="admin-tab-users"]')).toBeVisible({ timeout: 15_000 });
  await expect(page.getByRole('heading', { name: 'Instance Admin' })).toBeVisible();
  await page.screenshot({ path: `${SHOTS}/G-instance-admin.png`, fullPage: true });
});

test('H. owner can change a member role and remove a member from the panel', async ({ page, request }) => {
  await login(page, 'admin@dev.local');
  const workspace = await devWorkspace(request, 'admin@dev.local');
  await openMembersPanel(page, workspace.id);

  const roleSelect = page.getByLabel('Role for Alice Johnson');
  await expect(roleSelect).toBeVisible();
  await expect(roleSelect).toHaveValue('admin');
  await roleSelect.selectOption('member');
  await page.screenshot({ path: `${SHOTS}/H2-workspace-members.png`, fullPage: true });

  const auth = await authHeaders(request, 'admin@dev.local');
  const listed = await (await request.get('http://localhost:3000/api/workspaces', { headers: auth })).json();
  const ws = listed.data.find((w: { name: string }) => w.name === 'Dev Team');
  await expect
    .poll(
      async () => {
        const members = (
          await (
            await request.get(`http://localhost:3000/api/workspaces/${ws.id}/members`, { headers: auth })
          ).json()
        ).data;
        return members.find((m: { email: string }) => m.email === 'alice@dev.local').role;
      },
      { timeout: 10_000 },
    )
    .toBe('member');

  await roleSelect.selectOption('admin');
  await expect
    .poll(
      async () => {
        const members = (
          await (
            await request.get(`http://localhost:3000/api/workspaces/${ws.id}/members`, { headers: auth })
          ).json()
        ).data;
        return members.find((m: { email: string }) => m.email === 'alice@dev.local').role;
      },
      { timeout: 10_000 },
    )
    .toBe('admin');

  await expect(page.getByLabel('Role for Admin')).toHaveCount(0);
  await expect(page.getByLabel('Remove Admin')).toHaveCount(0);
});

test('H2. a plain member sees no role controls', async ({ page, request }) => {
  await login(page, 'bob@dev.local');
  const workspace = await devWorkspace(request, 'bob@dev.local');
  await openMembersPanel(page, workspace.id);
  await expect(page.locator('[data-qa="member-role-select"]')).toHaveCount(0);
  await expect(page.locator('[data-qa="member-remove"]')).toHaveCount(0);
  await page.screenshot({ path: `${SHOTS}/H3-member-view.png` });
});

test('I. notification badge + mark-all-read while viewing another channel', async ({ page, request }) => {
  const auth = await authHeaders(request, 'admin@dev.local');
  const ws = await devWorkspace(request, 'admin@dev.local');
  const chans = (
    await (
      await request.get(`http://localhost:3000/api/workspaces/${ws.id}/channels`, { headers: auth })
    ).json()
  ).data;
  const general = chans.find((c: { name: string }) => c.name === 'general').id;
  const members = (
    await (
      await request.get(`http://localhost:3000/api/workspaces/${ws.id}/members`, { headers: auth })
    ).json()
  ).data;
  const bobId = members.find((m: { email: string }) => m.email === 'bob@dev.local').user_id;

  await login(page, 'bob@dev.local');
  await page.goto(`/app/${ws.id}`);
  await page
    .getByRole('button', { name: /^random$/ })
    .first()
    .click();
  // The mention must land while bob is somewhere else, so the channel he moved
  // to has to be on screen before it is sent.
  await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('random');

  await request.post(`http://localhost:3000/api/channels/${general}/messages`, {
    headers: auth,
    data: { content: `@[Bob Smith](${bobId}) badge probe` },
  });

  await page.getByTitle('Notifications').click();
  const panel = page.locator('[data-qa="notifications-panel"]');
  await expect(panel).toBeVisible();
  // The mention becomes a notification through the event consumer, so wait for
  // this run's own probe rather than for any unread: marking all read before it
  // lands leaves it unread and the button on screen.
  await expect(panel).toContainText('badge probe', { timeout: 15_000 });
  await expect(page.getByRole('button', { name: /Mark all read/i })).toBeVisible({ timeout: 15_000 });
  await page.screenshot({ path: `${SHOTS}/I1-notifications-unread.png` });
  await page.getByRole('button', { name: /Mark all read/i }).click();
  await expect(page.getByRole('button', { name: /Mark all read/i })).toHaveCount(0, { timeout: 15_000 });
  await page.screenshot({ path: `${SHOTS}/I2-notifications-read.png` });

  await page.reload();
  await expect(page.locator('.ProseMirror[contenteditable="true"]')).toBeVisible({ timeout: 20_000 });
  await page.screenshot({ path: `${SHOTS}/I3-after-reload.png` });
  await page.getByTitle('Notifications').click();
  await expect(page.getByRole('button', { name: /Mark all read/i })).toHaveCount(0);
  await page.screenshot({ path: `${SHOTS}/I3-notifications-after-reload.png` });
});
