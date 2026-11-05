import { test, expect, type Page } from '@playwright/test';
import { API, login, userContext } from './helpers';

const rootText = (page: Page) =>
  page.evaluate(() => ({
    size: document.documentElement.style.fontSize,
    typeface: document.documentElement.dataset.typeface ?? null,
    body: getComputedStyle(document.body).fontFamily,
  }));

test('a text size and typeface stay after a reload and follow to another browser', async ({
  page,
  browser,
}) => {
  const { ctx: alice } = await userContext('alice@dev.local');

  try {
    await login(page, 'alice@dev.local');
    await page.getByTestId('open-you-menu').click();
    await page.getByTestId('open-preferences').click();
    await page.getByRole('tab', { name: 'Appearance' }).click();
    await page.getByTestId('text-size-125').click();
    await page.getByTestId('typeface-atkinson_hyperlegible').click();
    await expect(page.getByTestId('typeface-atkinson_hyperlegible').getByRole('radio')).toBeChecked();
    await expect
      .poll(async () => (await (await alice.get(`${API}/users/me/preferences`)).json()).typeface)
      .toBe('atkinson_hyperlegible');

    await page.reload();
    await expect(page.getByTestId('open-you-menu')).toBeVisible({ timeout: 20_000 });
    const after = await rootText(page);
    expect(after.size).toBe('125%');
    expect(after.typeface).toBe('atkinson_hyperlegible');
    expect(after.body).toContain('Atkinson Hyperlegible');

    const elsewhere = await browser.newContext();
    const other = await elsewhere.newPage();
    await login(other, 'alice@dev.local');
    await expect.poll(async () => (await rootText(other)).size).toBe('125%');
    await expect.poll(async () => (await rootText(other)).typeface).toBe('atkinson_hyperlegible');
    await elsewhere.close();
  } finally {
    await alice.patch(`${API}/users/me/preferences`, { data: { text_size: '100', typeface: 'system' } });
    await alice.dispose();
  }
});
