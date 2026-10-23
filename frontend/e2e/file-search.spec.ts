import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test('a file posted without a caption is found by a fragment of its name', async ({ page }) => {
  test.slow();
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'file-search');
  try {
    const uploaded = await alice.post(`${API}/files/upload/${workspace.id}?draft=true`, {
      multipart: {
        file: {
          name: 'Q3 budget.xlsx',
          mimeType: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
          buffer: Buffer.from('not really a spreadsheet'),
        },
      },
    });
    expect(uploaded.status()).toBe(200);
    const [file] = await uploaded.json();
    const posted = await alice.post(`${API}/channels/${channel.id}/messages`, {
      data: { content: '', attachment_ids: [file.id] },
    });
    expect(posted.status()).toBe(200);

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await page.locator('[data-qa="channel-header-search"]').click();
    await page.locator('[data-qa="search-tab-files"]').click();
    await page.getByPlaceholder('Search files...').fill('budg');

    const hit = page
      .locator('[data-qa="file-hit"]')
      .filter({ hasText: `#${channel.name}` })
      .filter({ hasText: 'Q3 budget.xlsx' });
    await expect(hit).toHaveCount(1, { timeout: 15_000 });
    await expect(hit.locator('[data-qa="file-hit-match"]')).toHaveText('budg');
    await expect(hit.locator('[data-qa="file-hit-kind-spreadsheet"]')).toBeVisible();

    await hit.getByTitle('Show in conversation').click();
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(channel.name);
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
