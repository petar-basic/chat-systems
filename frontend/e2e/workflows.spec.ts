import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test('a member builds a webhook workflow and a call to it posts in the channel', async ({
  page,
  playwright,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const caller = await playwright.request.newContext();
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'alerts');
  await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
  const name = `Alert relay ${Date.now()}`;

  try {
    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await page.getByRole('button', { name: workspace.name }).first().click();
    await page.getByTestId('open-workflows').click();
    await expect(page.getByTestId('workflows-panel')).toBeVisible();

    await page.getByTestId('workflow-new').click();
    const editor = page.getByTestId('workflow-editor');
    await editor.getByTestId('workflow-name').fill(name);
    await editor.getByTestId('workflow-trigger-type').selectOption('webhook');
    await editor.getByTestId('workflow-variable-add').click();
    await editor.getByTestId('workflow-variable-name').fill('service');
    await editor.getByTestId('workflow-step-channel').selectOption(channel.id);
    await editor.getByTestId('workflow-step-text').fill(' is down');
    await editor.getByTestId('workflow-step-text').press('Home');
    await editor.getByTestId('workflow-step-variable').selectOption({ label: 'service' });
    await expect(editor.getByTestId('workflow-step-text')).toHaveValue('{{webhook.service}} is down');

    await editor.getByTestId('workflow-publish').click();
    await expect(editor.getByTestId('workflow-disable')).toBeVisible();
    const url = await editor.getByTestId('workflow-webhook-url').innerText();
    expect(url).toContain('/api/workflows/webhook/');

    const called = await caller.post(url, { data: { service: 'billing' } });
    expect(called.status(), 'the workflow accepts the call').toBe(202);

    await editor.getByRole('button', { name: 'Close' }).last().click();
    const posted = page.locator('[data-qa="message-list"]').getByText('billing is down');
    await expect(posted).toBeVisible({ timeout: 30_000 });
    await expect(
      page.locator('[data-qa="message-list"] [data-qa="bot-tag"]').filter({ hasText: 'Workflow' }).first(),
    ).toBeVisible();

    await page.getByTestId('workflow-row').filter({ hasText: name }).click();
    await page.getByTestId('workflow-history-toggle').click();
    await expect(page.getByTestId('workflow-run-status').first()).toHaveText('Succeeded');
    await expect(page.getByTestId('workflow-run-step').first()).toHaveText('Step 1 · Send a message');
  } finally {
    const listed = await (await bob.get(`${API}/workspaces/${workspace.id}/workflows`)).json();
    for (const workflow of listed.data ?? []) {
      if (workflow.name === name) await bob.delete(`${API}/workflows/${workflow.id}`);
    }
    await admin.delete(`${API}/channels/${channel.id}`);
    await caller.dispose();
    await admin.dispose();
    await bob.dispose();
  }
});

test('a member-joined workflow welcomes the newcomer with a DM', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const { ctx: alice, userId: aliceId } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'onboarding');
  await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
  const name = `Welcome ${Date.now()}`;
  const greeting = `Welcome aboard ${Date.now()}`;

  try {
    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await page.getByRole('button', { name: workspace.name }).first().click();
    await page.getByTestId('open-workflows').click();
    await page.getByTestId('workflow-new').click();

    const editor = page.getByTestId('workflow-editor');
    await editor.getByTestId('workflow-name').fill(name);
    await editor.getByTestId('workflow-trigger-type').selectOption('member_joined');
    await editor.getByTestId('workflow-trigger-channel').selectOption(channel.id);
    await editor.getByTestId('workflow-step-type').selectOption('send_dm');
    await editor.getByTestId('workflow-step-person').selectOption('trigger_user');
    await editor.getByTestId('workflow-step-text').fill(`${greeting} in {{trigger.channel}}`);
    await editor.getByTestId('workflow-publish').click();
    await expect(editor.getByTestId('workflow-disable')).toBeVisible();

    const added = await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: aliceId } });
    expect(added.status(), 'alice joins the channel').toBe(200);

    await expect
      .poll(
        async () => {
          const listed = await (await alice.get(`${API}/workspaces/${workspace.id}/conversations`)).json();
          const direct = (listed.data ?? []).find(
            (c: { kind: string; participant_ids: string[] }) =>
              c.kind === 'direct' && c.participant_ids.includes(bobId),
          );
          if (!direct) return '';
          return JSON.stringify(await (await alice.get(`${API}/channels/${direct.id}/messages`)).json());
        },
        { timeout: 30_000 },
      )
      .toContain(`${greeting} in #${channel.name}`);

    await editor.getByTestId('workflow-history-toggle').click();
    await expect(editor.getByTestId('workflow-run-status').first()).toHaveText('Succeeded');
    await expect(editor.getByTestId('workflow-run-step').first()).toHaveText('Step 1 · Send a DM');
  } finally {
    const listed = await (await bob.get(`${API}/workspaces/${workspace.id}/workflows`)).json();
    for (const workflow of listed.data ?? []) {
      if (workflow.name === name) await bob.delete(`${API}/workflows/${workflow.id}`);
    }
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
    await alice.dispose();
  }
});
