import { test, expect, type APIRequestContext } from '@playwright/test';
import { MAILHOG } from './helpers';

const EMAIL = 'charlie@dev.local';
const SUBJECT = /^(\d{6}) is your .+ sign-in code$/;

interface MailHogItem {
  ID: string;
  Content: { Headers: { Subject?: string[] } };
}

async function codeMails(request: APIRequestContext): Promise<MailHogItem[]> {
  const mail = await request.get(`${MAILHOG}/api/v2/search?kind=to&query=${encodeURIComponent(EMAIL)}`);
  const items: MailHogItem[] = (await mail.json()).items;
  return items.filter((item) => SUBJECT.test(item.Content.Headers.Subject?.[0] ?? ''));
}

async function newCode(request: APIRequestContext, seen: Set<string>): Promise<string> {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    const fresh = (await codeMails(request)).find((item) => !seen.has(item.ID));
    if (fresh) return SUBJECT.exec(fresh.Content.Headers.Subject?.[0] ?? '')![1];
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`no sign-in code for ${EMAIL} arrived in MailHog within 20s`);
}

test('a code from the email signs in with no password', async ({ page, request }) => {
  const seen = new Set((await codeMails(request)).map((item) => item.ID));

  await page.goto('/');
  await page.getByTestId('login-email').fill(EMAIL);
  await page.getByTestId('email-code').click();
  await expect(page.getByTestId('email-code-email')).toHaveValue(EMAIL);
  await page.getByTestId('email-code-send').click();

  const code = await newCode(request, seen);
  await page.getByTestId('email-code-input').fill(code);

  const verified = page.waitForResponse(
    (r) => r.url().endsWith('/api/auth/email-code/verify') && r.request().method() === 'POST',
  );
  await page.getByTestId('email-code-submit').click();
  expect((await verified).status()).toBe(200);
  await expect(page.locator('.ProseMirror[contenteditable="true"]').last()).toBeVisible({
    timeout: 20_000,
  });
});
