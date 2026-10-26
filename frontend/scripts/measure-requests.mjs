import { chromium } from '@playwright/test';

const BASE = process.env.E2E_BASE_URL || 'http://localhost:8080';
const EMAIL = process.env.MEASURE_EMAIL || 'alice@dev.local';
const PASSWORD = process.env.E2E_PASSWORD || 'admin123456';
const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g;

function exact(request) {
  const url = new URL(request.url());
  return `${request.method()} ${url.pathname}${url.search}`;
}

const shape = (entry) => entry.replace(UUID, '{id}');

function recorder(page) {
  let log = [];
  let inflight = 0;
  let lastChange = Date.now();
  const isApi = (request) => new URL(request.url()).pathname.startsWith('/api/');
  page.on('request', (request) => {
    if (!isApi(request)) return;
    log.push(exact(request));
    inflight += 1;
    lastChange = Date.now();
  });
  const done = (request) => {
    if (!isApi(request)) return;
    inflight -= 1;
    lastChange = Date.now();
  };
  page.on('requestfinished', done);
  page.on('requestfailed', done);

  return {
    reset() {
      log = [];
    },
    async settle(quietMs = 750) {
      while (inflight > 0 || Date.now() - lastChange < quietMs) {
        await page.waitForTimeout(50);
      }
      return log;
    },
  };
}

function tally(entries) {
  const counts = new Map();
  for (const entry of entries) counts.set(entry, (counts.get(entry) ?? 0) + 1);
  return counts;
}

function report(label, log) {
  const twice = [...tally(log)].filter(([, n]) => n > 1).map(([entry]) => shape(entry));
  console.log(`\n${label}: ${log.length} requests`);
  for (const [entry, n] of [...tally(log.map(shape))].sort()) {
    console.log(`  ${n > 1 ? `${n}× ` : '   '}${entry}`);
  }
  if (twice.length) console.log(`  the same request more than once: ${twice.join(', ')}`);
  return { total: log.length, twice: twice.length };
}

const browser = await chromium.launch(
  process.env.MEASURE_CHANNEL ? { channel: process.env.MEASURE_CHANNEL } : {},
);
const page = await browser.newPage();
const requests = recorder(page);

await page.goto(BASE);
await page.locator('[data-qa="login-email"]').fill(EMAIL);
await page.locator('[data-qa="login-password"]').fill(PASSWORD);
requests.reset();
await page.getByRole('button', { name: 'Connect' }).click();
await page.locator('[data-message-id], [data-qa="query-empty"]').first().waitFor({ timeout: 30_000 });
const start = report('Sign-in to the first message', await requests.settle());

const WORKSPACE = process.env.MEASURE_WORKSPACE || 'Dev Team';
const workspaces = await page.evaluate(async () => (await (await fetch('/api/workspaces')).json()).data);
const target = workspaces.find((ws) => ws.name === WORKSPACE);
if (target) {
  await page.goto(`${BASE}/app/${target.id}`);
  await page.locator('[data-message-id], [data-qa="query-empty"]').first().waitFor({ timeout: 30_000 });
}

const rows = page.locator('[data-qa="sidebar-channel-row"] button[data-nav-item]');
const count = await rows.count();
const switches = [];
for (const index of [...[1, 2].filter((i) => i < count), 0]) {
  const row = rows.nth(index);
  const name = (await row.innerText()).trim().split('\n')[0];
  await requests.settle();
  requests.reset();
  await row.click();
  await page.waitForFunction(
    (expected) => document.querySelector('[data-qa="channel-header-name"]')?.textContent?.trim() === expected,
    name,
  );
  switches.push(
    report(`Switch to #${name}${index === 0 ? ' (visited before)' : ''}`, await requests.settle()),
  );
}

console.log(
  `\nSummary: start ${start.total} (${start.twice} made more than once), switches ${switches.map((s) => s.total).join(' / ')}`,
);
await browser.close();
