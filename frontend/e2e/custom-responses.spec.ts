import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, openWorkspaceSettings, userContext } from './helpers';

test('a custom response added in settings is answered by the workspace bot in channels and threads', async ({
  page,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'responses');
  const other = await createChannel(admin, workspace.id, 'responses-thread');
  const stamp = Date.now().toString(36);
  const phrase = `pantry code ${stamp}`;
  const answer = `The pantry code is on the door (${stamp})`;
  let responseId: string | undefined;

  try {
    for (const ch of [channel, other]) {
      expect((await bob.post(`${API}/channels/${ch.id}/join`)).status()).toBe(200);
    }

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await openWorkspaceSettings(page, 'custom-responses');
    await page.getByTestId('custom-responses-add').click();
    await page.getByTestId('custom-response-trigger-text').first().fill(phrase);
    await page.getByTestId('custom-response-answer-text').first().fill(answer);
    await page.getByTestId('custom-response-save').click();
    const row = page.getByTestId('custom-responses-row').filter({ hasText: phrase });
    await expect(row).toBeVisible();

    const listed = await (await admin.get(`${API}/workspaces/${workspace.id}/custom-responses`)).json();
    responseId = listed.data.find((r: { triggers: string[] }) => r.triggers.includes(phrase)).id;
    const botId: string = listed.bot.user_id;
    await page.keyboard.press('Escape');

    const asked = await bob.post(`${API}/channels/${channel.id}/messages`, {
      data: { content: `What's the Pantry Code ${stamp}?` },
    });
    expect(asked.status()).toBe(200);
    const reply = page.locator('[data-message-id]').filter({ hasText: answer });
    await expect(reply).toBeVisible({ timeout: 20_000 });
    await expect(reply.locator('[data-qa="app-tag"]')).toBeVisible();

    const root = await (
      await bob.post(`${API}/channels/${other.id}/messages`, { data: { content: 'Kitchen questions' } })
    ).json();
    const inThread = await bob.post(`${API}/channels/${other.id}/messages`, {
      data: { content: `anyone know the pantry code ${stamp}`, thread_parent_id: root.id },
    });
    expect(inThread.status()).toBe(200);
    await expect
      .poll(
        async () => {
          const thread = await (await admin.get(`${API}/messages/${root.id}/thread`)).json();
          return thread.data.find((m: { user_id: string }) => m.user_id === botId)?.content;
        },
        { timeout: 20_000 },
      )
      .toBe(answer);
  } finally {
    if (responseId) await admin.delete(`${API}/workspaces/${workspace.id}/custom-responses/${responseId}`);
    await bob.dispose();
    await admin.dispose();
  }
});
