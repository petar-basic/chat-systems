import { test, expect } from '@playwright/test';
import { authHeaders, devWorkspace, login, SHOTS } from './helpers';

const stamp = process.env.E2E_STAMP || `x1-${Date.now()}`;

test('J. hostile payloads are rendered inert', async ({ page, request }) => {
  const auth = await authHeaders(request, 'admin@dev.local');
  const ws = await devWorkspace(request, 'admin@dev.local');
  const chans = (
    await (
      await request.get(`http://localhost:3000/api/workspaces/${ws.id}/channels`, { headers: auth })
    ).json()
  ).data;
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
  // Sign-in lands wherever the app decides, which is not necessarily the channel
  // the payloads were posted to. Every payload has to be on screen for the
  // assertions below to be about what rendering them did rather than about
  // whether they rendered at all.
  await page.goto(`/app/${ws.id}/${general}`);
  await expect(page.locator('[data-qa="channel-header-name"]')).toHaveText('general');
  await expect(page.locator('[data-qa="message-row"]', { hasText: stamp })).toHaveCount(payloads.length, {
    timeout: 15_000,
  });
  await page.screenshot({ path: `${SHOTS}/J-hostile-payloads.png` });

  expect(await page.evaluate(() => (window as unknown as Record<string, unknown>).__xss)).toBeUndefined();
  expect(await page.evaluate(() => (window as unknown as Record<string, unknown>).__xss2)).toBeUndefined();
  expect(await page.evaluate(() => (window as unknown as Record<string, unknown>).__xss3)).toBeUndefined();
  expect(alerted).toBe(false);
  expect(await page.locator('[data-qa="message-list"] img[src="x"]').count()).toBe(0);
  const jsHrefs = await page.locator('[data-qa="message-list"] a[href^="javascript:"]').count();
  expect(jsHrefs).toBe(0);
});
