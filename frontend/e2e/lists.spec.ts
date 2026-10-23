import { test, expect, type Page } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

function cell(page: Page, row: number, column: string) {
  return page
    .locator('[data-qa="list-row"]')
    .nth(row)
    .locator(`[data-qa="list-cell"][data-column="${column}"]`);
}

test('two people edit the same list and each sees the other edit arrive', async ({ browser }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'list');
  await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });

  const contexts = [await browser.newContext(), await browser.newContext()];
  const [ana, ben] = [await contexts[0].newPage(), await contexts[1].newPage()];
  try {
    await login(ana, 'admin@dev.local');
    await ana.goto(`/app/${workspace.id}/${channel.id}`);
    await ana.getByTestId('channel-lists').click();
    await ana.getByTestId('channel-list-new').click();
    await ana.getByTestId('new-list-title').fill('Launch plan');
    await ana.getByTestId('new-list-template-project').click();
    await ana.getByTestId('new-list-submit').click();
    await expect(ana.getByTestId('list-title')).toHaveText('Launch plan');
    await expect(ana.getByTestId('list-column-header')).toHaveText([
      'Task',
      'Status',
      'Owner',
      'Due',
      'Priority',
    ]);

    await ana.getByTestId('list-add-item').click();
    await ana.getByTestId('cell-editor').fill('Fix login');
    await ana.getByTestId('cell-editor').press('Enter');
    await expect(cell(ana, 0, 'Task')).toHaveText('Fix login');

    await login(ben, 'bob@dev.local');
    await ben.goto(`/app/${workspace.id}/${channel.id}`);
    await ben.getByTestId('channel-lists').click();
    await ben.getByTestId('channel-list-entry').filter({ hasText: 'Launch plan' }).click();
    await expect(cell(ben, 0, 'Task')).toHaveText('Fix login', { timeout: 20_000 });

    await cell(ben, 0, 'Status').dblclick();
    await ben.getByTestId('cell-editor').selectOption({ label: 'In progress' });
    await expect(cell(ben, 0, 'Status')).toHaveText('In progress');
    await expect(cell(ana, 0, 'Status')).toHaveText('In progress', { timeout: 10_000 });

    await cell(ana, 0, 'Task').dblclick();
    await ana.getByTestId('cell-editor').fill('Fix login page');
    await ana.getByTestId('cell-editor').press('Enter');
    await expect(cell(ben, 0, 'Task')).toHaveText('Fix login page', { timeout: 10_000 });
    await expect(cell(ben, 0, 'Status')).toHaveText('In progress');

    await ben.getByTestId('list-add-item').click();
    await ben.getByTestId('cell-editor').fill('Write release notes');
    await ben.getByTestId('cell-editor').press('Enter');
    await expect(ana.getByTestId('list-count')).toHaveText('2 items', { timeout: 10_000 });
    await expect(cell(ana, 1, 'Task')).toHaveText('Write release notes');
  } finally {
    await contexts[0].close();
    await contexts[1].close();
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
  }
});
