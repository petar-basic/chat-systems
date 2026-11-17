import { test, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { API, createChannel, devWorkspace, login, openWorkspaceSettings, send, userContext } from './helpers';

const REPO_ROOT = new URL('../..', import.meta.url).pathname;
const DEPLOY_APP = `${REPO_ROOT}examples/deploy-approval/deploy-approval.mjs`;

test.setTimeout(90_000);

test('a command opens the app’s modal, and its message’s buttons reach the app over a socket', async ({
  page,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const name = `Deployer ${Date.now()}`;
  let app: ReturnType<typeof spawn> | undefined;

  try {
    await login(page, 'admin@dev.local');
    await openWorkspaceSettings(page, 'apps');
    await page.locator('[data-qa="app-directory-build-open"]').click();
    const form = page.locator('[data-qa="bot-app-form"]');
    await form.getByLabel('App name').fill(name);
    await page.locator('[data-qa="bot-app-scope-commands"]').check();
    await page.locator('[data-qa="bot-app-create"]').click();
    await page.locator('[data-qa="app-consent-confirm"]').click();
    const shownToken = page.locator('[data-qa="bot-app-token-value"]');
    await expect(shownToken).toContainText('cbot-');
    const token = ((await shownToken.textContent()) ?? '').trim();
    await page.locator('[data-qa="bot-app-token-dismiss"]').click();

    await page.locator('[data-qa="app-directory-row"]').filter({ hasText: name }).click();
    const row = page.locator('[data-qa="app-page"]');
    await row.locator('[data-qa="bot-app-socket-mode"]').check();
    await row.locator('[data-qa="bot-app-interactivity"]').check();
    await row.locator('[data-qa="bot-app-events-save"]').click();
    await row.locator('[data-qa="bot-app-app-token-make"]').click();
    const shown = row.locator('[data-qa="bot-app-app-token-value"]');
    await expect(shown).toContainText('capp-');
    const appToken = ((await shown.textContent()) ?? '').trim();

    const me = await (
      await admin.get(`${API}/bot-apps/me`, { headers: { Authorization: `Bearer ${token}` } })
    ).json();
    const channel = await createChannel(admin, workspace.id, 'deploys');
    const command = await admin.post(`${API}/workspaces/${workspace.id}/hooks`, {
      data: {
        hook_type: 'slash_command',
        name: 'request',
        app_id: me.id,
        config: { command: 'request', channel_ids: [channel.id] },
      },
    });
    expect(command.status(), await command.text()).toBe(200);
    const added = await admin.post(`${API}/channels/${channel.id}/members`, {
      data: { user_id: me.bot_user_id },
    });
    expect(added.status()).toBe(200);

    app = spawn('node', [DEPLOY_APP, '--once'], {
      env: { ...process.env, CHAT_API_URL: API, CHAT_BOT_TOKEN: token, CHAT_APP_TOKEN: appToken },
    });
    let output = '';
    app.stdout?.on('data', (chunk) => (output += chunk));
    app.stderr?.on('data', (chunk) => (output += chunk));
    const exited = new Promise<number | null>((resolve) => app?.on('exit', resolve));
    await expect.poll(() => output, { timeout: 15_000 }).toContain('connected as app');

    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.locator('[data-qa="message-list"]')).toBeVisible({ timeout: 20_000 });
    await send(page, '/request');

    const modal = page.locator('[data-qa="app-view"]');
    await expect(modal.getByRole('heading', { name: 'Request a deploy' })).toBeVisible();
    await expect(page.locator('[data-qa="command-response"]')).toHaveCount(0);

    await modal.getByRole('button', { name: 'Request' }).click();
    await expect(modal.getByText('This field is required')).toBeVisible();

    const service = modal.getByLabel('Service');
    await service.fill('The API');
    await modal.getByRole('button', { name: 'Request' }).click();
    await expect(modal.getByText('Use the short name: lower-case letters, digits and dashes')).toBeVisible();

    await modal.getByRole('button', { name: 'Add release notes' }).click();
    await expect(modal.getByRole('heading', { name: 'Release notes' })).toBeVisible();
    await modal.getByLabel('What changes').fill('Faster search');
    await modal.getByRole('button', { name: 'Keep' }).click();
    await expect(modal.getByRole('heading', { name: 'Request a deploy' })).toBeVisible();
    await expect(modal.getByLabel('Service'), 'the view below kept what was typed').toHaveValue('The API');

    await modal.getByLabel('Service').fill('api');
    await modal.getByText('Production').click();
    await modal.getByRole('button', { name: 'Request' }).click();
    await expect(modal).toHaveCount(0);

    const fromApp = async () => {
      const messages = (await (await admin.get(`${API}/channels/${channel.id}/messages`)).json()).data;
      return messages.find((m: { user_id: string }) => m.user_id === me.bot_user_id)?.id as
        string | undefined;
    };
    await expect.poll(fromApp, { timeout: 10_000 }).toBeTruthy();
    const asked = page.locator(`[data-message-id="${await fromApp()}"]`);
    await expect(asked).toContainText('asks to deploy');
    await expect(asked).toContainText('api');
    await expect(asked).toContainText('production');
    await expect(asked).toContainText('Faster search');
    await asked.getByRole('button', { name: 'Approve' }).click();
    await expect(asked).toContainText(/Approved by \S/);
    await expect(asked.getByRole('button', { name: 'Approve' })).toHaveCount(0);
    expect(await exited).toBe(0);
    expect(output).toContain('decided 1');

    const again = await admin.post(`${API}/channels/${channel.id}/messages`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        content: 'Roll back web?',
        blocks: [
          {
            type: 'actions',
            block_id: 'decision',
            elements: [
              { type: 'button', action_id: 'approve', text: { type: 'plain_text', text: 'Roll back' } },
            ],
          },
        ],
      },
    });
    expect(again.status(), await again.text()).toBe(200);
    const nobody = page.locator(`[data-message-id="${(await again.json()).id}"]`);
    await nobody.getByRole('button', { name: 'Roll back' }).click();
    await expect(nobody.locator('[data-qa="block-failure"]')).toHaveText('The app did not respond');
  } finally {
    app?.kill();
    const apps = await (await admin.get(`${API}/workspaces/${workspace.id}/bot-apps`)).json();
    for (const each of apps.data.filter((a: { name: string }) => a.name === name)) {
      await admin.delete(`${API}/bot-apps/${each.id}`);
    }
    await admin.dispose();
  }
});
