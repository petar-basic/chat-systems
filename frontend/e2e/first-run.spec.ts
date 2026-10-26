import { test, expect, request as playwrightRequest } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { API, inviteEmailBody } from './helpers';

const REPO_ROOT = new URL('../..', import.meta.url).pathname;

test.setTimeout(120_000);

function compose(...args: string[]): string {
  return execFileSync('docker', ['compose', ...args], { cwd: REPO_ROOT }).toString();
}

function psql(sql: string): string[] {
  return compose(
    'exec',
    '-T',
    'postgres',
    'psql',
    '-U',
    'chat',
    '-d',
    'chatsystems',
    '-t',
    '-A',
    '-q',
    '-c',
    sql,
  )
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
}

function idList(ids: string[]): string {
  return ids.map((id) => `'${id.replace(/[^0-9a-f-]/g, '')}'`).join(',');
}

test('a stack with no admin is set up from /setup', async ({ page }) => {
  const stamp = Date.now();
  const email = `first-${stamp}@e2e.test`;
  const admins = psql('UPDATE users SET is_instance_admin = FALSE WHERE is_instance_admin RETURNING id');
  const anonymous = await playwrightRequest.newContext();

  try {
    compose('restart', 'api');
    await expect
      .poll(
        async () => {
          const res = await anonymous.get(`${API}/instance/info`).catch(() => null);
          return res?.ok() ? (await res.json()).setup_required : null;
        },
        { timeout: 60_000 },
      )
      .toBe(true);

    const tokens = [...compose('logs', '--no-color', 'api').matchAll(/setup token ([A-Za-z0-9_-]{24})/g)];
    expect(tokens.length, 'the API prints the setup token on start').toBeGreaterThan(0);
    const token = tokens[tokens.length - 1][1];

    await page.goto('/');
    await expect(page).toHaveURL(/\/setup$/);

    await page.getByTestId('setup-token').fill('not-the-token');
    await page.getByTestId('setup-name').fill('First Admin');
    await page.getByTestId('setup-email').fill(email);
    await page.getByTestId('setup-password').fill('a-long-enough-password');
    await page.getByTestId('setup-workspace').fill(`First ${stamp}`);
    await page.getByTestId('setup-submit').click();
    await expect(page.getByTestId('setup-error')).toContainText('setup token is not right');

    await page.getByTestId('setup-token').fill(token);
    await page.getByTestId('setup-submit').click();
    await expect(page.getByTestId('setup-done')).toBeVisible();

    await page.getByTestId('test-email-send').click();
    await expect(page.getByTestId('test-email-sent')).toContainText(email);
    expect(await inviteEmailBody(anonymous, email)).toContain('test email');

    await page.getByTestId('setup-continue').click();
    await expect(page).toHaveURL(/\/app\/[0-9a-f-]{36}/);
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('general');
    await expect(page.getByTestId('channel-intro')).toContainText('This is the very beginning of #general');
    await expect(page.getByTestId('channel-intro-add-people')).toBeVisible();

    const checklist = page.getByTestId('workspace-checklist');
    await expect(checklist).toContainText(`Get First ${stamp} going`);
    await expect(checklist.locator('[data-step="invite"]')).toHaveAttribute('data-done', 'false');
    await page.getByTestId('checklist-channel').click();
    await page.getByTestId('create-channel-name').fill(`plans-${stamp}`);
    await page.getByTestId('create-channel-submit').click();
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(`plans-${stamp}`);
    await expect(checklist.locator('[data-step="channel"]')).toHaveAttribute('data-done', 'true');
    await page.getByTestId('checklist-dismiss').click();
    await expect(checklist).toHaveCount(0);
    await page.reload();
    await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText(`plans-${stamp}`);
    await expect(checklist, 'a dismissed checklist stays dismissed').toHaveCount(0);

    const again = await anonymous.post(`${API}/setup`, {
      data: {
        token,
        display_name: 'Second',
        email: `second-${stamp}@e2e.test`,
        password: 'a-long-enough-password',
        workspace_name: 'Second',
      },
    });
    expect(again.status(), 'the token works once').toBe(409);
  } finally {
    psql(
      `UPDATE users SET is_instance_admin = (email <> '${email}') WHERE id IN (${idList(admins)}) OR email = '${email}'`,
    );
    await anonymous.dispose();
  }
});
