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

test('a board view one person saves is the board everyone sees, and its drags arrive live', async ({
  browser,
}) => {
  const { ctx: admin, userId: adminId } = await userContext('admin@dev.local');
  const { ctx: bob, userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'board');
  await admin.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });

  const created = await admin.post(`${API}/workspaces/${workspace.id}/lists`, {
    data: {
      title: 'Release board',
      channel_id: channel.id,
      columns: [
        { name: 'Task', kind: 'text' },
        { name: 'Status', kind: 'select', options: { choices: [{ label: 'To do' }, { label: 'Done' }] } },
        { name: 'Owner', kind: 'person' },
      ],
    },
  });
  expect(created.ok()).toBeTruthy();
  const detail = await created.json();
  const [task, status, owner] = detail.columns.map((c: { id: string }) => c.id);
  const todo = detail.columns[1].options.choices[0].id;
  for (const [title, who] of [
    ['Fix login', adminId],
    ['Write notes', bobId],
  ]) {
    const res = await admin.post(`${API}/lists/${detail.list.id}/items`, {
      data: { values: { [task]: title, [status]: todo, [owner]: who } },
    });
    expect(res.ok()).toBeTruthy();
  }

  const lane = (page: Page, name: string) => page.locator(`[data-qa="list-lane"][data-lane="${name}"]`);
  const cards = (page: Page, name: string) => lane(page, name).getByTestId('list-card-title');

  const contexts = [await browser.newContext(), await browser.newContext()];
  const [ana, ben] = [await contexts[0].newPage(), await contexts[1].newPage()];
  try {
    await login(ana, 'admin@dev.local');
    await ana.goto(`/app/${workspace.id}/lists/${detail.list.id}`);
    await expect(ana.getByTestId('list-view-tab')).toHaveText(['Table']);
    await ana.getByTestId('list-view-add').click();
    await ana.getByTestId('list-view-add-board').click();
    await expect(ana.getByTestId('list-view-tab')).toHaveText(['Table', 'Board']);
    await expect(cards(ana, 'To do')).toHaveText(['Fix login', 'Write notes']);

    await login(ben, 'bob@dev.local');
    await ben.goto(`/app/${workspace.id}/lists/${detail.list.id}`);
    await ben.getByTestId('list-view-tab').filter({ hasText: 'Board' }).click();
    await expect(cards(ben, 'To do')).toHaveText(['Fix login', 'Write notes'], { timeout: 20_000 });

    await ana.getByTestId('list-card').filter({ hasText: 'Fix login' }).dragTo(lane(ana, 'Done'));
    await expect(cards(ana, 'Done')).toHaveText(['Fix login']);
    await expect(cards(ben, 'Done')).toHaveText(['Fix login'], { timeout: 10_000 });
    await expect(cards(ben, 'To do')).toHaveText(['Write notes']);

    await ana.getByTestId('list-filter').click();
    await ana.getByTestId('list-filter-mine').click();
    await ana.keyboard.press('Escape');
    await expect(ana.getByTestId('list-card-title')).toHaveText(['Fix login']);
    await expect(ben.getByTestId('list-filter')).toContainText('1', { timeout: 10_000 });
    await expect(ben.getByTestId('list-card-title')).toHaveText(['Write notes']);
    await expect(ben.getByTestId('list-count')).toHaveText('1 of 2 items');
  } finally {
    await contexts[0].close();
    await contexts[1].close();
    await admin.delete(`${API}/channels/${channel.id}`);
    await admin.dispose();
    await bob.dispose();
  }
});
