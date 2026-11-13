import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace } from './helpers';

test('a group DM becomes a private channel that keeps its history and takes a fourth person', async ({
  page,
}) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const { ctx: charlie, userId: charlieId } = await userContext('charlie@dev.local');
  const { ctx: admin, userId: adminId } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(alice);
  const name = `crew-${Date.now()}`;

  const opened = await alice.post(`${API}/workspaces/${workspace.id}/conversations`, {
    data: { participant_ids: [bobId, charlieId] },
  });
  expect(opened.status()).toBe(200);
  const group = (await opened.json()) as { id: string; kind: string };
  expect(group.kind).toBe('group');

  try {
    const said = `before-the-channel-${Date.now()}`;
    expect(
      (await bob.post(`${API}/channels/${group.id}/messages`, { data: { content: said } })).status(),
    ).toBe(200);

    expect(
      (await admin.post(`${API}/conversations/${group.id}/convert`, { data: { name } })).status(),
      'an outsider',
    ).toBe(403);

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/c/${group.id}`);
    await expect(page.getByText(said)).toBeVisible();
    await page.getByTestId('conversation-convert').click();
    await page.getByTestId('conversation-convert-name').fill(name);
    await page.getByTestId('conversation-convert-submit').click();

    await expect(page).toHaveURL(new RegExp(`/app/${workspace.id}/${group.id}$`));
    await expect(page.getByTestId('channel-header-name')).toHaveText(name);
    await expect(page.getByText(said)).toBeVisible();
    await expect(page.getByText('converted this conversation into a private channel')).toBeVisible();
    await expect(page.getByRole('navigation').getByText(name)).toBeVisible();

    const listed = await (await charlie.get(`${API}/workspaces/${workspace.id}/conversations`)).json();
    expect(
      (listed.data as { id: string }[]).map((c) => c.id),
      'it left the DMs for everyone in it',
    ).not.toContain(group.id);

    const added = await alice.post(`${API}/channels/${group.id}/members`, { data: { user_id: adminId } });
    expect(added.status(), 'a channel takes a fourth person').toBe(200);
    const history = await (await admin.get(`${API}/channels/${group.id}/messages`)).json();
    expect(JSON.stringify(history.data), 'who reads what was said before').toContain(said);
  } finally {
    await alice.delete(`${API}/channels/${group.id}`);
    await alice.dispose();
    await bob.dispose();
    await charlie.dispose();
    await admin.dispose();
  }
});
