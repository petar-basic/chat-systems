import { test, expect, type Locator } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

const ONE_PIXEL_PNG =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';

/**
 * A real paste and a real drop both arrive as an event carrying a DataTransfer,
 * which is the one part of them a test can build. Playwright can hand a file to
 * a file input, and nothing else here goes through one.
 */
async function dropFileOn(target: Locator, kind: 'paste' | 'drop') {
  await target.evaluate(
    (el, { kind: eventKind, b64 }) => {
      const binary = atob(b64);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
      const file = new File([bytes], `${eventKind}-${Date.now()}.png`, { type: 'image/png' });
      const transfer = new DataTransfer();
      transfer.items.add(file);
      const event =
        eventKind === 'paste'
          ? new ClipboardEvent('paste', { clipboardData: transfer, bubbles: true, cancelable: true })
          : new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true });
      el.dispatchEvent(event);
    },
    { kind, b64: ONE_PIXEL_PNG },
  );
}

async function ownChannel(prefix: string) {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, prefix);
  return { alice, workspace, channel };
}

test('an image pasted into the composer is posted as an attachment', async ({ page }) => {
  test.slow();
  const { alice, workspace, channel } = await ownChannel('paste');
  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(channel.name);

    const composer = page.locator('.ProseMirror[contenteditable="true"]').last();
    await composer.click();
    await dropFileOn(composer, 'paste');

    await expect(page.locator('[data-qa="attachment-image"]')).toHaveCount(1, { timeout: 20_000 });
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});

test('an image dropped on the message list is posted as an attachment', async ({ page }) => {
  test.slow();
  const { alice, workspace, channel } = await ownChannel('drop');
  // An empty channel renders its empty state instead of the list, and the list
  // is the thing being dropped on.
  const seeded = `drop-target-${Date.now()}`;
  await alice.post(`${API}/channels/${channel.id}/messages`, { data: { content: seeded } });

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(channel.name);
    await expect(page.locator('[data-qa="message-list"]').getByText(seeded)).toBeVisible({
      timeout: 20_000,
    });

    await dropFileOn(page.locator('[data-qa="message-list"]'), 'drop');

    await expect(page.locator('[data-qa="attachment-image"]')).toHaveCount(1, { timeout: 20_000 });
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
