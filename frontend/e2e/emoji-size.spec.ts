import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test('an emoji-only message is rendered larger than an emoji inside a sentence', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'emoji-size');
  const sentence = `emoji-in-a-sentence-${Date.now()}`;

  await alice.post(`${API}/channels/${channel.id}/messages`, { data: { content: '🎉🔥' } });
  await alice.post(`${API}/channels/${channel.id}/messages`, { data: { content: `${sentence} 🎉` } });

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);

    const jumbo = page.locator('[data-qa="jumbo-emoji"] [data-qa="emoji"]').first();
    await expect(jumbo).toBeVisible({ timeout: 20_000 });

    const inline = page
      .locator('[data-qa="message-row"]', { hasText: sentence })
      .locator('[data-qa="emoji"]')
      .first();
    await expect(inline).toBeVisible();

    const sizeOf = (target: typeof jumbo) =>
      target.evaluate((el) => parseFloat(getComputedStyle(el).fontSize));

    const jumboSize = await sizeOf(jumbo);
    const inlineSize = await sizeOf(inline);
    const bodySize = await page
      .locator('[data-qa="message-row"]', { hasText: sentence })
      .evaluate((el) => parseFloat(getComputedStyle(el).fontSize));

    expect(jumboSize, 'an emoji-only message gets the large treatment').toBeGreaterThanOrEqual(32);
    expect(inlineSize, 'an emoji in a sentence stays inline-sized').toBeLessThan(jumboSize);
    expect(inlineSize, 'and still reads larger than the text around it').toBeGreaterThan(bodySize);
  } finally {
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
