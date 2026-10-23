import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace, createChannel } from './helpers';

test('a private channel is created from the visibility cards, named the way it will appear', async ({
  page,
}) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const stamp = Date.now();
  const expected = `plan-budget-${stamp}`;
  let createdId: string | undefined;

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}`);

    await page.getByRole('navigation').getByRole('button', { name: 'Create channel' }).click();
    const dialog = page.getByTestId('create-channel-modal');
    await expect(dialog).toBeVisible();

    const name = page.getByTestId('create-channel-name');
    await expect(name).toBeFocused();
    await name.pressSequentially(`Plan Budget ${stamp}`);
    await expect(name).toHaveValue(expected);

    await page.getByTestId('create-channel-private').click();
    await expect(page.getByRole('radio', { name: 'Private' })).toBeChecked();
    await page.getByTestId('create-channel-submit').click();
    await expect(dialog).toBeHidden();

    const listed = await (await alice.get(`${API}/workspaces/${workspace.id}/channels`)).json();
    const created = listed.data.find((c: { name: string }) => c.name === expected);
    expect(created, 'the channel exists under the name the field showed').toBeTruthy();
    expect(created.channel_type).toBe('private');
    createdId = created.id;
  } finally {
    if (createdId) await alice.delete(`${API}/channels/${createdId}`);
    await alice.dispose();
  }
});

test('deleting a message asks first and shows the message', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'delete-confirm');
  const text = `delete me ${Date.now()}`;

  try {
    const posted = await admin.post(`${API}/channels/${channel.id}/messages`, { data: { content: text } });
    expect(posted.status(), 'post the message').toBe(200);

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);

    const row = page.locator('[data-qa="message-row"]', { hasText: text });
    const openConfirmation = async () => {
      await row.hover();
      await row.locator('[data-qa="message-action-more"]').click();
      await page.locator('[data-qa="message-overflow"] [data-qa="message-action-delete"]').click();
    };

    await openConfirmation();
    const dialog = page.getByRole('dialog', { name: 'Delete message?' });
    await expect(dialog).toBeVisible();
    await expect(page.getByTestId('message-delete-preview')).toContainText(text);
    await expect(page.getByTestId('message-delete-confirm')).toBeFocused();

    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
    await expect(row).toBeVisible();

    await openConfirmation();
    await page.getByTestId('message-delete-confirm').click();
    await expect(dialog).toBeHidden();
    await expect(row).toHaveCount(0, { timeout: 10_000 });
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
  }
});
