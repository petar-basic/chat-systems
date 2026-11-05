import { test, expect } from '@playwright/test';
import { API, PASSWORD, login, userContext } from './helpers';

test('a passkey added in Preferences signs in with no email typed', async ({ page }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('WebAuthn.enable');
  await cdp.send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: 'ctap2',
      transport: 'internal',
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });
  const name = `e2e-${Date.now()}`;

  try {
    await login(page, 'alice@dev.local');
    await page.getByTestId('open-you-menu').click();
    await page.getByTestId('open-preferences').click();
    await page.getByRole('tab', { name: 'Security' }).click();

    await page.getByTestId('passkey-add').click();
    await page.getByTestId('passkey-name').fill(name);
    await page.getByTestId('passkey-proof').fill(PASSWORD);
    await page.getByTestId('passkey-confirm-submit').click();
    await expect(page.getByTestId('passkey-item').filter({ hasText: name })).toBeVisible();

    await page.context().clearCookies();
    await page.evaluate(() => localStorage.clear());
    await page.goto('/');

    const signIn = page.waitForResponse(
      (r) => r.url().endsWith('/api/auth/passkeys/sign-in') && r.request().method() === 'POST',
    );
    await page.getByRole('button', { name: 'Sign in with a passkey' }).click();
    expect((await signIn).status()).toBe(200);
    await expect(page.locator('.ProseMirror[contenteditable="true"]').last()).toBeVisible({
      timeout: 20_000,
    });
    await expect(page.getByTestId('login-email')).toHaveCount(0);
  } finally {
    const listed = await (await alice.get(`${API}/auth/passkeys`)).json();
    for (const passkey of listed.data ?? []) {
      if (passkey.name === name) {
        await alice.post(`${API}/auth/passkeys/${passkey.id}/remove`, { data: { password: PASSWORD } });
      }
    }
    await alice.dispose();
  }
});
