import { test, expect, type Page } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test.use({
  launchOptions: { args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'] },
  permissions: ['microphone'],
});

const huddleOf = (page: Page) => page.getByRole('dialog', { name: 'Huddle' });

const fakeScreen = () => {
  navigator.mediaDevices.getDisplayMedia = async () => {
    const canvas = document.createElement('canvas');
    canvas.width = 1280;
    canvas.height = 720;
    const ctx = canvas.getContext('2d');
    const paint = () => {
      if (!ctx) return;
      ctx.fillStyle = '#1e293b';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.fillStyle = '#f8fafc';
      ctx.font = '48px sans-serif';
      ctx.fillText(new Date().toISOString(), 40, 80);
    };
    paint();
    window.setInterval(paint, 100);
    return canvas.captureStream(10);
  };
};

const inked = (page: Page) =>
  page.evaluate(() => {
    const canvas = document.querySelector<HTMLCanvasElement>('[data-qa="huddle-draw-layer"]');
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx || canvas.width === 0 || canvas.height === 0) return false;
    const { data } = ctx.getImageData(0, 0, canvas.width, canvas.height);
    for (let i = 3; i < data.length; i += 4) if (data[i] > 0) return true;
    return false;
  });

test('a circle drawn on a shared screen shows for the sharer and fades', async ({ page, browser }) => {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'huddle-drawing');
  const added = await alice.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
  expect(added.status(), 'add a channel member').toBe(200);
  const bobContext = await browser.newContext({ permissions: ['microphone'] });
  const bobPage = await bobContext.newPage();

  try {
    await page.addInitScript(fakeScreen);
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await page.getByRole('button', { name: 'Start huddle' }).click();
    const aliceHuddle = huddleOf(page);
    await expect(aliceHuddle.getByText('Waiting for others to join…')).toBeVisible({ timeout: 20_000 });

    await login(bobPage, 'bob@dev.local');
    await bobPage.goto(`/app/${workspace.id}/${channel.id}`);
    await bobPage.getByRole('button', { name: 'Join huddle' }).click();
    const bobHuddle = huddleOf(bobPage);
    await expect(aliceHuddle.getByText('Waiting for others to join…')).toBeHidden({ timeout: 20_000 });

    await aliceHuddle.getByRole('button', { name: 'Share screen' }).click();
    await expect(aliceHuddle.getByTestId('huddle-drawing-toggle')).toHaveAttribute('aria-pressed', 'true');

    const layer = bobHuddle.getByTestId('huddle-draw-layer');
    await expect(layer).toBeVisible({ timeout: 30_000 });
    await bobHuddle.getByTestId('huddle-pen').click();
    await expect(layer).toHaveAttribute('data-pen', 'true');

    const box = await layer.boundingBox();
    if (!box) throw new Error('the drawing layer has no box');
    const [cx, cy, r] = [box.x + box.width / 2, box.y + box.height / 2, Math.min(box.width, box.height) / 6];
    await bobPage.mouse.move(cx + r, cy);
    await bobPage.mouse.down();
    for (let step = 1; step <= 16; step++) {
      const angle = (step / 16) * 2 * Math.PI;
      await bobPage.mouse.move(cx + r * Math.cos(angle), cy + r * Math.sin(angle));
    }
    await bobPage.mouse.up();

    await expect.poll(() => inked(bobPage), { timeout: 2_000 }).toBe(true);
    await expect.poll(() => inked(page), { timeout: 3_000 }).toBe(true);
    await expect.poll(() => inked(page), { timeout: 8_000 }).toBe(false);
    await expect.poll(() => inked(bobPage), { timeout: 8_000 }).toBe(false);

    await bobPage.keyboard.press('Escape');
    await expect(layer).toHaveAttribute('data-pen', 'false');

    await aliceHuddle.getByTestId('huddle-drawing-toggle').click();
    await expect(aliceHuddle.getByTestId('huddle-drawing-toggle')).toHaveAttribute('aria-pressed', 'false');
    await expect(bobHuddle.getByTestId('huddle-pen')).toBeHidden();
  } finally {
    for (const p of [bobPage, page]) {
      const leave = huddleOf(p).getByRole('button', { name: 'Leave huddle' });
      if (await leave.isVisible()) await leave.click();
    }
    await bobContext.close();
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
