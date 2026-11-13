import { test, expect, type Page } from '@playwright/test';
import { API, login, userContext, devWorkspace, createChannel } from './helpers';

async function openSettings(page: Page) {
  await page.getByTestId('channel-actions-more').click();
  await page.getByTestId('channel-details-open').click();
  await page.getByTestId('channel-details-tab-settings').click();
}

test('a channel made private leaves the open tab of someone outside it, and opens again', async ({
  page,
  browser,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'hiring');
  const watcherContext = await browser.newContext();
  const watcher = await watcherContext.newPage();

  try {
    const posted = await admin.post(`${API}/channels/${channel.id}/messages`, {
      data: { content: 'the shortlist' },
    });
    expect(posted.status()).toBe(200);

    await login(watcher, 'bob@dev.local');
    await watcher.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(watcher.getByTestId('channel-header-name')).toHaveText(channel.name);
    await expect(watcher.getByText('the shortlist')).toBeVisible();

    const notHis = await bob.put(`${API}/channels/${channel.id}/privacy`, { data: { private: true } });
    expect(notHis.status(), 'a member who is not an admin cannot').toBe(403);

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.getByTestId('channel-header-name')).toHaveText(channel.name);
    await openSettings(page);
    await expect(page.getByTestId('channel-settings-privacy')).toContainText('Public');
    await page.getByTestId('channel-settings-privacy-change').click();
    await page.getByTestId('channel-privacy-private-dialog-confirm').click();
    await expect(page.getByTestId('channel-settings-privacy')).toContainText('Private');

    await expect(watcher, 'the outsider is moved off the channel').not.toHaveURL(new RegExp(channel.id));
    await expect(watcher.getByText('the shortlist')).toHaveCount(0);
    const refused = await bob.get(`${API}/channels/${channel.id}/messages`);
    expect(refused.status()).toBe(403);
    const browsed = await bob.get(`${API}/workspaces/${workspace.id}/channels/browse`);
    const listed = ((await browsed.json()) as { data: { id: string }[] }).data.map((c) => c.id);
    expect(listed, 'nor does Browse list it').not.toContain(channel.id);

    await page.keyboard.press('Escape');
    await expect(page.getByText('made this channel private')).toBeVisible();

    const counted = await admin.get(`${API}/channels/${channel.id}/privacy`);
    expect(counted.status()).toBe(200);
    const { messages } = (await counted.json()) as { messages: number };
    expect(messages, 'the message and the notice').toBeGreaterThanOrEqual(2);

    await openSettings(page);
    await page.getByTestId('channel-settings-privacy-change').click();
    const dialog = page.getByTestId('channel-privacy-public-dialog');
    await expect(dialog).toContainText(`${messages} messages`);
    await expect(dialog).toContainText('0 files');
    const confirm = page.getByTestId('channel-privacy-public-dialog-confirm');
    await expect(confirm).toBeDisabled();
    await page.getByTestId('channel-privacy-public-understand').check();
    await confirm.click();
    await expect(page.getByTestId('channel-settings-privacy')).toContainText('Public');

    const reopened = await bob.get(`${API}/channels/${channel.id}/messages`);
    expect(reopened.status(), 'the history reads for everyone again').toBe(200);
  } finally {
    await admin.delete(`${API}/channels/${channel.id}`);
    await watcherContext.close();
    await admin.dispose();
    await bob.dispose();
  }
});
