import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace, createChannel } from './helpers';

test('a draft follows its author to another browser, and sending it clears it on both', async ({
  page,
  browser,
}) => {
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(bob);
  const channel = await createChannel(bob, workspace.id, 'drafts');
  const text = `half a thought ${Date.now()}`;
  const otherContext = await browser.newContext();
  const other = await otherContext.newPage();
  const serverHas = async () => {
    const drafts = await (await bob.get(`${API}/workspaces/${workspace.id}/drafts`)).json();
    return drafts.data.some(
      (d: { channel_id: string; content: string }) => d.channel_id === channel.id && d.content === text,
    );
  };

  try {
    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    const editor = page.locator('.ProseMirror[contenteditable="true"]').last();
    await editor.click();
    await editor.fill(text);
    await expect.poll(serverHas, { message: 'the draft reaches the server after a pause' }).toBe(true);

    await login(other, 'bob@dev.local');
    await other.goto(`/app/${workspace.id}/${channel.id}`);
    const otherEditor = other.locator('.ProseMirror[contenteditable="true"]').last();
    await expect(otherEditor).toContainText(text);
    await other.locator('[data-qa="open-you-menu"]').click();
    await other.locator('[data-qa="open-drafts"]').click();
    const row = other.getByTestId('draft-row').filter({ hasText: text });
    await expect(row).toBeVisible();

    await editor.press('Enter');
    await expect(row).toHaveCount(0);
    await expect(otherEditor).not.toContainText(text);
    expect(await serverHas()).toBe(false);

    await other.getByTestId('drafts-tab-sent').click();
    const sent = other.getByTestId('sent-row').filter({ hasText: text });
    await expect(sent).toBeVisible();
    const messageId = await sent.getAttribute('data-message-id');
    await sent.click();
    await expect(other).toHaveURL(new RegExp(`/${channel.id}/${messageId}`));
  } finally {
    await bob.delete(`${API}/channels/${channel.id}`);
    await otherContext.close();
    await bob.dispose();
  }
});
