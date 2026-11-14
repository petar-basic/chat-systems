import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace, openWorkspaceSettings } from './helpers';

test('an admin defines a profile field and People filters by it', async ({ page }) => {
  const { ctx: admin, userId: adminId } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const label = `Department ${Date.now()}`;
  let fieldId: string | undefined;

  try {
    const members = await (await admin.get(`${API}/workspaces/${workspace.id}/members`)).json();
    const bobName: string = members.data.find((m: { user_id: string }) => m.user_id === bobId).display_name;

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await openWorkspaceSettings(page, 'profile-fields');
    await page.getByTestId('profile-fields-add').click();
    await page.getByTestId('profile-field-label').fill(label);
    await page.getByTestId('profile-field-kind').selectOption('options');
    const options = page.getByTestId('profile-field-option-label');
    await options.nth(0).fill('Finance');
    await options.nth(1).fill('Support');
    await page.getByTestId('profile-field-save').click();
    await expect(page.getByTestId('profile-fields-row').filter({ hasText: label })).toBeVisible();

    const listed = await (await admin.get(`${API}/workspaces/${workspace.id}/profile-fields`)).json();
    const field = listed.data.find((f: { label: string }) => f.label === label);
    fieldId = field.id;
    const optionId = (name: string) => field.options.find((o: { label: string }) => o.label === name).id;
    const finance = optionId('Finance');

    const bobSet = await bob.put(`${API}/workspaces/${workspace.id}/members/${bobId}/profile-fields`, {
      data: { values: { [field.id]: finance } },
    });
    expect(bobSet.status(), 'a member fills in their own field').toBe(200);
    const adminSet = await admin.put(`${API}/workspaces/${workspace.id}/members/${adminId}/profile-fields`, {
      data: { values: { [field.id]: optionId('Support') } },
    });
    expect(adminSet.status()).toBe(200);

    await page.goto(`/app/${workspace.id}/people`);
    await page.getByTestId(`people-filter-${field.id}`).selectOption(finance);
    await expect(page).toHaveURL(new RegExp(`filter=${field.id}%3A${finance}`));
    const rows = page.getByTestId('people-row');
    await expect(rows).toHaveCount(1);
    await expect(rows.first()).toContainText(bobName);

    await page.reload();
    await expect(page.getByTestId(`people-filter-${field.id}`)).toHaveValue(finance);
    await expect(rows).toHaveCount(1);

    await rows.first().getByRole('button').first().click();
    await expect(page.getByTestId('person-panel-fields')).toContainText(label);
    await expect(page.getByTestId('person-panel-fields')).toContainText('Finance');
  } finally {
    if (fieldId) await admin.delete(`${API}/workspaces/${workspace.id}/profile-fields/${fieldId}`);
    await bob.dispose();
    await admin.dispose();
  }
});
