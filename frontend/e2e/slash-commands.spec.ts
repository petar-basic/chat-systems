import { test, expect, type APIRequestContext, type Page } from '@playwright/test';
import { API, authHeaders, login, send, devWorkspace } from './helpers';

// Three round trips through the composer plus a sign-in; the default 30s is
// tight on a seeded instance.
test.setTimeout(60_000);

async function openGeneral(page: Page, admin: APIRequestContext) {
  const workspace = await devWorkspace(admin);
  const channels = (await (await admin.get(`${API}/workspaces/${workspace.id}/channels`)).json()).data;
  const channel = channels.find((c: { name: string }) => c.name === 'general');
  await login(page, 'admin@dev.local');
  await page.goto(`/app/${workspace.id}/${channel.id}`);
  await expect(page.locator('[data-qa="message-list"]')).toBeVisible({ timeout: 20_000 });
  return { workspace, channel };
}

test('a built-in command answers only the person who ran it, and does the thing', async ({
  page,
  playwright,
}) => {
  const admin = await playwright.request.newContext({
    extraHTTPHeaders: await authHeaders(await playwright.request.newContext(), 'admin@dev.local'),
  });

  const workspace = await devWorkspace(admin);
  const dndRoute = `${API}/workspaces/${workspace.id}/notifications/dnd`;

  try {
    await openGeneral(page, admin);
    await send(page, '/dnd 15');

    const response = page.locator('[data-qa="command-response"]');
    await expect(response).toBeVisible();
    await expect(response).toContainText('Notifications paused');
    await expect(
      page.locator('[data-qa="message-list"]').getByText('/dnd 15'),
      'the command itself is not posted, and neither is the answer',
    ).toHaveCount(0);

    const dnd = await (await admin.get(dndRoute)).json();
    expect(dnd.dnd_until, 'the command changed real state, not just the reply').toBeTruthy();
  } finally {
    await admin.patch(dndRoute, { data: { dnd_until: null } });
    await admin.dispose();
  }
});

test('an unknown command is refused and stays in the composer to fix', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext({
    extraHTTPHeaders: await authHeaders(await playwright.request.newContext(), 'admin@dev.local'),
  });

  try {
    await openGeneral(page, admin);
    const typed = `/nosuchcommand ${Date.now()}`;
    await send(page, typed);

    await expect(page.locator('[data-qa="command-response"]')).toContainText(
      '/nosuchcommand is not a command here',
    );
    await expect(page.locator('.ProseMirror[contenteditable="true"]').last()).toHaveText(typed);
    await expect(page.locator('[data-qa="message-list"]').getByText(typed)).toHaveCount(0);
  } finally {
    await admin.dispose();
  }
});

test('typing a slash offers the commands, and picking one fills it in', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext({
    extraHTTPHeaders: await authHeaders(await playwright.request.newContext(), 'admin@dev.local'),
  });

  try {
    await openGeneral(page, admin);
    const editor = page.locator('.ProseMirror[contenteditable="true"]').last();
    await editor.click();
    await editor.pressSequentially('/sh');

    const suggestions = page.getByTestId('command-suggestion');
    await expect(suggestions).toHaveCount(1);
    await expect(suggestions.first()).toContainText('/shrug');

    await editor.press('Enter');
    await expect(page.getByTestId('command-suggestions')).toHaveCount(0);
    await expect(editor).toHaveText('/shrug');
  } finally {
    await admin.dispose();
  }
});
