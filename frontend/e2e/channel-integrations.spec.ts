import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace, createChannel } from './helpers';

test('a channel admin adds an incoming webhook from Channel details and a post through it lands in the channel', async ({
  page,
  request,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'ci');

  try {
    const added = await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
    expect(added.status(), 'admin adds bob to the channel').toBe(200);
    const promoted = await admin.patch(`${API}/channels/${channel.id}/members/${bobId}/role`, {
      data: { role: 'admin' },
    });
    expect(promoted.status(), 'admin makes bob a channel admin').toBe(200);

    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.getByTestId('channel-header-name')).toHaveText(channel.name);

    await page.getByTestId('channel-actions-more').click();
    await page.getByTestId('channel-details-open').click();
    await page.getByTestId('channel-details-tab-integrations').click();
    await page.getByTestId('channel-webhook-name').fill('Build bot');
    await page.getByTestId('channel-webhook-add').click();

    const shown = page.getByTestId('channel-webhook-url-value');
    await expect(shown).toContainText('/api/hooks/incoming/');
    await expect(page.getByTestId('channel-integration-incoming')).toContainText('Build bot');
    const token = (await shown.textContent())!.split('/hooks/incoming/')[1];
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('channel-details-modal')).toBeHidden();

    const text = `build ${Date.now()} passed`;
    const posted = await request.post(`${API}/hooks/incoming/${token}`, { data: { text } });
    expect(posted.status(), 'the webhook URL takes a post without a session').toBe(200);
    await expect(page.getByText(text)).toBeVisible();
  } finally {
    const listed = await admin.get(`${API}/channels/${channel.id}/integrations`);
    for (const hook of (await listed.json()).incoming_webhooks ?? []) {
      await admin.delete(`${API}/hooks/${hook.id}`);
    }
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
  }
});
