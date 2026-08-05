import { test, expect } from '@playwright/test';
import { authHeaders, login, SHOTS } from './helpers';

const stamp = process.env.E2E_STAMP || 'x1';

test('J. hostile payloads are rendered inert', async ({ page, request }) => {
  const auth = await authHeaders(request, 'admin@dev.local');
  const ws = (await (await request.get('http://localhost:3000/api/workspaces', { headers: auth })).json()).data[0];
  const chans = (await (await request.get(`http://localhost:3000/api/workspaces/${ws.id}/channels`, { headers: auth })).json()).data;
  const general = chans.find((c: { name: string }) => c.name === 'general').id;

  const payloads = [
    `<img src=x onerror="window.__xss=1"> ${stamp}`,
    `<script>window.__xss2=1</script> ${stamp}`,
    `[click](javascript:window.__xss3=1) ${stamp}`,
    `**bold** _italic_ \`code\` ${stamp}`,
  ];
  for (const p of payloads) {
    const r = await request.post(`http://localhost:3000/api/channels/${general}/messages`, {
      headers: auth,
      data: { content: p },
    });
    expect(r.status()).toBe(200);
  }

  let alerted = false;
  page.on('dialog', async (d) => {
    alerted = true;
    await d.dismiss();
  });
  await login(page, 'bob@dev.local');
  await page.waitForTimeout(2500);
  await page.screenshot({ path: `${SHOTS}/J-hostile-payloads.png` });

  expect(await page.evaluate(() => (window as unknown as Record<string, unknown>).__xss)).toBeUndefined();
  expect(await page.evaluate(() => (window as unknown as Record<string, unknown>).__xss2)).toBeUndefined();
  expect(await page.evaluate(() => (window as unknown as Record<string, unknown>).__xss3)).toBeUndefined();
  expect(alerted).toBe(false);
  expect(await page.locator('[data-qa="message-list"] img[src="x"]').count()).toBe(0);
  const jsHrefs = await page.locator('[data-qa="message-list"] a[href^="javascript:"]').count();
  expect(jsHrefs).toBe(0);
});

test('K. write rate limit protects the API', async ({ request }) => {
  const auth = await authHeaders(request, 'charlie@dev.local');
  const ws = (await (await request.get('http://localhost:3000/api/workspaces', { headers: auth })).json()).data[0];
  const chans = (await (await request.get(`http://localhost:3000/api/workspaces/${ws.id}/channels`, { headers: auth })).json()).data;
  const general = chans.find((c: { name: string }) => c.name === 'general').id;

  let limited = 0;
  for (let i = 0; i < 140; i++) {
    const r = await request.post(`http://localhost:3000/api/channels/${general}/messages`, {
      headers: auth,
      data: { content: `flood ${stamp} ${i}` },
    });
    if (r.status() === 429) limited++;
  }
  expect(limited).toBeGreaterThan(0);
});
