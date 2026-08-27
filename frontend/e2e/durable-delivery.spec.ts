import { test, expect } from '@playwright/test';
import { execSync } from 'node:child_process';
import { API, login, userContext, devWorkspace } from './helpers';

const REPO_ROOT = new URL('../..', import.meta.url).pathname;

/**
 * Reconnecting and refetching what is on screen was the old guarantee. This
 * asserts the new one: an event that arrived while the socket was down reaches
 * the client even when it concerns a view the client is not looking at, which
 * the refetch path could never do because that query is not mounted.
 */
test('a message sent while the socket is down still marks its channel unread', async ({ page }) => {
  test.setTimeout(120_000);
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bobCtx, userId: bobId } = await userContext('bob@dev.local');

  const workspace = await devWorkspace(admin);
  const stamp = Date.now();

  const watched = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `watched-${stamp}`, channel_type: 'public' },
  });
  const watchedId = (await watched.json()).id as string;
  const elsewhere = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `elsewhere-${stamp}`, channel_type: 'public' },
  });
  const elsewhereId = (await elsewhere.json()).id as string;

  for (const id of [watchedId, elsewhereId]) {
    await admin.post(`${API}/channels/${id}/members`, { data: { user_id: bobId } });
  }

  try {
    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${watchedId}`);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(`watched-${stamp}`);

    execSync('docker compose stop realtime', { cwd: REPO_ROOT });
    await expect(page.locator('[data-qa="connection-banner"]')).toBeVisible({ timeout: 30_000 });

    // Posted into the channel bob is *not* looking at, so nothing on screen
    // would refetch it back.
    const posted = await admin.post(`${API}/channels/${elsewhereId}/messages`, {
      data: { content: `while-you-were-out ${stamp}` },
    });
    expect(posted.status()).toBe(200);

    execSync('docker compose start realtime', { cwd: REPO_ROOT });
    await expect(page.locator('[data-qa="connection-banner"]')).toHaveCount(0, { timeout: 60_000 });

    const badge = page.locator(`[data-qa="channel-unread-badge"][data-channel-id="${elsewhereId}"]`);
    await expect(badge, 'the replayed event has to reach a view that was never open').toBeVisible({
      timeout: 30_000,
    });
    await expect(badge).toHaveText('1');
  } finally {
    execSync('docker compose start realtime', { cwd: REPO_ROOT });
    await admin.delete(`${API}/channels/${watchedId}`);
    await admin.delete(`${API}/channels/${elsewhereId}`);
    await admin.dispose();
    await bobCtx.dispose();
  }
});
