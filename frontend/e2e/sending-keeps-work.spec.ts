import { test, expect, type Page } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

const ONE_PIXEL_PNG =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';

async function pasteImage(page: Page, composer: ReturnType<Page['locator']>) {
  await composer.evaluate((el, b64) => {
    const binary = atob(b64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
    const transfer = new DataTransfer();
    transfer.items.add(new File([bytes], `kept-${Date.now()}.png`, { type: 'image/png' }));
    el.dispatchEvent(
      new ClipboardEvent('paste', { clipboardData: transfer, bubbles: true, cancelable: true }),
    );
  }, ONE_PIXEL_PNG);
  await expect(page.locator('[data-qa="attachment-tray-item"][data-status="ready"]')).toHaveCount(1, {
    timeout: 20_000,
  });
}

async function cutSends(page: Page) {
  await page.route('**/api/channels/*/messages', (route) =>
    route.request().method() === 'POST' ? route.abort('internetdisconnected') : route.continue(),
  );
}

test('a send that loses the network keeps its text and files until it is retried', async ({ page }) => {
  test.slow();
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'kept-send');
  const text = `kept-${Date.now()}`;

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(channel.name);

    const composer = page.locator('.ProseMirror[contenteditable="true"]').last();
    await composer.click();
    await pasteImage(page, composer);
    await composer.pressSequentially(text);

    await cutSends(page);
    await page.locator('[data-qa="composer-send"]').last().click();
    const row = page.locator('[data-qa="message-row"]', { hasText: text });
    await expect(row.locator('[data-qa="message-failed"]')).toBeVisible({ timeout: 15_000 });
    await expect(row.locator('[data-qa="attachment-image"]')).toHaveCount(1);

    await page.getByRole('navigation').getByText('general', { exact: true }).click();
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('general');
    await page.getByRole('navigation').getByText(channel.name).click();
    await expect(row.locator('[data-qa="message-failed"]'), 'a refetch does not lose it').toBeVisible();

    await page.unroute('**/api/channels/*/messages');
    await row.locator('[data-qa="message-retry"]').click();
    await expect(row.locator('[data-qa="message-failed"]')).toHaveCount(0, { timeout: 15_000 });
    await expect(row.locator('[data-qa="attachment-image"]')).toHaveCount(1);

    const listed = await (await alice.get(`${API}/channels/${channel.id}/messages`)).json();
    const sent = listed.data.find((m: { content: string }) => m.content === text);
    expect(sent.metadata.attachments, 'the retry carried the file').toHaveLength(1);
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});

test('a thread reply that loses the network can be retried or put back', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'kept-reply');
  const stamp = Date.now();

  try {
    const root = await (
      await alice.post(`${API}/channels/${channel.id}/messages`, { data: { content: `root-${stamp}` } })
    ).json();

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}/${root.id}?thread=1`);
    const panel = page.locator('[data-qa="thread-panel"]');
    await expect(panel).toBeVisible({ timeout: 20_000 });
    const editor = panel.locator('.ProseMirror[contenteditable="true"]');

    await cutSends(page);
    for (const text of [`retried-${stamp}`, `put-back-${stamp}`]) {
      await editor.click();
      await editor.fill(text);
      await editor.press('Enter');
      await expect(
        panel.locator('[data-qa="thread-reply"]', { hasText: text }).locator('[data-qa="message-failed"]'),
      ).toBeVisible({ timeout: 15_000 });
    }

    await panel
      .locator('[data-qa="thread-reply"]', { hasText: `put-back-${stamp}` })
      .locator('[data-qa="message-discard"]')
      .click();
    await expect(panel.locator('[data-qa="thread-reply"]', { hasText: `put-back-${stamp}` })).toHaveCount(0);
    await expect(editor).toContainText(`put-back-${stamp}`);

    await page.unroute('**/api/channels/*/messages');
    const retried = panel.locator('[data-qa="thread-reply"]', { hasText: `retried-${stamp}` });
    await retried.locator('[data-qa="message-retry"]').click();
    await expect(retried.locator('[data-qa="message-failed"]')).toHaveCount(0, { timeout: 15_000 });
    await expect(panel.locator('[data-qa="thread-reply-count"]')).toHaveText('1 reply');
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
