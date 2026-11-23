import { test, expect } from '@playwright/test';
import { API, createChannel, devWorkspace, login, userContext } from './helpers';

test.use({
  launchOptions: { args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'] },
  permissions: ['microphone', 'camera'],
});

test('a clip recorded from the camera goes out with text and plays, by range, for someone else', async ({
  page,
  browser,
}) => {
  test.slow();
  const { ctx: alice } = await userContext('alice@dev.local');
  const { userId: bobId } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(alice);
  const channel = await createChannel(alice, workspace.id, 'clips');
  const added = await alice.post(`${API}/channels/${channel.id}/members`, { data: { user_id: bobId } });
  expect(added.status(), 'add a channel member').toBe(200);
  const bobContext = await browser.newContext();
  const bobPage = await bobContext.newPage();

  try {
    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);
    await expect(page.getByTestId('channel-header-name')).toHaveText(channel.name);

    await page.getByTestId('composer-clip').last().click();
    const recorder = page.getByTestId('clip-recorder');
    await recorder.getByTestId('clip-mode-camera').click();
    await recorder.getByTestId('clip-start').click();
    await expect(recorder.getByTestId('clip-live')).toBeVisible({ timeout: 15_000 });
    await expect(recorder.getByTestId('clip-clock')).toContainText('0:03 of 5:00', { timeout: 15_000 });
    await recorder.getByTestId('clip-stop').click();
    await expect(recorder.getByTestId('clip-player')).toHaveAttribute('data-kind', 'video', {
      timeout: 30_000,
    });
    await recorder.getByTestId('clip-use').click();
    await expect(recorder).toBeHidden();

    await expect(page.locator('[data-qa="attachment-tray-item"][data-status="ready"]')).toHaveCount(1, {
      timeout: 30_000,
    });
    const composer = page.locator('.ProseMirror[contenteditable="true"]').last();
    await composer.click();
    await composer.pressSequentially('the bug, recorded');
    await page.locator('[data-qa="composer-send"]').last().click();
    await expect(page.getByTestId('message-list').getByTestId('clip-player')).toBeVisible({
      timeout: 20_000,
    });

    const ranged: string[] = [];
    bobPage.on('request', (request) => {
      const range = request.headers().range;
      if (request.url().includes('/api/v1/files/download/') && range) ranged.push(range);
    });
    await login(bobPage, 'bob@dev.local');
    await bobPage.goto(`/app/${workspace.id}/${channel.id}`);
    const list = bobPage.getByTestId('message-list');
    await expect(list.getByText('the bug, recorded')).toBeVisible({ timeout: 20_000 });
    const player = list.getByTestId('clip-player');
    await expect(player).toHaveAttribute('data-kind', 'video');
    await expect(player.getByTestId('clip-duration')).toHaveText(/^0:0[2-5]$/);
    const video = player.locator('video');
    await expect(video).toHaveAttribute('poster', /poster/);

    const watched = await video.evaluate(async (element: HTMLVideoElement) => {
      element.muted = true;
      await element.play();
      await new Promise((resolve) => setTimeout(resolve, 500));
      element.pause();
      const middle = element.duration / 2;
      await new Promise<void>((resolve) => {
        element.addEventListener('seeked', () => resolve(), { once: true });
        element.currentTime = middle;
      });
      return { duration: element.duration, at: element.currentTime };
    });
    expect(Number.isFinite(watched.duration), 'the clip knows its length before it is played through').toBe(
      true,
    );
    expect(watched.at).toBeGreaterThan(watched.duration / 2 - 0.5);
    expect(ranged.length, 'the clip is read by range').toBeGreaterThan(0);

    await player.getByTestId('clip-speed').click();
    await player.getByTestId('clip-speed').click();
    await expect(player.getByTestId('clip-speed')).toHaveText('2×');
    expect(await video.evaluate((element: HTMLVideoElement) => element.playbackRate)).toBe(2);
  } finally {
    await bobContext.close();
    await alice.delete(`${API}/channels/${channel.id}`);
    await alice.dispose();
  }
});
