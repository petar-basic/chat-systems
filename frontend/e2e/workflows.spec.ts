import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext, openWorkspaceMenu } from './helpers';

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
    await openWorkspaceMenu(page);
    await page.getByTestId('open-workflows').click();
    await expect(page.getByTestId('workflows-panel')).toBeVisible();

    await page.getByTestId('workflow-new').click();
    await page.getByTestId('new-workflow-submit').click();
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

    await editor.getByTestId('workflow-save').click();
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
    await openWorkspaceMenu(page);
    await page.getByTestId('open-workflows').click();
    await page.getByTestId('workflow-new').click();
    await page.getByTestId('new-workflow-submit').click();

    const editor = page.getByTestId('workflow-editor');
    await editor.getByTestId('workflow-name').fill(name);
    await editor.getByTestId('workflow-trigger-type').selectOption('member_joined');
    await editor.getByTestId('workflow-trigger-channel').selectOption(channel.id);
    await editor.getByTestId('workflow-step-type').selectOption('send_dm');
    await editor.getByTestId('workflow-step-person').selectOption('trigger_user');
    await editor.getByTestId('workflow-step-text').fill(`${greeting} in {{trigger.channel}}`);
    await editor.getByTestId('workflow-save').click();
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

test('a shortcut asks the person who ran it a form and posts the answers', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice, userId: aliceId } = await userContext('alice@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'requests');
  for (const user_id of [aliceId, bobId]) {
    await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id } });
  }
  const name = `Time off ${Date.now()}`;
  let workflowId = '';

  try {
    const created = await alice.post(`${API}/workspaces/${workspace.id}/workflows`, {
      data: {
        name,
        definition: {
          trigger: { type: 'shortcut', channel_id: channel.id },
          steps: [
            {
              type: 'form',
              to: { kind: 'trigger_user' },
              title: 'Request time off',
              fields: [
                { name: 'reason', label: 'Reason', kind: 'short_text', required: true },
                { name: 'kind', label: 'Kind', kind: 'select', options: ['Sick', 'Holiday'], required: true },
              ],
            },
            {
              type: 'send_message',
              channel_id: channel.id,
              text: '{{trigger.user}} asked for time off',
              summary: true,
            },
          ],
        },
      },
    });
    expect(created.status(), 'alice builds the workflow').toBe(200);
    workflowId = (await created.json()).id;

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await page.getByTestId('composer-workflows').click();
    await page.getByTestId('composer-workflow').filter({ hasText: name }).click();

    let conversationId = '';
    await expect
      .poll(
        async () => {
          const listed = await (await bob.get(`${API}/workspaces/${workspace.id}/conversations`)).json();
          const direct = (listed.data ?? []).find(
            (c: { kind: string; participant_ids: string[] }) =>
              c.kind === 'direct' && c.participant_ids.includes(aliceId),
          );
          conversationId = direct?.id ?? '';
          if (!direct) return '';
          return JSON.stringify(await (await bob.get(`${API}/channels/${direct.id}/messages`)).json());
        },
        { timeout: 30_000 },
      )
      .toContain('Request time off');

    await page.goto(`/app/${workspace.id}/c/${conversationId}`);
    const card = page.getByTestId('workflow-form-card').filter({ hasText: 'Request time off' }).last();
    await card.getByTestId('workflow-form-fill').click();
    const dialog = page.getByTestId('workflow-form-dialog');
    const answers = dialog.getByTestId('workflow-form-answer');
    await answers.nth(0).fill('Dentist');
    await answers.nth(1).selectOption('Sick');
    await dialog.getByTestId('workflow-form-submit').click();
    await expect(dialog).toBeHidden();
    await expect(card.getByTestId('workflow-form-card-status')).toContainText('Filled in');

    await page.goto(`/app/${workspace.id}/${channel.id}`);
    const posted = page.locator('[data-qa="message-list"]').getByText('Dentist');
    await expect(posted).toBeVisible({ timeout: 30_000 });
    await expect(page.locator('[data-qa="message-list"]').getByText('asked for time off')).toBeVisible();
  } finally {
    if (workflowId) await alice.delete(`${API}/workflows/${workflowId}`);
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await alice.dispose();
    await bob.dispose();
  }
});

test('the time-off template adds a request to a list with who asked', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice, userId: aliceId } = await userContext('alice@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'timeoff');
  for (const user_id of [aliceId, bobId]) {
    await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id } });
  }
  const created = await admin.post(`${API}/workspaces/${workspace.id}/lists`, {
    data: {
      title: `Time off ${Date.now()}`,
      channel_id: channel.id,
      columns: [
        {
          name: 'Kind',
          kind: 'select',
          options: { choices: [{ label: 'Holiday' }, { label: 'Sick' }, { label: 'Other' }] },
        },
        { name: 'From', kind: 'date' },
        { name: 'Until', kind: 'date' },
        { name: 'Who', kind: 'person' },
      ],
    },
  });
  expect(created.ok(), 'the list exists').toBeTruthy();
  const detail = await created.json();
  const listId: string = detail.list.id;
  type Column = { id: string; name: string; options: { choices?: { id: string; label: string }[] } };
  const columns: Column[] = detail.columns;
  const columnId = (name: string) => columns.find((c) => c.name === name)!.id;
  const sick = columns.find((c) => c.name === 'Kind')!.options.choices!.find((c) => c.label === 'Sick')!.id;
  const name = `Request time off ${Date.now()}`;

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await openWorkspaceMenu(page);
    await page.getByTestId('open-workflows').click();
    await page.getByTestId('workflow-new').click();
    await page.getByTestId('new-workflow-template-time_off').click();
    await page.getByTestId('new-workflow-submit').click();

    const editor = page.getByTestId('workflow-editor');
    await editor.getByTestId('workflow-name').fill(name);
    await editor.getByTestId('workflow-trigger-channel').selectOption(channel.id);
    await editor.getByTestId('workflow-step-list').selectOption(listId);
    await editor.getByTestId('workflow-step-match').click();
    const who = editor
      .locator('[data-qa="workflow-cell"][data-column="Who"]')
      .getByTestId('workflow-cell-value');
    await who.selectOption('{{trigger.user}}');
    await editor.getByTestId('workflow-step-channel').selectOption(channel.id);
    await editor.getByTestId('workflow-save').click();
    await expect(editor.getByTestId('workflow-disable')).toBeVisible();

    const listed = await (await alice.get(`${API}/workspaces/${workspace.id}/workflows`)).json();
    const workflow = (listed.data ?? []).find((w: { name: string }) => w.name === name);
    const started = await bob.post(`${API}/workflows/${workflow.id}/start`);
    expect(started.status(), 'bob runs the shortcut').toBe(202);
    const { run_id } = await started.json();

    await expect
      .poll(
        async () =>
          (
            await bob.post(`${API}/workflow-runs/${run_id}/forms/0`, {
              data: { answers: { kind: 'Sick', from: '2026-11-02', until: '2026-11-03', note: '' } },
            })
          ).status(),
        { timeout: 30_000 },
      )
      .toBe(200);

    await expect
      .poll(
        async () => {
          const items = await (await alice.get(`${API}/lists/${listId}/items`)).json();
          return (items.data ?? []).map((item: { values: Record<string, unknown> }) => item.values);
        },
        { timeout: 30_000 },
      )
      .toContainEqual({
        [columnId('Kind')]: sick,
        [columnId('From')]: '2026-11-02',
        [columnId('Until')]: '2026-11-03',
        [columnId('Who')]: bobId,
      });
  } finally {
    const listed = await (await alice.get(`${API}/workspaces/${workspace.id}/workflows`)).json();
    for (const workflow of listed.data ?? []) {
      if (workflow.name === name) await alice.delete(`${API}/workflows/${workflow.id}`);
    }
    await admin.delete(`${API}/lists/${listId}`);
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await alice.dispose();
    await bob.dispose();
  }
});
