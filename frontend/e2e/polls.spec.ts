import { test, expect, type Page } from '@playwright/test';
import { API, createChannel, devWorkspace, login, send, userContext } from './helpers';

function option(page: Page, label: string) {
  return page.locator('[data-qa="poll-card"] [data-qa="poll-option"]').filter({ hasText: label });
}

test('two people vote on a poll and each sees the other vote arrive', async ({ browser }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'poll');
  await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });

  const contexts = [await browser.newContext(), await browser.newContext()];
  const [asker, voter] = [await contexts[0].newPage(), await contexts[1].newPage()];
  try {
    await login(asker, 'admin@dev.local');
    await asker.goto(`/app/${workspace.id}/${channel.id}`);
    await send(asker, '/poll "Lunch?" "Thursday" "Friday"');
    await expect(asker.locator('[data-qa="poll-question"]')).toHaveText('Lunch?');

    await login(voter, 'bob@dev.local');
    await voter.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(voter.locator('[data-qa="poll-question"]')).toHaveText('Lunch?', { timeout: 20_000 });

    await option(voter, 'Friday').click();
    await expect(option(voter, 'Friday')).toHaveAttribute('aria-pressed', 'true');
    await expect(option(asker, 'Friday').locator('[data-qa="poll-option-count"]')).toHaveText('1', {
      timeout: 10_000,
    });
    await expect(option(asker, 'Friday')).toHaveAttribute('aria-pressed', 'false');

    await option(asker, 'Thursday').click();
    await expect(option(asker, 'Thursday')).toHaveAttribute('aria-pressed', 'true');
    await expect(option(voter, 'Thursday').locator('[data-qa="poll-option-count"]')).toHaveText('1', {
      timeout: 10_000,
    });
    await expect(voter.locator('[data-qa="poll-footer"]')).toHaveText(/^2 votes/);
    await expect(option(voter, 'Friday')).toHaveAttribute('aria-pressed', 'true');
  } finally {
    await contexts[0].close();
    await contexts[1].close();
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
  }
});
