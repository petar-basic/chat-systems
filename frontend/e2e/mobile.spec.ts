import { test, expect } from '@playwright/test';
import { API, authHeaders, login } from './helpers';
import type { APIRequestContext } from '@playwright/test';

/// Picks a channel and makes sure it has something in it: the message list only
/// mounts when there are messages, and which workspace `data[0]` returns depends
/// on what earlier specs left behind.
async function seededChannel(admin: APIRequestContext) {
  const workspace = (await (await admin.get(`${API}/workspaces`)).json()).data[0];
  const channels = (
    await (await admin.get(`${API}/workspaces/${workspace.id}/channels`)).json()
  ).data;
  const channel = channels.find((c: { name: string }) => c.name === 'general') ?? channels[0];

  const posted = await admin.post(`${API}/channels/${channel.id}/messages`, {
    data: { content: `mobile fixture ${Date.now()}` },
  });
  expect(posted.status(), 'the fixture message').toBe(200);

  return { workspace, channel };
}


/// Reading, opening a channel and sending have to work on a phone-sized screen
/// with the sidebar collapsed — that is the journey the whole ticket is about.
test('the core journey works on a phone-sized screen', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext({
    extraHTTPHeaders: await authHeaders(await playwright.request.newContext(), 'admin@dev.local'),
  });

  try {
    const { workspace, channel } = await seededChannel(admin);

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="message-list"]')).toBeVisible({ timeout: 20_000 });

    // The sidebar is a drawer here, so the message pane owns the width.
    const list = page.locator('[data-qa="message-list"]');
    const viewport = page.viewportSize();
    const box = await list.boundingBox();
    expect(box, 'the message list is laid out').toBeTruthy();
    expect(box!.width).toBeGreaterThan((viewport?.width ?? 0) * 0.8);

    const stamp = `mobile hello ${Date.now()}`;
    const editor = page.locator('.ProseMirror[contenteditable="true"]').last();
    await editor.click();
    await editor.fill(stamp);
    await editor.press('Enter');
    await expect(list.getByText(stamp)).toBeVisible();
  } finally {
    await admin.dispose();
  }
});

/// The composer has to stay reachable: on iOS anything under 16px makes Safari
/// zoom the page on focus, and the home indicator overlaps the send button
/// without safe-area padding.
test('the composer is readable and clear of the bottom edge', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext({
    extraHTTPHeaders: await authHeaders(await playwright.request.newContext(), 'admin@dev.local'),
  });

  try {
    const { workspace, channel } = await seededChannel(admin);

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    const editor = page.locator('.ProseMirror[contenteditable="true"]').last();
    await expect(editor).toBeVisible({ timeout: 20_000 });

    const fontSize = await editor.evaluate((el) => parseFloat(getComputedStyle(el).fontSize));
    expect(fontSize, 'under 16px and iOS zooms the whole app on focus').toBeGreaterThanOrEqual(16);

    const box = await editor.boundingBox();
    const viewport = page.viewportSize();
    expect(box!.y + box!.height).toBeLessThanOrEqual((viewport?.height ?? 0) - 8);
  } finally {
    await admin.dispose();
  }
});

/// A dialog on a phone is a sheet that reaches the bottom edge, not a card
/// floating in the middle with the page scrolling behind it.
test('a modal becomes a full-width sheet', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext({
    extraHTTPHeaders: await authHeaders(await playwright.request.newContext(), 'admin@dev.local'),
  });

  try {
    const { workspace, channel } = await seededChannel(admin);
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="message-list"]')).toBeVisible({ timeout: 20_000 });

    // The sidebar is a drawer at this width, so it has to be opened first —
    // which is itself the navigation behaviour this ticket is about.
    await page.locator('[data-qa="mobile-nav-toggle"]').click();
    await page.getByRole('button', { name: /browse channels/i }).first().click();
    const dialog = page.getByRole('dialog').first();
    await expect(dialog).toBeVisible();

    const box = await dialog.boundingBox();
    const viewport = page.viewportSize();
    expect(
      box!.width,
      'a dialog opened from the drawer must escape it: a transformed ancestor is \
       the containing block for `position: fixed`',
    ).toBeGreaterThan((viewport?.width ?? 0) * 0.9);
  } finally {
    await admin.dispose();
  }
});
