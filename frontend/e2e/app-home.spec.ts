import { test, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { API, createChannel, devWorkspace, login, send, userContext } from './helpers';

const REPO_ROOT = new URL('../..', import.meta.url).pathname;
const HELPDESK = `${REPO_ROOT}examples/helpdesk/helpdesk.mjs`;

test.setTimeout(120_000);

test('an app’s Home and both kinds of shortcut reach the app, and go away when it turns them off', async ({
  page,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice, userId: aliceId } = await userContext('alice@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const name = `Helpdesk ${Date.now()}`;
  let app: ReturnType<typeof spawn> | undefined;
  let appId: string | undefined;

  try {
    const created = await admin.post(`${API}/workspaces/${workspace.id}/bot-apps`, {
      data: { name, description: 'Ask the help desk for anything', scopes: ['chat:write'] },
    });
    expect(created.status(), await created.text()).toBe(200);
    const made = await created.json();
    appId = made.id as string;
    const token = made.token as string;
    const botUserId = made.bot_user_id as string;

    const configured = await admin.patch(`${API}/bot-apps/${appId}`, {
      data: { socket_mode: true, interactivity: true, home_enabled: true, events: ['app_home_opened'] },
    });
    expect(configured.status(), await configured.text()).toBe(200);
    const appToken = (await (await admin.post(`${API}/bot-apps/${appId}/app-token`)).json())
      .app_token as string;
    const shortcuts = await admin.put(`${API}/bot-apps/${appId}/shortcuts`, {
      data: {
        shortcuts: [
          {
            kind: 'global',
            name: 'New request',
            description: 'Ask the help desk for something',
            callback_id: 'new_request',
          },
          {
            kind: 'message',
            name: 'Create ticket from message',
            description: 'Turn this message into a ticket',
            callback_id: 'ticket_from_message',
          },
        ],
      },
    });
    expect(shortcuts.status(), await shortcuts.text()).toBe(200);

    const desk = await createChannel(admin, workspace.id, 'desk');
    const quiet = await createChannel(admin, workspace.id, 'quiet');
    for (const channel of [desk, quiet]) {
      const invited = await admin.post(`${API}/channels/${channel.id}/members`, {
        data: { user_id: aliceId },
      });
      expect(invited.status(), await invited.text()).toBe(200);
    }
    const added = await admin.post(`${API}/channels/${desk.id}/members`, { data: { user_id: botUserId } });
    expect(added.status(), await added.text()).toBe(200);

    const dm = await alice.post(`${API}/workspaces/${workspace.id}/conversations`, {
      data: { participant_ids: [botUserId] },
    });
    expect(dm.status(), await dm.text()).toBe(200);
    const conversation = await dm.json();

    app = spawn('node', [HELPDESK, '--once'], {
      env: { ...process.env, CHAT_API_URL: API, CHAT_BOT_TOKEN: token, CHAT_APP_TOKEN: appToken },
    });
    let output = '';
    app.stdout?.on('data', (chunk) => (output += chunk));
    app.stderr?.on('data', (chunk) => (output += chunk));
    const exited = new Promise<number | null>((resolve) => app?.on('exit', resolve));
    await expect.poll(() => output, { timeout: 15_000 }).toContain('connected as app');

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/c/${conversation.id}`);
    const tabs = page.locator('[data-qa="app-tabs"]');
    await expect(tabs.locator('[data-qa="app-tab-home"]')).toHaveAttribute('aria-selected', 'true', {
      timeout: 20_000,
    });
    const home = page.locator('[data-qa="app-home"]');
    await expect(home.getByRole('heading', { name: 'Your requests' })).toBeVisible();
    await expect(home).toContainText('Nothing asked yet.');
    await expect(page.locator('[data-qa="composer-send"]')).toHaveCount(0);

    const modal = page.locator('[data-qa="app-view"]');
    await home.getByRole('button', { name: 'New request' }).click();
    await expect(modal.getByRole('heading', { name: 'New request' })).toBeVisible();
    await modal.getByLabel('What do you need?').fill('A new laptop');
    await modal.locator('[data-qa="app-view-submit"]').click();
    await expect(modal).toHaveCount(0);
    await expect(home, 'the Home the app republished shows without a reload').toContainText(
      '#1 A new laptop · Open',
    );

    const bobsDm = await (
      await bob.post(`${API}/workspaces/${workspace.id}/conversations`, {
        data: { participant_ids: [botUserId] },
      })
    ).json();
    const opened = await bob.post(`${API}/apps/${appId}/home/opened`, { data: { channel_id: bobsDm.id } });
    expect(opened.status(), await opened.text()).toBe(200);
    const bobsHome = async () =>
      JSON.stringify((await (await bob.get(`${API}/apps/${appId}/home`)).json()).home?.blocks ?? null);
    await expect.poll(bobsHome, { timeout: 10_000 }).toContain('Nothing asked yet.');
    expect(await bobsHome(), 'Bob reads only his own Home').not.toContain('A new laptop');

    await tabs.locator('[data-qa="app-tab-messages"]').click();
    await expect(page.locator('[data-qa="app-home"]')).toHaveCount(0);
    await expect(page.locator('[data-qa="composer-send"]')).toBeVisible();

    await page.goto(`/app/${workspace.id}/${desk.id}`);
    await expect(page.getByTestId('channel-header-name')).toHaveText(desk.name, { timeout: 20_000 });
    await page.locator('[data-qa="composer-workflows"]').click();
    await page
      .locator('[data-qa="composer-workflows-menu"] [data-qa="composer-app-shortcut"]')
      .filter({ hasText: 'New request' })
      .click();
    await expect(modal.getByRole('heading', { name: 'New request' })).toBeVisible();
    await modal.getByLabel('What do you need?').fill('A second monitor');
    await modal.locator('[data-qa="app-view-submit"]').click();
    await expect(modal).toHaveCount(0);

    const jammed = `The printer on floor 2 is jammed ${Date.now()}`;
    await send(page, jammed);
    const row = page.locator('[data-qa="message-row"]', { hasText: jammed }).last();
    await expect(row).toBeVisible();
    await row.hover();
    await row.locator('[data-qa="message-action-more"]').click();
    await page
      .locator('[data-qa="message-overflow"] [data-qa="message-action-shortcut"]')
      .filter({ hasText: 'Create ticket from message' })
      .click();
    await expect(modal.getByRole('heading', { name: 'Create a ticket' })).toBeVisible();
    await expect(modal.getByLabel('What do you need?'), 'the app got the message').toHaveValue(jammed);
    await modal.locator('[data-qa="app-view-submit"]').click();
    await expect(modal).toHaveCount(0);
    await expect(page.locator('[data-qa="message-row"]', { hasText: 'Ticket #3 opened by' })).toContainText(
      jammed,
    );
    expect(await exited).toBe(0);
    expect(output).toContain('opened 3');

    await page.keyboard.press('ControlOrMeta+k');
    const switcher = page.locator('[data-qa="quick-switcher"]');
    await switcher.getByPlaceholder('Jump to a channel or person…').fill('New requ');
    await expect(switcher.getByRole('button').filter({ hasText: 'New request' })).toContainText(name);
    await page.keyboard.press('Escape');

    await page.goto(`/app/${workspace.id}/${quiet.id}`);
    await expect(page.getByTestId('channel-header-name')).toHaveText(quiet.name, { timeout: 20_000 });
    const elsewhere = `Nobody here asked the help desk ${Date.now()}`;
    await send(page, elsewhere);
    const quietRow = page.locator('[data-qa="message-row"]', { hasText: elsewhere }).last();
    await quietRow.hover();
    await quietRow.locator('[data-qa="message-action-more"]').click();
    await expect(page.locator('[data-qa="message-overflow"]')).toBeVisible();
    await expect(page.locator('[data-qa="message-shortcuts"]'), 'the app is not in this channel').toHaveCount(
      0,
    );
    await page.keyboard.press('Escape');

    expect(await bobsHome()).not.toContain('A second monitor');
    const alicesHome = await (await alice.get(`${API}/apps/${appId}/home`)).json();
    expect(JSON.stringify(alicesHome.home.blocks)).toContain('A second monitor');

    await page.goto(`/app/${workspace.id}/${desk.id}`);
    const deskRow = page.locator('[data-qa="message-row"]', { hasText: jammed }).last();
    await expect(deskRow).toBeVisible({ timeout: 20_000 });
    await deskRow.hover();
    await deskRow.locator('[data-qa="message-action-more"]').click();
    await expect(page.locator('[data-qa="message-shortcuts"]')).toBeVisible();
    const off = await admin.patch(`${API}/bot-apps/${appId}`, { data: { interactivity: false } });
    expect(off.status(), await off.text()).toBe(200);
    await expect(
      page.locator('[data-qa="message-shortcuts"]'),
      'the shortcuts left without a reload',
    ).toHaveCount(0);
    await page.keyboard.press('Escape');

    await page.goto(`/app/${workspace.id}/c/${conversation.id}`);
    await expect(page.locator('[data-qa="app-home"]')).toContainText('#2 A second monitor', {
      timeout: 20_000,
    });
    const homeOff = await admin.patch(`${API}/bot-apps/${appId}`, { data: { home_enabled: false } });
    expect(homeOff.status(), await homeOff.text()).toBe(200);
    await expect(page.locator('[data-qa="app-tabs"]'), 'the Home tab left without a reload').toHaveCount(0);
    await expect(page.locator('[data-qa="composer-send"]')).toBeVisible();
  } finally {
    app?.kill();
    if (appId) await admin.delete(`${API}/bot-apps/${appId}`);
    await alice.dispose();
    await bob.dispose();
    await admin.dispose();
  }
});
