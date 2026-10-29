import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { API, createChannel, devWorkspace, login, openWorkspaceSettings, userContext } from './helpers';

const REPO_ROOT = new URL('../..', import.meta.url).pathname;
const ECHO_BOT = `${REPO_ROOT}examples/echo-bot/echo-bot.mjs`;

test.setTimeout(60_000);

test('an app made in the settings gets a token that the example bot answers with', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const name = `Echo ${Date.now()}`;

  try {
    await login(page, 'admin@dev.local');
    await openWorkspaceSettings(page, 'apps');
    const form = page.locator('[data-qa="bot-app-form"]');
    await form.getByLabel('App name').fill(name);
    await page.locator('[data-qa="bot-app-scope-channels:read"]').check();
    await page.locator('[data-qa="bot-app-scope-channels:history"]').check();
    await page.locator('[data-qa="bot-app-create"]').click();

    const shown = page.locator('[data-qa="bot-app-token-value"]');
    await expect(shown).toContainText('cbot-');
    const token = (await shown.textContent())?.trim() ?? '';
    await page.locator('[data-qa="bot-app-token-dismiss"]').click();
    await expect(page.getByText(token)).toHaveCount(0);
    await expect(page.locator('[data-qa="bot-app-row"]').filter({ hasText: name })).toBeVisible();

    const bot = await (
      await admin.get(`${API}/bot-apps/me`, { headers: { Authorization: `Bearer ${token}` } })
    ).json();
    const channel = await createChannel(admin, workspace.id, 'echo');
    const added = await admin.post(`${API}/channels/${channel.id}/members`, {
      data: { user_id: bot.bot_user_id },
    });
    expect(added.status()).toBe(200);
    const asked = await (
      await admin.post(`${API}/channels/${channel.id}/messages`, { data: { content: 'echo ship it' } })
    ).json();

    const output = execFileSync('node', [ECHO_BOT, '--once'], {
      env: { ...process.env, CHAT_API_URL: API, CHAT_BOT_TOKEN: token },
      encoding: 'utf8',
      timeout: 30_000,
    });
    expect(output).toContain('answered 1');

    const thread = await (await admin.get(`${API}/messages/${asked.id}/thread`)).json();
    const reply = thread.data.find((m: { user_id: string }) => m.user_id === bot.bot_user_id);
    expect(reply?.content).toBe('ship it');

    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await page.locator('[data-qa="message-thread-open"]').first().click();
    await expect(page.locator('[data-qa="thread-reply"] [data-qa="app-tag"]')).toBeVisible();
  } finally {
    const apps = await (await admin.get(`${API}/workspaces/${workspace.id}/bot-apps`)).json();
    for (const app of apps.data.filter((a: { name: string }) => a.name === name)) {
      await admin.delete(`${API}/bot-apps/${app.id}`);
    }
    await admin.dispose();
  }
});
