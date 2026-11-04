import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test('with Enter set to start a new line, two lines go out as one message on ⌘Enter', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'enter-key');
  const first = `first-line-${Date.now()}`;
  const second = `second-line-${Date.now()}`;

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);

    await page.getByTestId('open-you-menu').click();
    await page.getByTestId('open-preferences').click();
    await page.getByRole('tab', { name: 'Advanced' }).click();
    await page.getByTestId('enter-newline').click();
    await expect(page.getByTestId('enter-newline').getByRole('radio')).toBeChecked();
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('preferences-modal')).toBeHidden();

    const editor = page.locator('.ProseMirror[contenteditable="true"]').last();
    await editor.click();
    await editor.pressSequentially(first);
    await editor.press('Enter');
    await editor.pressSequentially(second);
    await expect(page.locator('[data-qa="message-row"]', { hasText: first })).toHaveCount(0);

    await editor.press('ControlOrMeta+Enter');
    const row = page.locator('[data-qa="message-row"]', { hasText: first });
    await expect(row).toBeVisible({ timeout: 10_000 });
    await expect(row).toContainText(second);
    await expect(page.locator('[data-qa="message-row"]', { hasText: second })).toHaveCount(1);
  } finally {
    await alice.patch(`${API}/users/me/preferences`, { data: { enter_key: 'send' } });
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
