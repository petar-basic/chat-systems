import { test, expect } from '@playwright/test';
import { execFileSync, spawn } from 'node:child_process';
import { API, createChannel, devWorkspace, login, openWorkspaceSettings, userContext } from './helpers';

const REPO_ROOT = new URL('../..', import.meta.url).pathname;
const ECHO_BOT = `${REPO_ROOT}examples/echo-bot/echo-bot.mjs`;
const SOCKET_BOT = `${REPO_ROOT}examples/echo-bot/echo-bot-socket.mjs`;

test.setTimeout(60_000);

test('an app made in the settings gets a token that the example bot answers with', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const name = `Echo ${Date.now()}`;

  try {
    await login(page, 'admin@dev.local');
    await openWorkspaceSettings(page, 'apps');
    await page.locator('[data-qa="app-directory-build-open"]').click();
    const form = page.locator('[data-qa="bot-app-form"]');
    await form.getByLabel('App name').fill(name);
    await page.locator('[data-qa="bot-app-scope-channels:read"]').check();
    await page.locator('[data-qa="bot-app-scope-channels:history"]').check();
    await page.locator('[data-qa="bot-app-create"]').click();
    await expect(page.locator('[data-qa="app-consent-permissions"]')).toContainText(
      'Read the public channels it is in',
    );
    await page.locator('[data-qa="app-consent-confirm"]').click();

    const shown = page.locator('[data-qa="bot-app-token-value"]');
    await expect(shown).toContainText('cbot-');
    const token = (await shown.textContent())?.trim() ?? '';
    await page.locator('[data-qa="bot-app-token-dismiss"]').click();
    await expect(page.getByText(token)).toHaveCount(0);
    await expect(page.locator('[data-qa="app-directory-row"]').filter({ hasText: name })).toBeVisible();

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

test('an app in socket mode answers a mention with no public URL', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const name = `Socket ${Date.now()}`;
  let bot: ReturnType<typeof spawn> | undefined;

  try {
    await login(page, 'admin@dev.local');
    await openWorkspaceSettings(page, 'apps');
    await page.locator('[data-qa="app-directory-build-open"]').click();
    const form = page.locator('[data-qa="bot-app-form"]');
    await form.getByLabel('App name').fill(name);
    await page.locator('[data-qa="bot-app-scope-channels:history"]').check();
    await page.locator('[data-qa="bot-app-create"]').click();
    await page.locator('[data-qa="app-consent-confirm"]').click();
    const shownToken = page.locator('[data-qa="bot-app-token-value"]');
    await expect(shownToken).toContainText('cbot-');
    const token = ((await shownToken.textContent()) ?? '').trim();
    await page.locator('[data-qa="bot-app-token-dismiss"]').click();

    await page.locator('[data-qa="app-directory-row"]').filter({ hasText: name }).click();
    const row = page.locator('[data-qa="app-page"]');
    await row.locator('[data-qa="bot-app-event-app_mention"]').check();
    await row.locator('[data-qa="bot-app-socket-mode"]').check();
    await row.locator('[data-qa="bot-app-events-save"]').click();
    await row.locator('[data-qa="bot-app-app-token-make"]').click();
    const shown = row.locator('[data-qa="bot-app-app-token-value"]');
    await expect(shown).toContainText('capp-');
    const appToken = ((await shown.textContent()) ?? '').trim();

    const me = await (
      await admin.get(`${API}/bot-apps/me`, { headers: { Authorization: `Bearer ${token}` } })
    ).json();
    const channel = await createChannel(admin, workspace.id, 'socket');
    const added = await admin.post(`${API}/channels/${channel.id}/members`, {
      data: { user_id: me.bot_user_id },
    });
    expect(added.status()).toBe(200);

    bot = spawn('node', [SOCKET_BOT, '--once'], {
      env: { ...process.env, CHAT_API_URL: API, CHAT_BOT_TOKEN: token, CHAT_APP_TOKEN: appToken },
    });
    let output = '';
    bot.stdout?.on('data', (chunk) => (output += chunk));
    bot.stderr?.on('data', (chunk) => (output += chunk));
    const exited = new Promise<number | null>((resolve) => bot?.on('exit', resolve));
    await expect.poll(() => output, { timeout: 15_000 }).toContain('connected as app');

    const asked = await (
      await admin.post(`${API}/channels/${channel.id}/messages`, {
        data: { content: `@[${name}](${me.bot_user_id}) ship it` },
      })
    ).json();

    expect(await exited).toBe(0);
    expect(output).toContain('answered 1');
    const thread = await (await admin.get(`${API}/messages/${asked.id}/thread`)).json();
    const reply = thread.data.find((m: { user_id: string }) => m.user_id === me.bot_user_id);
    expect(reply?.content).toBe('ship it');
  } finally {
    bot?.kill();
    const apps = await (await admin.get(`${API}/workspaces/${workspace.id}/bot-apps`)).json();
    for (const app of apps.data.filter((a: { name: string }) => a.name === name)) {
      await admin.delete(`${API}/bot-apps/${app.id}`);
    }
    await admin.dispose();
  }
});
