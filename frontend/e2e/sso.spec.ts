import { test, expect } from '@playwright/test';
import { API, PASSWORD } from './helpers';

// `oidc` is the compose service name, so the API container resolves it directly.
// Chromium is told to resolve the same name to the published port, which keeps one
// issuer string true on both sides — a mismatch there is rejected by the client and
// is the single most common way an SSO setup fails.
test.use({
  launchOptions: { args: ['--host-resolver-rules=MAP oidc 127.0.0.1'] },
});

test('signing in through the identity provider lands in the app', async ({ page }) => {
  await page.goto('/api/auth/oidc/start');

  await page.waitForURL(/\/app/, { timeout: 20_000 });
  await expect(page).toHaveURL(/\/app/);
  await expect(page).not.toHaveURL(/sso=1/);

  const me = await page.request.get(`${API}/users/me`);
  expect(me.status()).toBe(200);
  expect((await me.json()).email).toBe('sso@dev.local');
});

test('the provisioned account has no password to fall back to', async ({ request }) => {
  const res = await request.post(`${API}/auth/login`, {
    data: { email: 'sso@dev.local', password: PASSWORD },
  });
  expect(res.status(), 'the identity provider is the only credential').toBe(401);
});

test('a callback that did not start here is refused', async ({ request }) => {
  const res = await request.get(`${API}/auth/oidc/callback?code=stolen&state=guessed`, {
    maxRedirects: 0,
  });
  expect(res.status()).toBe(401);
});

test('the sign-in page offers SSO only when the instance has it', async ({ page }) => {
  await page.goto('/');
  const info = await page.request.get(`${API}/instance/info`);
  const { sso_enabled: ssoEnabled } = await info.json();
  expect(ssoEnabled).toBe(true);
  await expect(page.getByRole('link', { name: 'Sign in with SSO' })).toBeVisible();
});
