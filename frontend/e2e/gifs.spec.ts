import { test, expect, type Page } from '@playwright/test';
import { API, createChannel, devWorkspace, login, openWorkspaceSettings, send, userContext } from './helpers';

const KEY = 'e2eGiphyKey0123456789';

function item(id: string, title: string) {
  const media = `https://media1.giphy.com/media/${id}`;
  return {
    id,
    title,
    alt_text: '',
    images: {
      original: {
        url: `${media}/giphy.gif?cid=e2e`,
        mp4: `${media}/giphy.mp4?cid=e2e&rid=giphy.mp4`,
        width: '480',
        height: '270',
      },
      original_still: { url: `${media}/giphy_s.gif?cid=e2e` },
      fixed_width: {
        url: `${media}/200w.gif?cid=e2e`,
        webp: `${media}/200w.webp?cid=e2e`,
        width: '200',
        height: '113',
      },
      fixed_width_still: { url: `${media}/200w_s.gif?cid=e2e` },
    },
  };
}

async function fakeGiphy(page: Page) {
  const asked: URL[] = [];
  await page.route('https://api.giphy.com/**', async (route) => {
    const url = new URL(route.request().url());
    asked.push(url);
    const words = url.searchParams.get('q') ?? 'trending';
    await route.fulfill({
      json: {
        data: [item('e2eGif1', `${words} one`), item('e2eGif2', `${words} two`)],
        pagination: { total_count: 2, count: 2, offset: 0 },
      },
    });
  });
  await page.route(/^https:\/\/media\d*\.giphy\.com\//, (route) =>
    route.fulfill({ status: 200, contentType: 'application/octet-stream', body: '' }),
  );
  return asked;
}

test('a GIF picked from GIPHY is sent and stays after a reload', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const channel = await createChannel(admin, workspace.id, 'gifs');
  const keyed = await admin.patch(`${API}/admin/settings`, { data: { giphy_api_key: KEY } });
  expect(keyed.status()).toBe(200);
  try {
    const asked = await fakeGiphy(page);
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}/${channel.id}`);

    await page.getByTestId('composer-gif').click();
    const picker = page.getByTestId('gif-picker');
    await expect(picker.getByTestId('gif-attribution')).toHaveText('Powered by GIPHY');
    await expect(picker.getByTestId('gif-choice')).toHaveCount(2);
    expect(asked[0]?.pathname).toBe('/v1/gifs/trending');
    expect(asked[0]?.searchParams.get('api_key')).toBe(KEY);
    expect(asked[0]?.searchParams.get('rating')).toBe('pg');

    await picker.getByTestId('gif-search').fill('party cat');
    await expect(picker.getByTestId('gif-choice').first()).toHaveAttribute('aria-label', 'party cat one');
    const sent = page.waitForResponse(
      (r) => r.url().endsWith(`/channels/${channel.id}/messages`) && r.request().method() === 'POST',
    );
    await picker.getByTestId('gif-choice').first().click();
    expect((await sent).status()).toBe(200);
    await expect(picker).toHaveCount(0);

    const gif = page.getByTestId('message-list').getByTestId('message-gif');
    await expect(gif.locator('video')).toHaveAttribute(
      'src',
      'https://media1.giphy.com/media/e2eGif1/giphy.mp4?cid=e2e&rid=giphy.mp4',
    );
    await expect(gif).toContainText('via GIPHY');

    await page.reload();
    await expect(
      page.getByTestId('message-list').getByTestId('message-gif').locator('video'),
    ).toHaveAttribute('aria-label', 'party cat one');

    await send(page, '/giphy dance');
    await expect(picker.getByTestId('gif-search')).toHaveValue('dance');
    await expect(picker.getByTestId('gif-choice').first()).toHaveAttribute('aria-label', 'dance one');
    await page.keyboard.press('Escape');
    await expect(picker).toHaveCount(0);
  } finally {
    await admin.patch(`${API}/admin/settings`, { data: { giphy_api_key: '' } });
    await admin.dispose();
  }
});

test('a workspace that turns GIFs off loses the GIF button', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  await admin.patch(`${API}/admin/settings`, { data: { giphy_api_key: KEY } });
  try {
    await fakeGiphy(page);
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await expect(page.getByTestId('composer-gif').last()).toBeVisible();

    await openWorkspaceSettings(page);
    const saved = page.waitForResponse(
      (r) => r.url().endsWith(`/workspaces/${workspace.id}`) && r.request().method() === 'PATCH',
    );
    await page.getByTestId('workspace-gifs').uncheck();
    expect((await saved).status()).toBe(200);
    await expect(page.getByTestId('workspace-gif-rating')).toHaveCount(0);
    await page.keyboard.press('Escape');

    await expect(page.getByTestId('composer-gif')).toHaveCount(0);
  } finally {
    await admin.patch(`${API}/workspaces/${workspace.id}`, { data: { gifs: true } });
    await admin.patch(`${API}/admin/settings`, { data: { giphy_api_key: '' } });
    await admin.dispose();
  }
});
