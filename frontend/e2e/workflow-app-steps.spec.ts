import { test, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { API, createChannel, devWorkspace, login, openWorkspaceMenu, userContext } from './helpers';

const REPO_ROOT = new URL('../..', import.meta.url).pathname;
const TICKETING = `${REPO_ROOT}examples/ticketing-step/ticketing.mjs`;

test.setTimeout(150_000);

test('a workflow opens a ticket through an app’s step and posts its id', async ({ page, playwright }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const caller = await playwright.request.newContext();
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'helpdesk');
  await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
  const stamp = Date.now();
  const name = `Help desk ${stamp}`;
  let app: ReturnType<typeof spawn> | undefined;
  let appId: string | undefined;

  try {
    const created = await admin.post(`${API}/workspaces/${workspace.id}/bot-apps`, {
      data: { name: `Ticketing ${stamp}`, scopes: ['chat:write', 'workflow.steps:execute'] },
    });
    expect(created.status(), await created.text()).toBe(200);
    const made = await created.json();
    appId = made.id as string;
    const configured = await admin.patch(`${API}/bot-apps/${appId}`, {
      data: { socket_mode: true, events: ['function_executed'] },
    });
    expect(configured.status(), await configured.text()).toBe(200);
    const appToken = (await (await admin.post(`${API}/bot-apps/${appId}/app-token`)).json())
      .app_token as string;

    app = spawn('node', [TICKETING], {
      env: { ...process.env, CHAT_API_URL: API, CHAT_BOT_TOKEN: made.token, CHAT_APP_TOKEN: appToken },
    });
    let output = '';
    app.stdout?.on('data', (chunk) => (output += chunk));
    app.stderr?.on('data', (chunk) => (output += chunk));
    await expect.poll(() => output, { timeout: 15_000 }).toContain('offers Open ticket');
    await expect.poll(() => output, { timeout: 15_000 }).toContain('connected as app');

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await openWorkspaceMenu(page);
    await page.getByTestId('open-workflows').click();
    await page.getByTestId('workflow-new').click();
    await page.getByTestId('new-workflow-submit').click();
    const editor = page.getByTestId('workflow-editor');
    await editor.getByTestId('workflow-name').fill(name);
    await editor.getByTestId('workflow-trigger-type').selectOption('webhook');
    await editor.getByTestId('workflow-variable-add').click();
    await editor.getByTestId('workflow-variable-name').fill('subject');

    const steps = editor.getByTestId('workflow-step');
    await steps.nth(0).getByTestId('workflow-step-type').selectOption({ label: 'Open ticket' });
    await expect(steps.nth(0).getByTestId('workflow-step-title')).toHaveText('Open ticket');
    await steps.nth(0).getByTestId('workflow-app-variable-title').selectOption({ label: 'subject' });
    await expect(steps.nth(0).getByTestId('workflow-app-input-title')).toHaveValue('{{webhook.subject}}');
    await expect(steps.nth(0).getByTestId('workflow-app-outputs')).toHaveText(
      'Later steps can use what it hands back: Ticket ID.',
    );

    await editor.getByTestId('workflow-step-add').click();
    await steps.nth(1).getByTestId('workflow-step-channel').selectOption(channel.id);
    await steps.nth(1).getByTestId('workflow-step-text').fill('Ticket  opened');
    await steps.nth(1).getByTestId('workflow-step-text').press('Home');
    for (let i = 0; i < 'Ticket '.length; i += 1) {
      await steps.nth(1).getByTestId('workflow-step-text').press('ArrowRight');
    }
    await steps.nth(1).getByTestId('workflow-step-variable').selectOption({ label: 'Step 1: Ticket ID' });
    await expect(steps.nth(1).getByTestId('workflow-step-text')).toHaveValue(
      'Ticket {{step.1.ticket_id}} opened',
    );

    await editor.getByTestId('workflow-save').click();
    await expect(editor.getByTestId('workflow-disable')).toBeVisible();
    const url = await editor.getByTestId('workflow-webhook-url').innerText();
    await editor.getByRole('button', { name: 'Close' }).last().click();

    const list = page.locator('[data-qa="message-list"]');
    expect((await caller.post(url, { data: { subject: 'Printer on 3 is out' } })).status()).toBe(202);
    await expect(list.getByText(/^Ticket HD-\d+ opened$/)).toBeVisible({ timeout: 30_000 });
    const ticket = /function_executed \S+: (HD-\d+) Printer on 3 is out/.exec(output)?.[1];
    expect(ticket, 'the app opened the ticket').toBeTruthy();
    await expect(list.getByText(`Ticket ${ticket} opened`)).toBeVisible();

    expect((await caller.post(url, { data: { subject: 'The help desk is down' } })).status()).toBe(202);
    await expect.poll(() => output, { timeout: 30_000 }).toContain(': failed');

    await page.getByTestId('workflow-row').filter({ hasText: name }).click();
    await page.getByTestId('workflow-history-toggle').click();
    const runs = page.getByTestId('workflow-run');
    await expect(runs).toHaveCount(2, { timeout: 20_000 });
    const failed = runs.nth(0);
    await expect(failed.getByTestId('workflow-run-status')).toHaveText('Failed', { timeout: 20_000 });
    await expect(failed.getByTestId('workflow-run-step-error')).toHaveText('The help desk is down');
    await expect(failed.getByTestId('workflow-run-step')).toHaveCount(1);
    const done = runs.nth(1);
    await expect(done.getByTestId('workflow-run-status')).toHaveText('Succeeded');
    await expect(done.getByTestId('workflow-run-step').first()).toContainText('Step 1 · Open ticket');
    await expect(done.getByTestId('workflow-run-inputs')).toContainText('title Printer on 3 is out');
    await expect(done.getByTestId('workflow-run-outputs')).toHaveText(`Got back: ticket_id ${ticket}`);
    await expect(list.getByText(/^Ticket HD-\d+ opened$/)).toHaveCount(1);

    app.kill();
    app = undefined;
    const removed = await admin.delete(`${API}/bot-apps/${appId}`);
    expect(removed.status(), await removed.text()).toBe(200);
    appId = undefined;
    await page.getByTestId('workflow-editor').getByRole('button', { name: 'Close' }).last().click();
    await page.getByTestId('workflow-row').filter({ hasText: name }).click();
    await expect(page.getByTestId('workflow-app-warning')).toHaveText(
      'The app that runs this step was removed. Runs fail here until you replace the step.',
      { timeout: 20_000 },
    );
  } finally {
    app?.kill();
    const listed = await (await bob.get(`${API}/workspaces/${workspace.id}/workflows`)).json();
    for (const workflow of listed.data ?? []) {
      if (workflow.name === name) await bob.delete(`${API}/workflows/${workflow.id}`);
    }
    if (appId) await admin.delete(`${API}/bot-apps/${appId}`);
    await admin.delete(`${API}/channels/${channel.id}`);
    await caller.dispose();
    await admin.dispose();
    await bob.dispose();
  }
});
