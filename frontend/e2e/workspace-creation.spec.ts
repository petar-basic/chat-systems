import { test, expect, type Page } from '@playwright/test';
import { API, login, userContext } from './helpers';

const CREATE = /^Create workspace on /;

async function reloadRail(page: Page) {
  const info = page.waitForResponse((r) => r.url().includes('/api/instance/info') && r.ok());
  await page.reload();
  await info;
  await expect(page.getByTestId('workspace-tile').first()).toBeVisible();
}

test('an admin keeps workspace creation to the people they choose', async ({ browser }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bobApi, userId: bobId } = await userContext('bob@dev.local');
  const bobContext = await browser.newContext();
  const adminContext = await browser.newContext();
  const bob = await bobContext.newPage();
  const adminPage = await adminContext.newPage();

  try {
    await login(bob, 'bob@dev.local');
    await expect(bob.getByRole('button', { name: CREATE })).toBeVisible();

    await login(adminPage, 'admin@dev.local');
    await adminPage.goto('/app/admin');
    const setting = adminPage.getByTestId('workspace-creation-setting');
    await expect(setting.getByRole('radio', { name: 'Everyone' })).toBeChecked();
    await setting.getByText('Workspace creators and admins').click();
    await expect(setting.getByRole('radio', { name: 'Workspace creators and admins' })).toBeChecked();
    const info = await (await admin.get(`${API}/instance/info`)).json();
    expect(info.workspace_creation_restricted).toBe(true);

    await reloadRail(bob);
    await expect(bob.getByRole('button', { name: CREATE })).toHaveCount(0);
    const refused = await bobApi.post(`${API}/workspaces`, { data: { name: `Shadow ${Date.now()}` } });
    expect(refused.status()).toBe(403);

    const role = adminPage.getByLabel('Instance role for Bob', { exact: false });
    await expect(role).toHaveValue('member');
    await role.selectOption('workspace_creator');
    await expect(role).toHaveValue('workspace_creator');

    await reloadRail(bob);
    await expect(bob.getByRole('button', { name: CREATE })).toBeVisible();
  } finally {
    await admin.patch(`${API}/admin/users/${bobId}/instance-role`, { data: { role: 'member' } });
    await admin.patch(`${API}/admin/settings`, { data: { workspace_creation_restricted: false } });
    await admin.dispose();
    await bobApi.dispose();
    await bobContext.close();
    await adminContext.close();
  }
});
