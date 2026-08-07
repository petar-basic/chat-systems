import { expect, request as playwrightRequest, type APIRequestContext, type Page } from '@playwright/test';

export const API = process.env.E2E_API_URL || 'http://localhost:3000/api';
export const PASSWORD = process.env.E2E_PASSWORD || 'admin123456';
export const SHOTS = process.env.E2E_SHOTS || '/tmp/shots';
export const MAILHOG = process.env.E2E_MAILHOG_URL || 'http://localhost:8025';

interface Session {
  token: string;
  userId: string;
}

const sessionCache = new Map<string, Session>();

async function signIn(request: APIRequestContext, email: string): Promise<Session> {
  const cached = sessionCache.get(email);
  if (cached) return cached;
  const res = await request.post(`${API}/auth/login`, { data: { email, password: PASSWORD } });
  expect(res.status(), `login for ${email}`).toBe(200);
  const body = await res.json();
  const session = { token: body.access_token as string, userId: body.user.id as string };
  sessionCache.set(email, session);
  return session;
}

export async function apiToken(request: APIRequestContext, email: string) {
  return (await signIn(request, email)).token;
}

export async function authHeaders(request: APIRequestContext, email: string) {
  return { Authorization: `Bearer ${await apiToken(request, email)}` };
}

/**
 * A request context that acts as one user. The API prefers the session cookie over the
 * Authorization header, so tests that share a context end up acting as whoever logged in
 * last; a per-user context with a cached bearer token keeps identities apart and costs a
 * single login per user per run (the login limiter allows ten).
 */
export async function userContext(email: string) {
  const anonymous = await playwrightRequest.newContext();
  const session = await signIn(anonymous, email);
  await anonymous.dispose();

  const ctx = await playwrightRequest.newContext({
    extraHTTPHeaders: { Authorization: `Bearer ${session.token}` },
  });
  return { ctx, userId: session.userId };
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
