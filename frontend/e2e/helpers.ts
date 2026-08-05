import { expect, type APIRequestContext, type Page } from '@playwright/test';

export const API = 'http://localhost:3000/api';
export const PASSWORD = process.env.E2E_PASSWORD || 'admin123456';
export const SHOTS = process.env.E2E_SHOTS || '/tmp/shots';

const tokenCache = new Map<string, string>();

export async function apiToken(request: APIRequestContext, email: string) {
  const cached = tokenCache.get(email);
  if (cached) return cached;
  const res = await request.post(`${API}/auth/login`, { data: { email, password: PASSWORD } });
  expect(res.status(), `login for ${email}`).toBe(200);
  const token = (await res.json()).access_token as string;
  tokenCache.set(email, token);
  return token;
}

export async function authHeaders(request: APIRequestContext, email: string) {
  return { Authorization: `Bearer ${await apiToken(request, email)}` };
}

export async function login(page: Page, email: string) {
  await page.goto('/');
  await page.locator('#email').fill(email);
  await page.locator('#password').fill(PASSWORD);
  await page.getByRole('button', { name: 'Connect' }).click();
  await expect(page.locator('.ProseMirror[contenteditable="true"]').last()).toBeVisible({ timeout: 20_000 });
}

export async function send(page: Page, text: string) {
  const editor = page.locator('.ProseMirror[contenteditable="true"]').last();
  await editor.click();
  await editor.fill(text);
  await editor.press('Enter');
}
