import { test, expect, type Page } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test.use({
  launchOptions: { args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'] },
  permissions: ['microphone'],
});

const huddleOf = (page: Page) => page.getByRole('dialog', { name: 'Huddle' });

test('a reply written in the huddle reaches the channel through its thread', async ({ page, browser }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { userId: bobId } = await userContext('bob@dev.local');
  const { userId: charlieId } = await userContext('charlie@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'huddle-thread');
  for (const user_id of [bobId, charlieId]) {
    const added = await alice.post(`${API}/channels/${channel.id}/members`, { data: { user_id } });
    expect(added.status(), 'add a channel member').toBe(200);
  }
  const bobContext = await browser.newContext({ permissions: ['microphone'] });
  const charlieContext = await browser.newContext();
  const bobPage = await bobContext.newPage();
  const charliePage = await charlieContext.newPage();
  const reply = `huddle-note-${Date.now()}`;

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await page.getByRole('button', { name: 'Start huddle' }).click();
    await expect(huddleOf(page).getByTestId('huddle-thread-toggle')).toBeVisible({ timeout: 20_000 });

    await login(bobPage, 'bob@dev.local');
    await bobPage.goto(`/app/${workspace.id}/${channel.id}`);
    await bobPage.getByRole('button', { name: 'Join huddle' }).click();
    const bobHuddle = huddleOf(bobPage);
    await bobHuddle.getByTestId('huddle-thread-toggle').click();
    const panel = bobHuddle.getByTestId('thread-panel');
    await expect(panel).toBeVisible();
    const editor = panel.locator('.ProseMirror[contenteditable="true"]');
    await editor.click();
    await editor.fill(reply);
    await editor.press('Enter');
    await expect(panel.getByTestId('thread-reply').getByText(reply)).toBeVisible();

    await expect(huddleOf(page).getByTestId('huddle-thread-unread')).toHaveText('1');

    await login(charliePage, 'charlie@dev.local');
    await charliePage.goto(`/app/${workspace.id}/${channel.id}`);
    const started = charliePage.getByTestId('huddle-system-message');
    await started.getByTestId('message-thread-open').click();
    await expect(
      charliePage.getByTestId('thread-panel').getByTestId('thread-reply').getByText(reply),
    ).toBeVisible();
  } finally {
    for (const p of [bobPage, page]) {
      const leave = huddleOf(p).getByRole('button', { name: 'Leave huddle' });
      if (await leave.isVisible()) await leave.click();
    }
    await bobContext.close();
    await charlieContext.close();
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
