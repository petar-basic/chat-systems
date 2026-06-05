import { test, expect, type Page } from '@playwright/test';

const PASSWORD = process.env.E2E_PASSWORD;

test.skip(!PASSWORD, 'Set E2E_PASSWORD (the seeded admin password) and run the stack to enable E2E.');

async function login(page: Page, email: string) {
  await page.goto('/');
  await page.locator('#email').fill(email);
  await page.locator('#password').fill(PASSWORD as string);
  await page.getByRole('button', { name: 'Connect' }).click();
  await expect(page.locator('[data-qa="message-list"]')).toBeVisible();
}

async function sendMessage(page: Page, text: string) {
  const editor = page.locator('.ProseMirror');
  await editor.click();
  await editor.fill(text);
  await editor.press('Enter');
}

test('a message sent by one user appears live for another in the same channel', async ({ browser }) => {
  const sender = await browser.newContext();
  const receiver = await browser.newContext();
  const a = await sender.newPage();
  const b = await receiver.newPage();

  await login(a, 'admin@dev.local');
  await login(b, 'alice@dev.local');

  const text = `e2e-${Date.now()}`;
  await sendMessage(a, text);

  await expect(a.getByText(text)).toBeVisible();
  await expect(b.getByText(text)).toBeVisible();

  await sender.close();
  await receiver.close();
});
