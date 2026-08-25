import { test, expect, type Page, type BrowserContext } from '@playwright/test';
import { authHeaders, login, send, SHOTS, userContext } from './helpers';

let ctxA: BrowserContext;
let ctxB: BrowserContext;
let admin: Page;
let bob: Page;

const stamp = process.env.E2E_STAMP || `run-${Date.now()}`;

async function openChannel(page: Page, name: string) {
  await page
    .getByRole('button', { name: new RegExp(`^${name}$`) })
    .first()
    .click();
  // The header, not the message list: a channel with no messages renders the
  // empty state instead, and "the channel is open" is the thing being waited on.
  await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(name);
}

test.describe.configure({ mode: 'serial' });

test.beforeAll(async ({ browser }) => {
  ctxA = await browser.newContext({ permissions: ['notifications'] });
  ctxB = await browser.newContext({ permissions: ['notifications'] });
  admin = await ctxA.newPage();
  bob = await ctxB.newPage();
});

test.afterAll(async () => {
  await ctxA?.close();
  await ctxB?.close();
});

test('1. cold login both users, land in workspace', async () => {
  await login(admin, 'admin@dev.local');
  await login(bob, 'bob@dev.local');
  await admin.screenshot({ path: `${SHOTS}/01-admin-logged-in.png`, fullPage: false });
  await bob.screenshot({ path: `${SHOTS}/01-bob-logged-in.png` });
  await expect(admin.locator('[data-qa="connection-banner"]')).toHaveCount(0);
  await expect(bob.locator('[data-qa="connection-banner"]')).toHaveCount(0);
});

test('2. typing indicator reaches the other user (before any send)', async () => {
  await openChannel(admin, 'general');
  await openChannel(bob, 'general');
  const editor = bob.locator('.ProseMirror[contenteditable="true"]').last();
  await editor.click();
  await editor.type('typing test…', { delay: 40 });
  await expect(admin.getByText(/is typing/i)).toBeVisible({ timeout: 10_000 });
  await admin.screenshot({ path: `${SHOTS}/03-typing-indicator.png` });
  await editor.fill('');
});

test('3. channel message is delivered live in both directions', async () => {
  const fromAdmin = `A→B ${stamp} hello from admin`;
  await send(admin, fromAdmin);
  await expect(admin.getByText(fromAdmin).first()).toBeVisible();
  await expect(bob.getByText(fromAdmin).first()).toBeVisible({ timeout: 10_000 });

  const fromBob = `B→A ${stamp} hello from bob`;
  await send(bob, fromBob);
  await expect(admin.getByText(fromBob)).toBeVisible({ timeout: 10_000 });
  await admin.screenshot({ path: `${SHOTS}/02-realtime-both-ways.png` });
});

test('3b. typing indicator still works after the user sent a message', async () => {
  const editor = bob.locator('.ProseMirror[contenteditable="true"]').last();
  await editor.click();
  await editor.type('typing again after send', { delay: 40 });
  await bob.waitForTimeout(1500);
  await expect(admin.getByText(/is typing/i)).toBeVisible({ timeout: 5000 });
  await admin.screenshot({ path: `${SHOTS}/03b-typing-after-send.png` });
  await editor.fill('');
});

test('4. reaction added by one user shows for the other', async () => {
  const target = admin.locator('[data-qa="message-row"]').last();
  await target.hover();
  await target.locator('[data-qa="message-action-react"]').click();
  const picker = admin.locator('em-emoji-picker');
  await picker.waitFor({ state: 'attached', timeout: 15_000 });
  const search = picker.locator('input').first();
  await search.fill('thumbsup');
  await admin.waitForTimeout(500);
  await search.press('Enter');
  await expect(admin.locator('[data-qa="message-reaction"]').last()).toContainText('👍');
  await expect(bob.locator('[data-qa="message-reaction"]').last()).toContainText('👍', { timeout: 10_000 });
  await bob.screenshot({ path: `${SHOTS}/04-reaction-live.png` });
});

test('5. thread reply is live and updates the reply count', async () => {
  const parentText = `thread-parent ${stamp}`;
  await send(admin, parentText);
  await expect(bob.getByText(parentText)).toBeVisible({ timeout: 10_000 });

  const parentRow = admin.locator('[data-qa="message-row"]', { hasText: parentText }).last();
  await parentRow.hover();
  await parentRow.locator('[data-qa="message-action-thread"]').click();
  await expect(admin.locator('[data-qa="thread-panel"]')).toBeVisible();

  const replyText = `thread-reply ${stamp}`;
  const threadEditor = admin.locator('[data-qa="thread-panel"] .ProseMirror[contenteditable="true"]');
  await threadEditor.click();
  await threadEditor.fill(replyText);
  await threadEditor.press('Enter');
  await expect(admin.locator('[data-qa="thread-panel"]').getByText(replyText)).toBeVisible({
    timeout: 10_000,
  });
  await admin.waitForTimeout(1500);
  await expect(admin.locator('[data-qa="thread-panel"]').getByText(replyText)).toHaveCount(1);
  await expect(admin.locator('[data-qa="thread-panel"]').getByText(/^1 reply$/)).toBeVisible();
  await admin.screenshot({ path: `${SHOTS}/05-thread-admin.png` });

  const bobParent = bob.locator('[data-qa="message-row"]', { hasText: parentText }).last();
  await expect(bobParent.getByText(/1 reply/)).toBeVisible({ timeout: 10_000 });
  await bobParent.getByText(/1 reply/).click();
  await expect(bob.locator('[data-qa="thread-panel"]').getByText(replyText)).toHaveCount(1);
  await bob.screenshot({ path: `${SHOTS}/05-thread-bob.png` });
  const secondReply = `thread-reply-2 ${stamp}`;
  await threadEditor.click();
  await threadEditor.fill(secondReply);
  await threadEditor.press('Enter');
  await admin.waitForTimeout(1500);
  await expect(admin.locator('[data-qa="thread-panel"]').getByText(secondReply)).toHaveCount(1);
  await expect(bob.locator('[data-qa="thread-panel"]').getByText(secondReply)).toHaveCount(1);

  await bob.getByRole('button', { name: 'Close thread' }).click();
  await admin.getByRole('button', { name: 'Close thread' }).click();
});

test('6. @mention produces a notification for the mentioned user', async () => {
  const editor = admin.locator('.ProseMirror[contenteditable="true"]').last();
  await editor.click();
  await editor.type('@Bob', { delay: 60 });
  await expect(admin.getByText('Bob Smith').last()).toBeVisible({ timeout: 5000 });
  await editor.press('Enter');
  await editor.type(` please review ${stamp}`, { delay: 20 });
  await editor.press('Enter');

  await expect(bob.getByText(new RegExp(`please review ${stamp}`))).toBeVisible({ timeout: 10_000 });
  await bob.screenshot({ path: `${SHOTS}/06-mention-in-channel.png` });

  await bob.getByTitle('Notifications').click();
  await expect(bob.locator('[data-qa="notifications-panel"]')).toBeVisible();
  await expect(bob.locator('[data-qa="notification-row"]').first()).toBeVisible({ timeout: 10_000 });
  await bob.screenshot({ path: `${SHOTS}/06-notification-panel.png` });
  await bob.getByRole('button', { name: 'Close notifications' }).click();
});

test('7. edit and delete propagate live', async () => {
  const original = `edit-me ${stamp}`;
  await send(admin, original);
  await expect(bob.getByText(original)).toBeVisible({ timeout: 10_000 });

  const row = admin.locator('[data-qa="message-row"]', { hasText: original }).last();
  await row.hover();
  await row.locator('[data-qa="message-action-edit"]').click();
  const editEditor = admin.locator('[data-qa="message-row"] .ProseMirror[contenteditable="true"]');
  await editEditor.click();
  await editEditor.fill(`edited ${stamp}`);
  await admin.locator('[data-qa="message-edit-save"]').click();
  await expect(bob.getByText(`edited ${stamp}`)).toBeVisible({ timeout: 10_000 });

  const row2 = admin.locator('[data-qa="message-row"]', { hasText: `edited ${stamp}` }).last();
  await row2.hover();
  await row2.locator('[data-qa="message-action-delete"]').click();
  await row2.locator('[data-qa="message-delete-confirm"]').click();
  await expect(bob.getByText(`edited ${stamp}`)).toHaveCount(0, { timeout: 10_000 });
  await bob.screenshot({ path: `${SHOTS}/07-after-edit-delete.png` });
});

test('8. unread indicator appears on a channel the user is not viewing', async () => {
  const api = admin.request;
  const auth = await authHeaders(api, 'admin@dev.local');
  const wsId = (await (await api.get('http://localhost:3000/api/workspaces', { headers: auth })).json())
    .data[0].id;
  const chList = await (
    await api.get(`http://localhost:3000/api/workspaces/${wsId}/channels`, { headers: auth })
  ).json();
  const randomId = chList.data.find((c: { name: string }) => c.name === 'random').id;
  const members = await (
    await api.get(`http://localhost:3000/api/workspaces/${wsId}/members`, { headers: auth })
  ).json();
  const bobId = members.data.find((m: { email: string }) => m.email === 'bob@dev.local').user_id;
  await api.post(`http://localhost:3000/api/channels/${randomId}/members`, {
    headers: auth,
    data: { user_id: bobId },
  });
  await bob.reload();
  await expect(bob.locator('[data-qa="channel-header-name"]')).toBeVisible({ timeout: 20_000 });
  await openChannel(bob, 'random');
  const unreadText = `unread-probe ${stamp}`;
  await send(admin, unreadText);
  const generalItem = bob.getByRole('button', { name: /general/ }).first();
  await expect(generalItem).toHaveClass(/text-white|font-semibold|font-bold/, { timeout: 10_000 });
  await bob.screenshot({ path: `${SHOTS}/08-unread-badge.png` });
  await openChannel(bob, 'general');
  await expect(bob.getByText(unreadText)).toBeVisible({ timeout: 10_000 });
});

test('9. direct message is delivered live', async () => {
  await admin.getByTitle('New direct message').click();
  await admin.getByLabel('Search people').fill('Bob');
  await admin.locator('[data-qa="new-dm-modal"]').getByText('Bob Smith').first().click();
  await admin.locator('[data-qa="new-dm-start"]').click();
  await expect(admin.locator('[data-qa="new-dm-modal"]')).toBeHidden();
  // The composer is shared scenery: type before the conversation view has
  // replaced the channel view and the message goes to the channel instead.
  await expect(admin.locator('[data-qa="conversation-title"]')).toContainText('Bob Smith');
  const dmText = `dm ${stamp} secret hello`;
  await send(admin, dmText);
  await expect(admin.locator('[data-qa="conversation-message"]').last()).toContainText(dmText, {
    timeout: 10_000,
  });
  await admin.screenshot({ path: `${SHOTS}/09-dm-admin.png` });

  await bob.screenshot({ path: `${SHOTS}/09-dm-bob-sidebar-before-open.png` });
  await bob.getByTitle('Message Admin').first().click();
  await expect(bob.locator('[data-qa="conversation-message"]').last()).toContainText(dmText, {
    timeout: 10_000,
  });
  await bob.screenshot({ path: `${SHOTS}/09-dm-bob.png` });
});

test('10. search finds a message', async () => {
  await bob.goto('/');
  await expect(bob.locator('[data-qa="message-list"]')).toBeVisible({ timeout: 20_000 });
  await bob.getByLabel('Search messages').click();
  await bob
    .getByPlaceholder(/Search/i)
    .first()
    .fill(`unread-probe ${stamp}`);
  await bob.keyboard.press('Enter');
  await expect(bob.locator('[data-qa="search-result"]').first()).toBeVisible({ timeout: 15_000 });
  await bob.screenshot({ path: `${SHOTS}/10-search.png` });
});

test('11. member cannot reach the instance admin area', async () => {
  await expect(bob.getByRole('button', { name: 'Instance Admin' })).toHaveCount(0);
  await bob.goto('/app/admin');
  await bob.waitForTimeout(2000);
  await bob.screenshot({ path: `${SHOTS}/11-bob-admin-page.png` });
  const body = await bob.locator('body').innerText();
  expect(body).not.toMatch(/Instance Statistics|Total Users|Suspend/i);
});

test('12. suspending a user kills their live session', async () => {
  const api = admin.request;
  const auth = await authHeaders(api, 'admin@dev.local');
  // Bob's own session is where his id comes from: the admin listing is paged
  // newest-first, and a long-lived dev database pushes the seeded accounts off
  // the first page.
  const { ctx: bobApi, userId: bobId } = await userContext('bob@dev.local');
  await bobApi.dispose();

  await bob.goto('/');
  await expect(bob.locator('[data-qa="message-list"]')).toBeVisible({ timeout: 20_000 });

  const susp = await api.post(`http://localhost:3000/api/admin/users/${bobId}/suspend`, { headers: auth });
  expect(susp.status()).toBe(200);

  await bob.waitForTimeout(3000);
  await bob.reload();
  await expect(bob.locator('#email')).toBeVisible({ timeout: 20_000 });
  await bob.screenshot({ path: `${SHOTS}/12-bob-suspended.png` });

  await api.post(`http://localhost:3000/api/admin/users/${bobId}/activate`, { headers: auth });
});
