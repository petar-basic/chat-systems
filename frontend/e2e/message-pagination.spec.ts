import { test, expect, type Page } from '@playwright/test';
import { API, login, send, userContext, devWorkspace } from './helpers';

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
async function seedChannel(authors: Ctx[], prefix: string, total = TOTAL) {
  const [admin] = authors;
  const workspace = await devWorkspace(admin);
  const stamp = Date.now();
  const created = await admin.post(`${API}/workspaces/${workspace.id}/channels`, {
    data: { name: `${prefix}-${stamp}`, channel_type: 'public' },
  });
  expect(created.status(), 'create channel').toBe(200);
  const channelId = (await created.json()).id as string;

  for (let i = 0; i < total; i++) {
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

test('a new message lands at the bottom after older pages have loaded', async ({ page }) => {
  test.slow();
  const { ctx: admin } = await userContext('admin@dev.local');
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: bob } = await userContext('bob@dev.local');
  const authors = [admin, alice, bob];
  const { workspace, channelId, stamp } = await seedChannel(authors, 'bottom', 60);
  const prefix = `bottom-${stamp}`;
  const isSend = (url: string, method: string) =>
    method === 'POST' && new URL(url).pathname.endsWith(`/channels/${channelId}/messages`);

  try {
    let release = () => {};
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    let holding = true;
    await page.route(
      (url) => url.pathname.endsWith(`/channels/${channelId}/messages`),
      async (route) => {
        if (holding && route.request().method() === 'POST') {
          holding = false;
          await held;
        }
        await route.continue();
      },
    );

    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channelId}`);
    const list = page.locator('[data-qa="message-list"]');
    const lastRow = list.locator('[data-qa="message-row"]').last();
    await expect(list.getByText(`${prefix} #59`)).toBeVisible({ timeout: 20_000 });

    const scrollToOldest = () =>
      expect
        .poll(
          async () => {
            await list.evaluate((el) => {
              el.scrollTop = 0;
            });
            return list.getByText(`${prefix} #0`).isVisible();
          },
          { timeout: 20_000 },
        )
        .toBe(true);

    await scrollToOldest();
    await send(page, `${prefix} mine`);
    await expect(lastRow, 'the sent message, before the server has it').toContainText(`${prefix} mine`);
    await expect(lastRow).toHaveAttribute('data-own', 'false');

    const confirmed = page.waitForResponse((res) => isSend(res.url(), res.request().method()));
    release();
    expect((await confirmed).status()).toBe(200);
    await expect(lastRow).toHaveAttribute('data-own', 'true');
    await expect(lastRow, 'the sent message, once the server has it').toContainText(`${prefix} mine`);

    await scrollToOldest();
    const posted = await alice.post(`${API}/channels/${channelId}/messages`, {
      data: { content: `${prefix} theirs` },
    });
    expect(posted.status()).toBe(200);
    await page.locator('[data-qa="jump-to-latest"]').click();
    await expect(lastRow, 'a message that arrives').toContainText(`${prefix} theirs`);

    await scrollToOldest();
    const oldest = list.locator('[data-qa="message-row"]', { hasText: `${prefix} #0` });
    await oldest.hover();
    await oldest.locator('[data-qa="message-action-thread"]').click();
    const panel = page.locator('[data-qa="thread-panel"]');
    await panel.locator('[data-qa="thread-also-to-channel"]').check();
    const editor = panel.locator('.ProseMirror[contenteditable="true"]');
    await editor.click();
    await editor.fill(`${prefix} loud`);
    await editor.press('Enter');
    await expect(panel.getByText(`${prefix} loud`)).toBeVisible({ timeout: 15_000 });
    await expect(lastRow, 'a reply also sent to the channel').toContainText(`${prefix} loud`);
  } finally {
    await admin.delete(`${API}/channels/${channelId}`);
    await Promise.all(authors.map((ctx) => ctx.dispose()));
  }
});
