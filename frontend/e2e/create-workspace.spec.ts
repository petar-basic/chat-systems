import { test, expect } from '@playwright/test';
import { API, authHeaders, login } from './helpers';

test('creating a workspace opens it, and its sidebar says nobody else is there yet', async ({
  page,
  playwright,
}) => {
  const admin = await playwright.request.newContext({
    extraHTTPHeaders: await authHeaders(await playwright.request.newContext(), 'admin@dev.local'),
  });
  const name = `Fresh ${Date.now()}`;
  let createdId: string | undefined;

  try {
    await login(page, 'admin@dev.local');
    await page
      .getByRole('button', { name: /^Create workspace on / })
      .first()
      .click();
    const dialog = page.getByTestId('create-workspace-modal');
    await dialog.getByLabel('Workspace name').fill(name);
    await dialog.getByRole('button', { name: 'Create' }).click();

    await expect(page.locator('[data-qa="workspace-menu"]')).toContainText(name, { timeout: 15_000 });
    await page.waitForTimeout(1000);
    await expect(page.locator('[data-qa="workspace-menu"]')).toContainText(name);
    createdId = /\/app\/([0-9a-f-]{36})/.exec(page.url())?.[1];
    const listed = (await (await admin.get(`${API}/workspaces`)).json()).data;
    expect(listed.find((w: { id: string }) => w.id === createdId)?.name).toBe(name);

    await expect(page.getByTestId('sidebar-people-empty')).toContainText('Just you so far.');
    await expect(page.getByTestId('sidebar-people-invite')).toBeVisible();
  } finally {
    if (createdId) await admin.delete(`${API}/workspaces/${createdId}`);
    await admin.dispose();
  }
});
