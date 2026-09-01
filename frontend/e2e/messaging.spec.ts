import { test, expect } from '@playwright/test';
import { devWorkspace, login, send, userContext } from './helpers';

const PASSWORD = process.env.E2E_PASSWORD;

test.skip(!PASSWORD, 'Set E2E_PASSWORD (the seeded admin password) and run the stack to enable E2E.');

test('a message sent by one user appears live for another in the same channel', async ({ browser }) => {
  const sender = await browser.newContext();
  const receiver = await browser.newContext();
  const a = await sender.newPage();
  const b = await receiver.newPage();

  await login(a, 'admin@dev.local');
  await login(b, 'alice@dev.local');

  // Sign-in lands wherever the app decides, which stops being the same place for
  // both users as soon as anything else has created a channel. Live delivery is
  // the thing under test, so put both of them in one named channel first.
  const { ctx } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(ctx);
  await ctx.dispose();
  for (const page of [a, b]) {
    await page.goto(`/app/${workspace.id}`);
    await page
      .getByRole('button', { name: /^general$/ })
      .first()
      .click();
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('general');
  }

  const text = `e2e-${Date.now()}`;
  await send(a, text);

  // Scoped to the list: for a moment the sender holds both the optimistic copy
  // and the echo of it, and an unscoped match turns that into a strict-mode
  // failure about the wrong thing.
  await expect(a.locator('[data-qa="message-list"]').getByText(text).last()).toBeVisible();
  await expect(b.locator('[data-qa="message-list"]').getByText(text).last()).toBeVisible();

  await sender.close();
  await receiver.close();
});
