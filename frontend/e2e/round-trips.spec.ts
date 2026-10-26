import { test, expect, type Page } from '@playwright/test';
import { API, login, userContext, devWorkspace, createChannel } from './helpers';

const PER_CHANNEL_ROUTE =
  /\/channels\/[^/]+\/(members|pins|bookmarks|workflow-shortcuts|read)$|\/notifications\/read$|\/lists\?channel_id=/;

function recordApi(page: Page) {
  const seen: string[] = [];
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.pathname.startsWith('/api/')) seen.push(`${request.method()} ${url.pathname}${url.search}`);
  });
  return seen;
}

test('switching to a channel not opened before is two requests: its context and its messages', async ({
  page,
}) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(alice);
  const from = await createChannel(alice, workspace.id, 'trips-from');
  const to = await createChannel(alice, workspace.id, 'trips-to');

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${from.id}`);
    await expect(page.getByTestId('channel-header-name')).toHaveText(from.name);
    await expect(page.getByTestId('channel-intro')).toBeVisible();

    const seen = recordApi(page);
    const messages = page.waitForResponse((r) => r.url().includes(`/api/channels/${to.id}/messages`));
    const context = page.waitForResponse((r) => r.url().includes(`/api/channels/${to.id}/context`));
    await page
      .getByTestId('sidebar-channel-row')
      .filter({ hasText: to.name })
      .locator('button[data-nav-item]')
      .click();
    await expect(page.getByTestId('channel-header-name')).toHaveText(to.name);
    expect((await messages).status()).toBe(200);
    expect((await context).status()).toBe(200);
    await expect(page.getByTestId('channel-intro')).toBeVisible();
    await page.waitForTimeout(1000);

    const touching = seen.filter((r) => r.includes(to.id) || PER_CHANNEL_ROUTE.test(r));
    expect(touching.sort()).toEqual([
      `GET /api/channels/${to.id}/context`,
      `GET /api/channels/${to.id}/messages?limit=50&mark_read=true`,
    ]);
  } finally {
    await admin.delete(`${API}/channels/${from.id}`);
    await admin.delete(`${API}/channels/${to.id}`);
    await alice.dispose();
    await admin.dispose();
  }
});
