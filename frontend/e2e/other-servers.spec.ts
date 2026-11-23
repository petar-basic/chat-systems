import { test, expect, type Page } from '@playwright/test';
import { API, login, userContext } from './helpers';

const ADD = { name: 'Add instance' };

async function openSignIn(page: Page) {
  const info = page.waitForResponse((r) => r.url().includes('/api/instance/info') && r.ok());
  await page.goto('/');
  await info;
  await expect(page.getByTestId('login-email')).toBeVisible();
}

async function reloadRail(page: Page) {
  const info = page.waitForResponse((r) => r.url().includes('/api/instance/info') && r.ok());
  await page.reload();
  await info;
  await expect(page.getByTestId('workspace-tile').first()).toBeVisible();
}

test('an admin keeps everyone to this server', async ({ browser }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const bobContext = await browser.newContext();
  const adminContext = await browser.newContext();
  const signedOutContext = await browser.newContext();
  const bob = await bobContext.newPage();
  const adminPage = await adminContext.newPage();
  const signedOut = await signedOutContext.newPage();

  try {
    await login(bob, 'bob@dev.local');
    await expect(bob.getByTestId('workspace-tile').first()).toBeVisible();
    await expect(bob.getByRole('button', ADD)).toBeVisible();
    await openSignIn(signedOut);
    await expect(signedOut.getByLabel('Server URL')).toBeVisible();

    await login(adminPage, 'admin@dev.local');
    await adminPage.goto('/app/admin');
    const toggle = adminPage
      .getByTestId('other-servers-setting')
      .getByLabel('Let people connect to other servers');
    await expect(toggle).toBeChecked();
    await toggle.click();
    await expect(toggle).not.toBeChecked();
    const info = await (await admin.get(`${API}/instance/info`)).json();
    expect(info.other_servers_allowed).toBe(false);

    await reloadRail(bob);
    await expect(bob.getByRole('button', ADD)).toHaveCount(0);

    await openSignIn(signedOut);
    await expect(signedOut.getByLabel('Server URL')).toHaveCount(0);

    await toggle.click();
    await expect(toggle).toBeChecked();
    await reloadRail(bob);
    await expect(bob.getByRole('button', ADD)).toBeVisible();
  } finally {
    await admin.patch(`${API}/admin/settings`, { data: { other_servers_allowed: true } });
    await admin.dispose();
    await bobContext.close();
    await adminContext.close();
    await signedOutContext.close();
  }
});
