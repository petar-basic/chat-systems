import { execSync } from 'node:child_process';
import { test, expect, type Page, type BrowserContext } from '@playwright/test';
import { API, authHeaders, login, send, SHOTS } from './helpers';

const stamp = process.env.E2E_STAMP || 'run2';

let ctxA: BrowserContext;
let ctxB: BrowserContext;
let admin: Page;
let bob: Page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async ({ browser }) => {
  ctxA = await browser.newContext({ permissions: ['notifications'] });
  ctxB = await browser.newContext({ permissions: ['notifications'] });
  admin = await ctxA.newPage();
  bob = await ctxB.newPage();
  await login(admin, 'admin@dev.local');
  await login(bob, 'bob@dev.local');
});

test.afterAll(async () => {
  await ctxA?.close();
  await ctxB?.close();
});

test('A. realtime gateway restart: banner shows, socket reconnects, missed messages backfill', async () => {
  test.setTimeout(120_000);

  execSync('docker compose stop realtime', { cwd: '/Users/petarbasic/projects/chat-systems' });
  await expect(bob.locator('[data-qa="connection-banner"]')).toBeVisible({ timeout: 30_000 });
  await bob.screenshot({ path: `${SHOTS}/A1-offline-banner.png` });

  const missed = `missed-while-offline ${stamp}`;
  await send(admin, missed);
  await expect(admin.getByText(missed)).toBeVisible();

  execSync('docker compose start realtime', { cwd: '/Users/petarbasic/projects/chat-systems' });
  await expect(bob.locator('[data-qa="connection-banner"]')).toHaveCount(0, { timeout: 60_000 });
  await expect(bob.getByText(missed)).toBeVisible({ timeout: 60_000 });
  await bob.screenshot({ path: `${SHOTS}/A2-after-reconnect-backfill.png` });

  const live = `after-reconnect ${stamp}`;
  await send(admin, live);
  await expect(bob.getByText(live)).toBeVisible({ timeout: 30_000 });
});

test('B. same user in two tabs stays in sync', async () => {
  const tab2 = await ctxB.newPage();
  await tab2.goto('/');
  await expect(tab2.locator('[data-qa="message-list"]')).toBeVisible({ timeout: 20_000 });

  const text = `two-tab ${stamp}`;
  await send(bob, text);
  await expect(tab2.getByText(text)).toBeVisible({ timeout: 15_000 });
  await tab2.screenshot({ path: `${SHOTS}/B-two-tabs.png` });
  await tab2.close();
});

test('C. pin a message and read it back from the pinned panel', async () => {
  const text = `pin-me ${stamp}`;
  await send(admin, text);
  const row = admin.locator('[data-qa="message-row"]', { hasText: text }).last();
  await row.hover();
  await row.locator('[data-qa="message-action-pin"]').click();
  await admin.getByLabel('Pinned messages').click();
  await expect(admin.getByText(text).last()).toBeVisible({ timeout: 10_000 });
  await admin.screenshot({ path: `${SHOTS}/C-pinned.png` });
  await admin.getByLabel('Pinned messages').click();
});

test('D. file upload is delivered to the other user', async () => {
  const chooser = admin.waitForEvent('filechooser');
  await admin.getByLabel('Upload file').click();
  const fc = await chooser;
  await fc.setFiles({
    name: `qa-${stamp}.txt`,
    mimeType: 'text/plain',
    buffer: Buffer.from(`hello attachment ${stamp}`),
  });
  await expect(admin.locator('[data-qa="attachment-file"], [data-qa="attachment-image"]').last()).toBeVisible(
    {
      timeout: 20_000,
    },
  );
  await expect(bob.locator('[data-qa="attachment-file"], [data-qa="attachment-image"]').last()).toBeVisible({
    timeout: 20_000,
  });
  await expect(bob.getByText('file: undefined')).toHaveCount(0);
  await expect(bob.getByText(`qa-${stamp}.txt`).last()).toBeVisible({ timeout: 10_000 });
  await bob.screenshot({ path: `${SHOTS}/D-attachment-bob.png` });
});

test('E. invite → email → registration lands the new user inside the workspace', async () => {
  const email = `newhire-${stamp}@dev.local`;
  await admin
    .getByRole('button', { name: /Members|Channel members/i })
    .first()
    .click()
    .catch(() => {});
  const api = admin.request;
  const auth = await authHeaders(api, 'admin@dev.local');
  const ws = (await (await api.get(`${API}/workspaces`, { headers: auth })).json()).data[0];
  const inv = await api.post(`${API}/workspaces/${ws.id}/invites`, {
    headers: auth,
    data: { email, role: 'member' },
  });
  expect(inv.status()).toBe(200);

  const mail = await api.get(
    'http://localhost:8025/api/v2/search?kind=to&query=' + encodeURIComponent(email),
  );
  const items = (await mail.json()).items;
  expect(items.length).toBeGreaterThan(0);
  const body: string = items[0].Content.Body.replace(/=\r\n/g, '').replace(/=3D/g, '=');
  const link = body.match(/https?:\/\/[^\s"'<>]*invite\/[A-Za-z0-9_.-]+/)?.[0];
  expect(link, 'invite link present in the email').toBeTruthy();

  const ctxC = await admin.context().browser()!.newContext();
  const newbie = await ctxC.newPage();
  await newbie.goto(link as string);
  await newbie.waitForTimeout(2500);
  await newbie.screenshot({ path: `${SHOTS}/E1-invite-page.png` });
  await newbie.locator('input[type="text"]').first().fill('New Hire');
  const pw = newbie.locator('input[type="password"]');
  await pw.first().fill('newhirepass123');
  if ((await pw.count()) > 1) await pw.nth(1).fill('newhirepass123');
  await newbie
    .getByRole('button', { name: /Create account|Complete|Register|Join/i })
    .first()
    .click();
  await expect(newbie.locator('[data-qa="message-list"]')).toBeVisible({ timeout: 20_000 });
  await newbie.screenshot({ path: `${SHOTS}/E2-newbie-inside.png` });

  const hello = `newbie says hi ${stamp}`;
  await send(newbie, hello);
  await expect(admin.getByText(hello)).toBeVisible({ timeout: 15_000 });
  await ctxC.close();
});

test('F. guest is scoped to the channels they were added to', async () => {
  const ctxD = await admin.context().browser()!.newContext();
  const diana = await ctxD.newPage();
  await login(diana, 'diana@dev.local');
  await diana.screenshot({ path: `${SHOTS}/F-guest-view.png` });
  await expect(diana.getByRole('button', { name: 'Instance Admin' })).toHaveCount(0);
  await expect(diana.getByRole('button', { name: /^random$/ })).toHaveCount(0);
  const text = `guest posting ${stamp}`;
  await send(diana, text);
  await expect(admin.getByText(text)).toBeVisible({ timeout: 15_000 });

  const api = diana.request;
  const gauth = await authHeaders(api, 'diana@dev.local');
  const ws = (await (await api.get(`${API}/workspaces`, { headers: gauth })).json()).data[0];
  const chans = (await (await api.get(`${API}/workspaces/${ws.id}/channels`, { headers: gauth })).json())
    .data;
  expect(chans.some((c: { name: string }) => c.name === 'random')).toBe(false);
  const created = await api.post(`${API}/workspaces/${ws.id}/channels`, {
    headers: gauth,
    data: { name: `guest-made-${stamp}` },
  });
  expect(created.status()).toBe(403);
  await ctxD.close();
});
