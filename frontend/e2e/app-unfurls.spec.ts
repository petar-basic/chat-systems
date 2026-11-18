import { test, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { API, createChannel, devWorkspace, login, send, userContext } from './helpers';

const REPO_ROOT = new URL('../..', import.meta.url).pathname;
const TRACKER = `${REPO_ROOT}examples/tracker-previews/tracker.mjs`;

test.setTimeout(120_000);

test('an app previews links to its domain in both browsers, and a removed preview stays removed', async ({
  page,
  browser,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice, userId: aliceId } = await userContext('alice@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const bobContext = await browser.newContext();
  const stamp = Date.now();
  const issue = (number: number) => `https://tracker.example.com/issues/${number}?run=${stamp}`;
  let app: ReturnType<typeof spawn> | undefined;
  let appId: string | undefined;

  try {
    const created = await admin.post(`${API}/workspaces/${workspace.id}/bot-apps`, {
      data: { name: `Tracker ${stamp}`, scopes: ['chat:write', 'links:read', 'links:write'] },
    });
    expect(created.status(), await created.text()).toBe(200);
    const made = await created.json();
    appId = made.id as string;
    const configured = await admin.patch(`${API}/bot-apps/${appId}`, {
      data: { socket_mode: true, events: ['link_shared'] },
    });
    expect(configured.status(), await configured.text()).toBe(200);
    const claimed = await admin.put(`${API}/bot-apps/${appId}/unfurl-domains`, {
      data: { domains: ['tracker.example.com'] },
    });
    expect(claimed.status(), await claimed.text()).toBe(200);
    const appToken = (await (await admin.post(`${API}/bot-apps/${appId}/app-token`)).json())
      .app_token as string;

    const issues = await createChannel(admin, workspace.id, 'issues');
    const quiet = await createChannel(admin, workspace.id, 'quiet');
    for (const channel of [issues, quiet]) {
      for (const userId of [aliceId, bobId]) {
        const invited = await admin.post(`${API}/channels/${channel.id}/members`, {
          data: { user_id: userId },
        });
        expect(invited.status(), await invited.text()).toBe(200);
      }
    }
    const added = await admin.post(`${API}/channels/${issues.id}/members`, {
      data: { user_id: made.bot_user_id },
    });
    expect(added.status(), await added.text()).toBe(200);

    app = spawn('node', [TRACKER], {
      env: { ...process.env, CHAT_API_URL: API, CHAT_BOT_TOKEN: made.token, CHAT_APP_TOKEN: appToken },
    });
    let output = '';
    app.stdout?.on('data', (chunk) => (output += chunk));
    app.stderr?.on('data', (chunk) => (output += chunk));
    await expect.poll(() => output, { timeout: 15_000 }).toContain('connected as app');

    const unheard = await alice.post(`${API}/channels/${quiet.id}/messages`, {
      data: { content: `Where the tracker is not ${issue(5)}` },
    });
    expect(unheard.status(), await unheard.text()).toBe(200);
    const unheardId = (await unheard.json()).id as string;

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${issues.id}`);
    await expect(page.getByTestId('channel-header-name')).toHaveText(issues.name, { timeout: 20_000 });
    const bobPage = await bobContext.newPage();
    await login(bobPage, 'bob@dev.local');
    await bobPage.goto(`/app/${workspace.id}/${issues.id}`);
    await expect(bobPage.getByTestId('channel-header-name')).toHaveText(issues.name, { timeout: 20_000 });

    const text = `TRK-7 is back ${issue(7)}`;
    await send(page, text);
    for (const viewer of [page, bobPage]) {
      const card = viewer
        .locator('[data-qa="message-row"]', { hasText: 'TRK-7 is back' })
        .last()
        .locator('[data-qa="app-blocks-preview"]');
      await expect(card.locator('[data-qa="block-header"]')).toHaveText('TRK-7 Login fails on Safari', {
        timeout: 20_000,
      });
      await expect(card).toContainText('In progress');
      await expect(card).toContainText('Marko');
      await expect(card.locator('[data-qa="app-blocks-preview-app"]')).toHaveText(`Tracker ${stamp}`);
    }
    expect(output, 'the quiet channel never reached the app').not.toContain(unheardId);

    const mine = page.locator('[data-qa="message-row"]', { hasText: 'TRK-7 is back' }).last();
    const messageId = await mine.getAttribute('data-message-id');
    expect(messageId, 'the posted message').toBeTruthy();
    const edited = await alice.patch(`${API}/messages/${messageId}`, {
      data: { content: `${text} and ${issue(8)}` },
    });
    expect(edited.status(), await edited.text()).toBe(200);
    const row = bobPage.locator('[data-qa="message-row"]', { hasText: 'TRK-7 is back' }).last();
    await expect(row.locator('[data-qa="app-blocks-preview"]')).toHaveCount(2, { timeout: 20_000 });
    await expect(row.locator('[data-qa="block-header"]').last()).toHaveText(
      'TRK-8 Search misses new messages',
    );
    await expect.poll(() => output).toContain(`link_shared ${messageId}: ${issue(8)}\n`);

    await mine.hover();
    await mine.locator('[data-qa="app-blocks-preview-remove"]').first().click();
    await expect(row.locator('[data-qa="app-blocks-preview"]')).toHaveCount(0, { timeout: 20_000 });

    const again = await alice.patch(`${API}/messages/${messageId}`, {
      data: { content: `${text} and ${issue(8)} and ${issue(9)}` },
    });
    expect(again.status(), await again.text()).toBe(200);
    await send(page, `Next up ${issue(10)}`);
    const next = bobPage.locator('[data-qa="message-row"]', { hasText: 'Next up' }).last();
    await expect(next.locator('[data-qa="block-header"]')).toHaveText('TRK-10 Search misses new messages', {
      timeout: 20_000,
    });
    await expect(row).toContainText(issue(9));
    await expect(row.locator('[data-qa="app-blocks-preview"]'), 'removed stays removed').toHaveCount(0);
    expect(output, 'the app was not asked again').not.toContain(issue(9));
  } finally {
    app?.kill();
    if (appId) await admin.delete(`${API}/bot-apps/${appId}`);
    await bobContext.close();
    await alice.dispose();
    await bob.dispose();
    await admin.dispose();
  }
});
