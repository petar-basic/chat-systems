import { test, expect, type APIRequestContext } from '@playwright/test';
import { API, PASSWORD, login } from './helpers';

const TOTAL = 55;
const PAGE_SIZE = 50;

async function signIn(ctx: APIRequestContext, email: string) {
  const res = await ctx.post(`${API}/auth/login`, { data: { email, password: PASSWORD } });
  expect(res.status(), `login for ${email}`).toBe(200);
}

test('scrolling up loads older messages without repeating any', async ({ page, playwright }) => {
  const admin = await playwright.request.newContext();
  const alice = await playwright.request.newContext();
  const bob = await playwright.request.newContext();
  await signIn(admin, 'admin@dev.local');
  await signIn(alice, 'alice@dev.local');
  await signIn(bob, 'bob@dev.local');
  const authors = [admin, alice, bob];

  const workspace = (await (await admin.get(`${API}/workspaces`)).json()).data[0];
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `paging-${stamp}`, channel_type: 'public' },
  });
  expect(created.status(), 'create channel').toBe(200);
  const channelId = (await created.json()).id as string;

  try {
    for (let i = 0; i < TOTAL; i++) {
      const author = authors[i % authors.length];
      const posted = await author.post(`${API}/channels/${channelId}/messages`, {
        data: { content: `paging-${stamp} #${i}` },
      });
      expect(posted.status(), `seed message ${i}`).toBe(200);
    }

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channelId}`);

    const list = page.locator('[data-qa="message-list"]');
    const rows = list.locator('[data-message-id]');
    await expect(rows).toHaveCount(PAGE_SIZE);

    await list.evaluate((el) => {
      el.scrollTop = -el.scrollHeight;
    });

    await expect(rows).toHaveCount(TOTAL);
    await expect(list.getByText(`paging-${stamp} #0`)).toBeVisible();

    const ids = await rows.evaluateAll((els) => els.map((el) => el.getAttribute('data-message-id')));
    expect(new Set(ids).size, 'every rendered message must appear exactly once').toBe(ids.length);

    const order = await rows.evaluateAll((els) =>
      els.map((el) => Number(el.textContent?.match(/#(\d+)/)?.[1])),
    );
    expect(order[0], 'the oldest message must sit at the top').toBe(0);
    expect(order[order.length - 1], 'the newest message must sit at the bottom').toBe(TOTAL - 1);
    expect(order, 'older messages must load above the newer ones, never below').toEqual(
      [...order].sort((a, b) => a - b),
    );
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await Promise.all(authors.map((ctx) => ctx.dispose()));
  }
});
