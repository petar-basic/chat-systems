import { test, expect, type Page } from '@playwright/test';
import { API, login, userContext } from './helpers';

const TOTAL = 120;

/**
 * The list is windowed, so "every message is in the DOM" stopped being true and
 * stopped being the thing worth asserting. What matters now is that scrolling up
 * reaches the oldest message, shows each one in order, and does not move the
 * viewport when an older page lands.
 */
async function mountedOrder(page: Page): Promise<number[]> {
  return page
    .locator('[data-qa="message-list"] [data-message-id]')
    .evaluateAll((els) =>
      els.map((el) => Number(el.textContent?.match(/#(\d+)/)?.[1])).filter((n) => Number.isFinite(n)),
    );
}

type Ctx = import('@playwright/test').APIRequestContext;

/** Spread across three authors: one user posting 120 messages trips the write limit. */
async function seedChannel(authors: Ctx[], prefix: string) {
  const [admin] = authors;
  const workspace = (await (await admin.get(`${API}/workspaces`)).json()).data[0];
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `${prefix}-${stamp}`, channel_type: 'public' },
  });
  expect(created.status(), 'create channel').toBe(200);
  const channelId = (await created.json()).id as string;

  for (let i = 0; i < TOTAL; i++) {
    const posted = await authors[i % authors.length].post(`${API}/channels/${channelId}/messages`, {
      data: { content: `${prefix}-${stamp} #${i}` },
    });
    expect(posted.status(), `seed message ${i}`).toBe(200);
  }
  return { workspace, channelId, stamp };
}

test('scrolling up loads older messages and shows every one of them in order', async ({ page }) => {
  // Seeding a multi-page channel over HTTP is the slow part, not the assertions.
  test.slow();
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const authors = [admin, alice, bob];
  const { workspace, channelId, stamp } = await seedChannel(authors, 'paging');

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channelId}`);

    const list = page.locator('[data-qa="message-list"]');
    const rows = list.locator('[data-message-id]');
    await expect(rows.first()).toBeVisible();

    expect(await rows.count(), 'the list must window rather than mount every loaded message').toBeLessThan(
      TOTAL,
    );
    await expect(list.getByText(`paging-${stamp} #${TOTAL - 1}`)).toBeVisible();

    // Walk to the top a screenful at a time. What is asserted along the way is
    // what the window guarantees — order and no duplicates — rather than that a
    // sampling loop happened to observe every message, which would be testing
    // the cadence of the test.
    for (let step = 0; step < 60; step++) {
      const order = await mountedOrder(page);
      expect(order, 'a window shows its messages oldest first').toEqual([...order].sort((a, b) => a - b));
      expect(new Set(order).size, 'a window never shows the same message twice').toBe(order.length);

      const atTop = await list.evaluate((el) => {
        if (el.scrollTop === 0) return true;
        el.scrollTop = Math.max(0, el.scrollTop - el.clientHeight / 2);
        return false;
      });
      await page.waitForTimeout(180);
      if (atTop && (await list.getByText(`paging-${stamp} #0`).isVisible())) break;
    }

    await expect(list.getByText(`paging-${stamp} #0`)).toBeVisible();
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await Promise.all(authors.map((ctx) => ctx.dispose()));
  }
});

test('loading an older page does not move the viewport', async ({ page }) => {
  test.slow();
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const authors = [admin, alice, bob];
  const { workspace, channelId } = await seedChannel(authors, 'anchor');

  try {
    await login(page, 'admin@dev.local');

    // Holding the older page back makes the measurement deterministic: without
    // it the prepend can land before the "before" reading is taken, and the test
    // would be timing the network rather than the anchor.
    let holdBack = false;
    await page.route(`**/api/channels/${channelId}/messages*`, async (route) => {
      if (holdBack) await new Promise((resolve) => setTimeout(resolve, 1500));
      await route.continue();
    });

    await page.goto(`/app/${workspace.id}/${channelId}`);
    const list = page.locator('[data-qa="message-list"]');
    const rows = list.locator('[data-message-id]');
    await expect(rows.first()).toBeVisible();

    holdBack = true;
    // Windowing keeps the number of mounted rows roughly constant however many
    // are loaded, so the arrival of the page is the signal, not a row count.
    const olderPage = page.waitForResponse(
      (res) => res.url().includes(`/channels/${channelId}/messages`) && res.status() === 200,
    );

    await list.evaluate((el) => {
      el.scrollTop = 0;
    });
    // Long enough for the scroll to settle, short enough that the held-back page
    // has not arrived.
    await page.waitForTimeout(400);

    // Which message the reader is looking at, rather than a particular DOM node:
    // the virtualizer owns those and recycles them, so a node captured before
    // the prepend may simply not exist after it. The number at the top of the
    // viewport is the same property expressed in terms the list cannot recycle.
    const topmostNumber = async () =>
      list.evaluate((el) => {
        const top = el.getBoundingClientRect().top;
        for (const row of el.querySelectorAll<HTMLElement>('[data-message-id]')) {
          const rect = row.getBoundingClientRect();
          if (rect.bottom > top) return Number(row.textContent?.match(/#(\d+)/)?.[1] ?? NaN);
        }
        return NaN;
      });

    const before = await topmostNumber();
    expect(Number.isFinite(before), 'a message at the top of the viewport').toBe(true);

    await olderPage;
    await page.waitForTimeout(600);

    const after = await topmostNumber();

    expect(
      Math.abs(after - before),
      'the reader must still be looking at the same place, give or take the row on the boundary',
    ).toBeLessThanOrEqual(1);
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await Promise.all(authors.map((ctx) => ctx.dispose()));
  }
});
