import { test, expect } from '@playwright/test';
import { login, send } from './helpers';

const PASSWORD = process.env.E2E_PASSWORD;

test.skip(!PASSWORD, 'Set E2E_PASSWORD (the seeded admin password) and run the stack to enable E2E.');

test('a message sent by one user appears live for another in the same channel', async ({ browser }) => {
  const sender = await browser.newContext();
  const receiver = await browser.newContext();
  const a = await sender.newPage();
  const b = await receiver.newPage();

  await login(a, 'admin@dev.local');
  await login(b, 'alice@dev.local');

  const text = `e2e-${Date.now()}`;
  await send(a, text);

  await expect(a.getByText(text)).toBeVisible();
  await expect(b.getByText(text)).toBeVisible();

  await sender.close();
  await receiver.close();
});
